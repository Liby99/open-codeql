//! Build-system-aware source root discovery for Java projects.
//!
//! Detects Gradle (settings.gradle, build.gradle) and Maven (pom.xml) project
//! structures, and returns the set of source root directories that contain
//! Java source files.

use std::path::{Path, PathBuf};

/// Detected build system kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildSystem {
    Gradle,
    Maven,
    None,
}

/// A discovered source root with metadata.
#[derive(Debug, Clone)]
pub struct SourceRoot {
    /// Absolute path to the source directory (e.g. `project/app/src/main/java`).
    pub path: PathBuf,
    /// Whether this is a test source root.
    pub is_test: bool,
    /// The module name (e.g. "app", "gson", ":auto-value-gson-factory").
    pub module: String,
}

/// Discover Java source roots in a project directory.
///
/// Strategy:
/// 1. Detect build system (Gradle or Maven).
/// 2. Parse submodule/module declarations.
/// 3. For each module, look for `src/main/java` and `src/test/java`.
/// 4. If no build system found, fall back to scanning for any `src/**/java` dirs,
///    or just return the root directory.
pub fn discover_source_roots(project_dir: &Path) -> (BuildSystem, Vec<SourceRoot>) {
    // Try Gradle first
    if project_dir.join("settings.gradle").exists()
        || project_dir.join("settings.gradle.kts").exists()
        || project_dir.join("build.gradle").exists()
        || project_dir.join("build.gradle.kts").exists()
    {
        let modules = discover_gradle_modules(project_dir);
        let roots = find_source_roots_for_modules(project_dir, &modules);
        if !roots.is_empty() {
            return (BuildSystem::Gradle, roots);
        }
    }

    // Try Maven
    if project_dir.join("pom.xml").exists() {
        let modules = discover_maven_modules(project_dir);
        let roots = find_source_roots_for_modules(project_dir, &modules);
        if !roots.is_empty() {
            return (BuildSystem::Maven, roots);
        }
    }

    // Fallback: look for any src/**/java directories
    let roots = discover_source_dirs_heuristic(project_dir);
    if !roots.is_empty() {
        return (BuildSystem::None, roots);
    }

    // Ultimate fallback: the project directory itself
    (BuildSystem::None, vec![SourceRoot {
        path: project_dir.to_path_buf(),
        is_test: false,
        module: String::new(),
    }])
}

/// Discover Gradle submodules from settings.gradle.
fn discover_gradle_modules(project_dir: &Path) -> Vec<String> {
    let mut modules = Vec::new();

    // The root project is always a module
    modules.push(String::new());

    // Try settings.gradle, then settings.gradle.kts
    let settings_path = if project_dir.join("settings.gradle").exists() {
        project_dir.join("settings.gradle")
    } else if project_dir.join("settings.gradle.kts").exists() {
        project_dir.join("settings.gradle.kts")
    } else {
        return modules;
    };

    if let Ok(content) = std::fs::read_to_string(&settings_path) {
        for line in content.lines() {
            let trimmed = line.trim();
            // Match: include ':app', ':library'
            // Match: include(":app", ":library")
            if trimmed.starts_with("include") {
                // Extract all quoted strings after "include"
                let rest = &trimmed["include".len()..];
                for module in extract_quoted_strings(rest) {
                    // Strip leading ':'
                    let module = module.strip_prefix(':').unwrap_or(&module);
                    // Convert ':' separators to '/' for nested modules
                    let path = module.replace(':', "/");
                    modules.push(path);
                }
            }
        }
    }

    modules
}

/// Discover Maven modules from pom.xml.
fn discover_maven_modules(project_dir: &Path) -> Vec<String> {
    let mut modules = Vec::new();

    // The root project is always a module
    modules.push(String::new());

    let pom_path = project_dir.join("pom.xml");
    if let Ok(content) = std::fs::read_to_string(&pom_path) {
        // Simple XML parsing: look for <module>name</module>
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("<module>") {
                if let Some(module_name) = rest.strip_suffix("</module>") {
                    modules.push(module_name.to_string());
                }
            }
        }
    }

    // Also recurse into discovered modules to find sub-modules
    let initial_len = modules.len();
    for i in 1..initial_len {
        let sub_pom = project_dir.join(&modules[i]).join("pom.xml");
        if let Ok(content) = std::fs::read_to_string(&sub_pom) {
            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("<module>") {
                    if let Some(module_name) = rest.strip_suffix("</module>") {
                        let full_path = format!("{}/{}", modules[i], module_name);
                        modules.push(full_path);
                    }
                }
            }
        }
    }

    modules
}

