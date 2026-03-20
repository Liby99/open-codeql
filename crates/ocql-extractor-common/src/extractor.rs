use std::path::Path;

use ocql_database::Database;

use crate::FactEmitter;
use crate::source_roots::{BuildSystem, SourceRoot, discover_source_roots};

/// Result of extracting a single file.
#[derive(Debug)]
pub struct ExtractionResult {
    pub file_path: String,
    pub success: bool,
    pub error: Option<String>,
}

/// Trait for language-specific extractors.
///
/// Implementors provide a tree-sitter language and a method to extract
/// facts from a parsed syntax tree into the database.
pub trait Extractor {
    /// The tree-sitter language for this extractor.
    fn language(&self) -> tree_sitter::Language;

    /// File extensions this extractor handles (e.g., ["c", "h", "cpp", "hpp"]).
    fn extensions(&self) -> &[&str];

    /// Extract facts from a single file's syntax tree into the database.
    ///
    /// The `emitter` provides methods to insert tuples and allocate entity IDs.
    /// The `file_id` is the already-registered entity for this file.
    /// The `tree` is the parsed tree-sitter syntax tree.
    /// The `source` is the raw file content.
    fn extract_file(
        &self,
        emitter: &mut FactEmitter,
        file_id: ocql_database::EntityId,
        tree: &tree_sitter::Tree,
        source: &[u8],
    );

    /// Parse and extract a single source file.
    fn extract_source(
        &self,
        db: &mut Database,
        path: &str,
        source: &[u8],
    ) -> ExtractionResult {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&self.language()).unwrap();

        let tree = match parser.parse(source, None) {
            Some(tree) => tree,
            None => {
                return ExtractionResult {
                    file_path: path.to_string(),
                    success: false,
                    error: Some("tree-sitter parse returned None".to_string()),
                };
            }
        };

        let mut emitter = FactEmitter::new(db);
        let file_id = emitter.alloc();
        emitter.emit_file(file_id, path);

        self.extract_file(&mut emitter, file_id, &tree, source);

        ExtractionResult {
            file_path: path.to_string(),
            success: true,
            error: None,
        }
    }

    /// Extract all matching files from a directory tree.
    fn extract_directory(
        &self,
        db: &mut Database,
        dir: &Path,
    ) -> Vec<ExtractionResult> {
        let mut results = Vec::new();
        let extensions = self.extensions();

        if let Ok(entries) = walkdir(dir) {
            for path in entries {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if extensions.contains(&ext) {
                        match std::fs::read(&path) {
                            Ok(source) => {
                                let path_str = path.to_string_lossy().to_string();
                                results.push(self.extract_source(db, &path_str, &source));
                            }
                            Err(e) => {
                                results.push(ExtractionResult {
                                    file_path: path.to_string_lossy().to_string(),
                                    success: false,
                                    error: Some(format!("Failed to read file: {}", e)),
                                });
                            }
                        }
                    }
                }
            }
        }

        results
    }

    /// Extract a project with build-system-aware source root discovery.
    ///
    /// Detects Gradle/Maven project structure and extracts from the appropriate
    /// source roots (e.g., `src/main/java`, `src/test/java`).
    ///
    /// Returns the detected build system, discovered source roots, and extraction results.
    fn extract_project(
        &self,
        db: &mut Database,
        project_dir: &Path,
        include_tests: bool,
    ) -> ProjectExtractionResult {
        let (build_system, source_roots) = discover_source_roots(project_dir);

        let mut results = Vec::new();
        let active_roots: Vec<&SourceRoot> = source_roots.iter()
            .filter(|r| include_tests || !r.is_test)
            .collect();

        for root in &active_roots {
            let dir_results = self.extract_directory(db, &root.path);
            results.extend(dir_results);
        }

        ProjectExtractionResult {
            build_system,
            source_roots,
            results,
        }
    }
}

/// Result of extracting an entire project.
#[derive(Debug)]
pub struct ProjectExtractionResult {
    pub build_system: BuildSystem,
    pub source_roots: Vec<SourceRoot>,
    pub results: Vec<ExtractionResult>,
}

/// Simple recursive directory walker (avoids adding walkdir as dependency).
fn walkdir(dir: &Path) -> std::io::Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    walkdir_inner(dir, &mut files)?;
    Ok(files)
}

fn walkdir_inner(dir: &Path, files: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                walkdir_inner(&path, files)?;
            } else {
                files.push(path);
            }
        }
    }
    Ok(())
}
