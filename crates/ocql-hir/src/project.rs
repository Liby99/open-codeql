use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A QL pack discovered from a qlpack.yml file.
#[derive(Clone, Debug)]
pub struct QlPack {
    /// Pack name, e.g., "codeql/cpp-all"
    pub name: String,
    /// Root directory (where qlpack.yml lives — this is the import root)
    pub root: PathBuf,
    /// Dependencies: pack name → resolution path
    pub dependencies: Vec<String>,
    /// Whether this is a library pack
    pub is_library: bool,
    /// dbscheme file (relative to root), if specified
    pub dbscheme: Option<String>,
}

/// Manages QL packs and resolves import paths to file paths.
pub struct ProjectIndex {
    /// All discovered packs, keyed by pack name.
    packs: HashMap<String, QlPack>,
    /// Import roots: directories from which imports are resolved.
    /// Each qlpack root is an import root.
    import_roots: Vec<PathBuf>,
    /// All .ql/.qll files discovered, keyed by canonical path.
    files: HashMap<PathBuf, FileInfo>,
    /// Pack name → pack name mapping: what each pack depends on.
    dep_graph: HashMap<String, Vec<String>>,
}

#[derive(Clone, Debug)]
pub struct FileInfo {
    pub path: PathBuf,
    pub pack: Option<String>,
}

impl ProjectIndex {
    /// Discover all QL packs under a workspace root directory.
    pub fn discover(workspace_root: &Path) -> Self {
        let mut packs = HashMap::new();
        let mut import_roots = Vec::new();

        // Find all qlpack.yml files under workspace_root
        discover_qlpacks(workspace_root, &mut packs, &mut import_roots);

        // Resolve transitive dependencies by walking up from workspace root.
        // Repeat until no new deps are found (handles transitive deps).
        let mut all_available: Option<HashMap<String, QlPack>> = None;

        loop {
            let needed_deps: Vec<String> = packs
                .values()
                .flat_map(|p| p.dependencies.iter().cloned())
                .filter(|dep| !packs.contains_key(dep))
                .collect();

            if needed_deps.is_empty() {
                break;
            }

            // Lazily discover all available packs from parent directories
            if all_available.is_none() {
                let mut available = HashMap::new();
                let mut search_root = workspace_root.to_path_buf();
                for _ in 0..5 {
                    if let Some(parent) = search_root.parent() {
                        search_root = parent.to_path_buf();
                        let mut extra_packs = HashMap::new();
                        let mut extra_roots = Vec::new();
                        discover_qlpacks(&search_root, &mut extra_packs, &mut extra_roots);
                        for (name, pack) in extra_packs {
                            available.entry(name).or_insert(pack);
                        }
                    } else {
                        break;
                    }
                }
                all_available = Some(available);
            }

            let available = all_available.as_mut().unwrap();
            let mut found_any = false;
            for dep in &needed_deps {
                if let Some(pack) = available.remove(dep) {
                    import_roots.push(pack.root.clone());
                    packs.insert(pack.name.clone(), pack);
                    found_any = true;
                }
            }
            if !found_any {
                break; // Can't find any more deps
            }
        }

        // If no qlpacks found, treat the root itself as an import root
        if import_roots.is_empty() {
            import_roots.push(workspace_root.to_path_buf());
        }

        // Build dependency graph
        let dep_graph: HashMap<String, Vec<String>> = packs
            .values()
            .map(|p| (p.name.clone(), p.dependencies.clone()))
            .collect();

        // Discover all .ql/.qll files
        let mut files = HashMap::new();
        for root in &import_roots {
            let pack_name = packs
                .values()
                .find(|p| p.root == *root)
                .map(|p| p.name.clone());
            discover_ql_files(root, pack_name.as_deref(), &mut files);
        }

        Self {
            packs,
            import_roots,
            files,
            dep_graph,
        }
    }