/// For a list of module directories, find their Java source roots.
fn find_source_roots_for_modules(project_dir: &Path, modules: &[String]) -> Vec<SourceRoot> {
    let mut roots = Vec::new();

    for module in modules {
        let module_dir = if module.is_empty() {
            project_dir.to_path_buf()
        } else {
            project_dir.join(module)
        };

        if !module_dir.is_dir() {
            continue;
        }

        // Standard Maven/Gradle source layout
        let has_main = module_dir.join("src/main/java").is_dir();
        let has_test = module_dir.join("src/test/java").is_dir();

        if has_main {
            roots.push(SourceRoot {
                path: module_dir.join("src/main/java"),
                is_test: false,
                module: module.clone(),
            });
        }

        if has_test {
            roots.push(SourceRoot {
                path: module_dir.join("src/test/java"),
                is_test: true,
                module: module.clone(),
            });
        }

        // Android projects sometimes have src/androidTest/java
        let android_test = module_dir.join("src/androidTest/java");
        if android_test.is_dir() {
            roots.push(SourceRoot {
                path: android_test,
                is_test: true,
                module: module.clone(),
            });
        }

        // Some projects just have src/java or just src/
        if !has_main && !has_test {
            let src_java = module_dir.join("src/java");
            if src_java.is_dir() {
                roots.push(SourceRoot {
                    path: src_java,
                    is_test: false,
                    module: module.clone(),
                });
            } else {
                // Check if src/ directly contains .java files or package dirs
                let src = module_dir.join("src");
                if src.is_dir() && has_java_files_shallow(&src) {
                    roots.push(SourceRoot {
                        path: src,
                        is_test: false,
                        module: module.clone(),
                    });
                }
            }
        }
    }

    roots
}

/// Heuristic: walk the project tree looking for directories that look like source roots.
fn discover_source_dirs_heuristic(project_dir: &Path) -> Vec<SourceRoot> {
    let mut roots = Vec::new();

    // Walk up to 4 levels deep looking for src/main/java or src/test/java
    find_source_dirs_recursive(project_dir, project_dir, 0, 4, &mut roots);

    roots
}

fn find_source_dirs_recursive(
    base: &Path,
    dir: &Path,
    depth: usize,
    max_depth: usize,
    roots: &mut Vec<SourceRoot>,
) {
    if depth > max_depth || !dir.is_dir() {
        return;
    }

    let main_java = dir.join("src/main/java");
    if main_java.is_dir() {
        let module = dir.strip_prefix(base).unwrap_or(Path::new(""))
            .to_string_lossy().to_string();
        roots.push(SourceRoot {
            path: main_java,
            is_test: false,
            module,
        });
        // Don't recurse further into this module
        return;
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                // Skip hidden dirs, build dirs, etc.
                if name_str.starts_with('.')
                    || name_str == "build"
                    || name_str == "target"
                    || name_str == "node_modules"
                    || name_str == "bin"
                    || name_str == ".gradle"
                {
                    continue;
                }
                find_source_dirs_recursive(base, &path, depth + 1, max_depth, roots);
            }
        }
    }
}

/// Check if a directory has .java files directly or in its first-level subdirectories.
fn has_java_files_shallow(dir: &Path) -> bool {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "java" {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Extract quoted strings from a line (handles both single and double quotes).
fn extract_quoted_strings(s: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut chars = s.chars().peekable();

    while let Some(&c) = chars.peek() {
        if c == '\'' || c == '"' {
            let quote = c;
            chars.next(); // consume opening quote
            let mut value = String::new();
            for ch in chars.by_ref() {
                if ch == quote {
                    break;
                }
                value.push(ch);
            }
            if !value.is_empty() {
                results.push(value);
            }
        } else {
            chars.next();
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_quoted_strings_single() {
        let result = extract_quoted_strings("':app', ':library'");
        assert_eq!(result, vec![":app", ":library"]);
    }

    #[test]
    fn extract_quoted_strings_double() {
        let result = extract_quoted_strings("(\":app\", \":library\")");
        assert_eq!(result, vec![":app", ":library"]);
    }

    #[test]
    fn extract_quoted_strings_mixed() {
        let result = extract_quoted_strings(" ':core'");
        assert_eq!(result, vec![":core"]);
    }

    #[test]
    fn extract_quoted_nested_module() {
        let result = extract_quoted_strings("(':auto-value-gson:core')");
        assert_eq!(result, vec![":auto-value-gson:core"]);
    }
}