    /// Resolve an import path like "semmle.code.cpp.Element" to a file path.
    ///
    /// Resolution rules (tried in order):
    /// 1. Relative to importing file's directory
    /// 2. Relative to importing file's pack root
    /// 3. Search all import roots
    /// 4. Cross-pack: match pack name prefix in dependencies, resolve within that pack
    pub fn resolve_import(&self, import_path: &[String], from_file: &Path) -> Option<PathBuf> {
        if import_path.is_empty() {
            return None;
        }

        // Build the relative path: "semmle.code.cpp.Element" → "semmle/code/cpp/Element.qll"
        let relative = import_path.join("/") + ".qll";

        // Strategy 1: Resolve relative to the importing file's directory
        if let Some(parent) = from_file.parent() {
            let candidate = parent.join(&relative);
            if candidate.exists() {
                return Some(normalize_path(&candidate));
            }
            // Also try as a directory (import Foo → Foo/Foo.qll or Foo.qll)
            if import_path.len() == 1 {
                let dir_candidate = parent.join(&import_path[0]).join(&relative);
                if dir_candidate.exists() {
                    return Some(normalize_path(&dir_candidate));
                }
            }
        }

        // Strategy 1b: Walk up parent directories for short import paths
        if import_path.len() <= 2 {
            if let Some(mut dir) = from_file.parent().map(|p| p.to_path_buf()) {
                let pack_root = self.find_pack_root(from_file);
                loop {
                    if let Some(parent) = dir.parent() {
                        let candidate = parent.join(&relative);
                        if candidate.exists() {
                            return Some(normalize_path(&candidate));
                        }
                        // Stop at pack root boundary
                        if pack_root.is_some_and(|r| parent == r) {
                            break;
                        }
                        dir = parent.to_path_buf();
                    } else {
                        break;
                    }
                }
            }
        }

        // Strategy 2: Resolve relative to the importing file's pack root
        if let Some(pack_root) = self.find_pack_root(from_file) {
            let candidate = pack_root.join(&relative);
            if candidate.exists() {
                return Some(normalize_path(&candidate));
            }
        }

        // Strategy 3: Search all import roots
        for root in &self.import_roots {
            let candidate = root.join(&relative);
            if candidate.exists() {
                return Some(normalize_path(&candidate));
            }
        }

        // Strategy 4: Cross-pack resolution via dependency graph.
        // For "codeql.util.Unit", check if any dependency pack matches a prefix.
        // Pack names use "/" (e.g., "codeql/util"), import paths use "." (e.g., "codeql.util.Unit").
        let from_pack = self.find_pack_name(from_file);
        let dep_pack_names: Vec<&str> = match from_pack.as_deref() {
            Some(name) => self
                .dep_graph
                .get(name)
                .map(|deps| deps.iter().map(|s| s.as_str()).collect())
                .unwrap_or_default(),
            None => {
                // If not in a pack, search all packs
                self.packs.keys().map(|s| s.as_str()).collect()
            }
        };

        for &dep_name in &dep_pack_names {
            if let Some(dep_pack) = self.packs.get(dep_name) {
                // Convert pack name "codeql/util" to dot form "codeql.util"
                let _pack_dot_name = dep_name.replace('/', ".");
                let pack_segments: Vec<&str> = dep_name.split('/').collect();

                // Check if import path starts with this pack's dotted name
                if import_path.len() > pack_segments.len() {
                    let prefix_matches = pack_segments
                        .iter()
                        .zip(import_path.iter())
                        .all(|(a, b)| *a == b.as_str());

                    if prefix_matches {
                        // Resolve remainder relative to pack root
                        // e.g., "codeql.util.Unit" with pack "codeql/util" →
                        //   resolve "codeql/util/Unit.qll" relative to pack root
                        let full_relative = import_path.join("/") + ".qll";
                        let candidate = dep_pack.root.join(&full_relative);
                        if candidate.exists() {
                            return Some(normalize_path(&candidate));
                        }

                        // Also try just the suffix after the pack name
                        let suffix: Vec<&str> =
                            import_path[pack_segments.len()..].iter().map(|s| s.as_str()).collect();
                        let suffix_relative = suffix.join("/") + ".qll";
                        let candidate = dep_pack.root.join(&suffix_relative);
                        if candidate.exists() {
                            return Some(normalize_path(&candidate));
                        }
                    }
                }

                // Also check: the import might just match the pack name's last segment
                // e.g., "import dataflow.DataFlow" could resolve in codeql/dataflow pack
                if pack_segments.len() == 2 && import_path[0] == pack_segments[1] {
                    // Prepend the first pack segment
                    let mut full: Vec<&str> = vec![pack_segments[0]];
                    full.extend(import_path.iter().map(|s| s.as_str()));
                    let full_relative = full.join("/") + ".qll";
                    let candidate = dep_pack.root.join(&full_relative);
                    if candidate.exists() {
                        return Some(normalize_path(&candidate));
                    }
                }
            }
        }

        // Strategy 5: Last resort — search all packs (not just deps) for the import path
        for pack in self.packs.values() {
            let candidate = pack.root.join(&relative);
            if candidate.exists() {
                return Some(normalize_path(&candidate));
            }
        }

        None
    }

    /// Find the pack root for a file (the nearest ancestor with qlpack.yml).
    fn find_pack_root(&self, file: &Path) -> Option<&Path> {
        // Find the most specific (longest path) import root that contains this file
        let mut best: Option<&Path> = None;
        for root in &self.import_roots {
            if file.starts_with(root) {
                if best.is_none()
                    || root.as_os_str().len() > best.unwrap().as_os_str().len()
                {
                    best = Some(root);
                }
            }
        }
        best
    }

    /// Find the pack name for a file.
    fn find_pack_name(&self, file: &Path) -> Option<String> {
        let mut best: Option<&QlPack> = None;
        for pack in self.packs.values() {
            if file.starts_with(&pack.root) {
                if best.is_none()
                    || pack.root.as_os_str().len()
                        > best.unwrap().root.as_os_str().len()
                {
                    best = Some(pack);
                }
            }
        }
        best.map(|p| p.name.clone())
    }

    /// Get all discovered .ql/.qll file paths.
    pub fn all_files(&self) -> impl Iterator<Item = &PathBuf> {
        self.files.keys()
    }

    /// Get all import roots.
    pub fn import_roots(&self) -> &[PathBuf] {
        &self.import_roots
    }

    /// Number of discovered files.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Number of discovered packs.
    pub fn pack_count(&self) -> usize {
        self.packs.len()
    }

    /// Get pack names.
    pub fn pack_names(&self) -> impl Iterator<Item = &str> {
        self.packs.keys().map(|s| s.as_str())
    }

    /// Get all dbscheme file paths from discovered packs.
    pub fn dbscheme_paths(&self) -> Vec<PathBuf> {
        self.packs
            .values()
            .filter_map(|p| {
                p.dbscheme.as_ref().map(|d| p.root.join(d))
            })
            .filter(|p| p.exists())
            .collect()
    }
}

/// Recursively discover qlpack.yml files.
fn discover_qlpacks(
    dir: &Path,
    packs: &mut HashMap<String, QlPack>,
    import_roots: &mut Vec<PathBuf>,
) {
    let qlpack_yml = dir.join("qlpack.yml");
    if qlpack_yml.exists() {
        if let Some(pack) = parse_qlpack_yml(&qlpack_yml) {
            if !packs.contains_key(&pack.name) {
                import_roots.push(pack.root.clone());
                packs.insert(pack.name.clone(), pack);
            }
        }
        // Don't recurse into subdirectories of a pack root looking for more packs.
        // Packs don't nest. But we DO recurse to find sibling packs.
        return;
    }

    // Recurse into subdirectories
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.')
                || name_str == "node_modules"
                || name_str == "target"
                || name_str == "ql-packs"
            {
                continue;
            }
            discover_qlpacks(&path, packs, import_roots);
        }
    }
}

/// Parse a qlpack.yml file (minimal YAML parsing — just extract name and deps).
fn parse_qlpack_yml(path: &Path) -> Option<QlPack> {
    let content = std::fs::read_to_string(path).ok()?;
    let root = path.parent()?.to_path_buf();

    let mut name = None;
    let mut dependencies = Vec::new();
    let mut is_library = false;
    let mut dbscheme = None;
    let mut in_dependencies = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Detect the "dependencies:" section
        if trimmed.starts_with("dependencies:") {
            let rest = trimmed.strip_prefix("dependencies:").unwrap().trim();
            if rest == "null" || rest == "~" {
                in_dependencies = false;
            } else {
                in_dependencies = true;
            }
            continue;
        }

        // If we're in the dependencies section, collect dependency names
        if in_dependencies {
            // Dependencies are indented lines like "  codeql/dataflow: ${workspace}"
            if line.starts_with(' ') || line.starts_with('\t') {
                if let Some(dep_name) = trimmed.split(':').next() {
                    let dep = dep_name.trim().to_string();
                    if !dep.is_empty() && !dep.starts_with('#') {
                        dependencies.push(dep);
                    }
                }
            } else {
                // Non-indented line: we've left the dependencies section
                in_dependencies = false;
            }
        }

        if trimmed.starts_with("name:") {
            let rest = trimmed.strip_prefix("name:").unwrap().trim();
            name = Some(rest.trim_matches('"').trim_matches('\'').to_string());
        } else if trimmed.starts_with("library:") {
            let rest = trimmed.strip_prefix("library:").unwrap().trim();
            is_library = rest == "true";
        } else if trimmed.starts_with("dbscheme:") {
            let rest = trimmed.strip_prefix("dbscheme:").unwrap().trim();
            let val = rest.trim_matches('"').trim_matches('\'').to_string();
            if !val.is_empty() {
                dbscheme = Some(val);
            }
        }
    }

    Some(QlPack {
        name: name?,
        root,
        dependencies,
        is_library,
        dbscheme,
    })
}

/// Recursively discover .ql/.qll files under a root.
fn discover_ql_files(
    dir: &Path,
    pack_name: Option<&str>,
    files: &mut HashMap<PathBuf, FileInfo>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.')
                || name_str == "node_modules"
                || name_str == "target"
                || name_str == "test"
                || name_str == "upgrades"
            {
                continue;
            }
            discover_ql_files(&path, pack_name, files);
        } else if let Some(ext) = path.extension() {
            if ext == "ql" || ext == "qll" {
                let canonical = normalize_path(&path);
                files.insert(
                    canonical.clone(),
                    FileInfo {
                        path: canonical,
                        pack: pack_name.map(|s| s.to_string()),
                    },
                );
            }
        }
    }
}

/// Normalize a path (resolve . and .., but don't require the file to exist for canonicalize).
fn normalize_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
