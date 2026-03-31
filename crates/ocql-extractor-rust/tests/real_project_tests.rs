//! Integration tests that extract real-world Rust projects (git submodules).
//!
//! These tests validate that extraction succeeds on real code and produces
//! reasonable output. They're #[ignore]d by default since they require
//! the git submodules to be checked out:
//!
//!   git submodule update --init vendor/test-repos/*
//!
//! Run with:
//!   cargo test -p ocql-extractor-rust --test real_project_tests -- --ignored

use std::path::Path;

use ocql_database::{Database, Value};
use ocql_extractor_common::Extractor;
use ocql_extractor_rust::{RustExtractor, rust_schema};

/// Extract a real project directory, returning the database and results.
fn extract_project(dir: &str) -> (Database, Vec<ocql_extractor_common::ExtractionResult>) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor/test-repos")
        .join(dir);
    if !path.is_dir() {
        panic!(
            "Test repo not found: {:?}\nRun: git submodule update --init vendor/test-repos/{}",
            path, dir
        );
    }
    let schema = rust_schema();
    let mut db = Database::from_schema(schema);
    let extractor = RustExtractor::new();
    let results = extractor.extract_directory(&mut db, &path);
    (db, results)
}

fn table_count(db: &Database, table: &str) -> usize {
    db.relation(table).map_or(0, |r| r.len())
}

fn column_strings(db: &Database, table: &str, col: usize) -> Vec<String> {
    db.scan(table)
        .map(|iter| iter.map(|t| match &t[col] {
            Value::String(s) => db.strings.resolve(*s).to_string(),
            other => format!("{:?}", other),
        }).collect())
        .unwrap_or_default()
}

// ============================================================
// ripgrep — BurntSushi/ripgrep
// Regex search tool — large, well-structured Rust project
// ============================================================

#[test]
#[ignore]
fn ripgrep_extraction_succeeds() {
    let (_, results) = extract_project("ripgrep");
    assert!(results.len() >= 30, "ripgrep should have >= 30 .rs files");
    assert!(
        results.iter().all(|r| r.success),
        "All ripgrep files should extract successfully"
    );
}

#[test]
#[ignore]
fn ripgrep_modules() {
    let (db, _) = extract_project("ripgrep");
    let names = column_strings(&db, "rs_modules", 1);
    assert!(names.len() >= 10, "ripgrep should have >= 10 modules, got {}", names.len());
}

#[test]
#[ignore]
fn ripgrep_functions() {
    let (db, _) = extract_project("ripgrep");
    let func_count = table_count(&db, "rs_functions");
    assert!(func_count >= 100, "ripgrep should have >= 100 functions, got {}", func_count);
}

#[test]
#[ignore]
fn ripgrep_structs() {
    let (db, _) = extract_project("ripgrep");
    let struct_count = table_count(&db, "rs_structs");
    assert!(struct_count >= 20, "ripgrep should have >= 20 structs, got {}", struct_count);
}

#[test]
#[ignore]
fn ripgrep_enums() {
    let (db, _) = extract_project("ripgrep");
    let enum_count = table_count(&db, "rs_enums");
    assert!(enum_count >= 10, "ripgrep should have >= 10 enums, got {}", enum_count);
}

#[test]
#[ignore]
fn ripgrep_traits() {
    let (db, _) = extract_project("ripgrep");
    let trait_count = table_count(&db, "rs_traits");
    assert!(trait_count >= 5, "ripgrep should have >= 5 traits, got {}", trait_count);
}

#[test]
#[ignore]
fn ripgrep_impls() {
    let (db, _) = extract_project("ripgrep");
    let impl_count = table_count(&db, "rs_impls");
    assert!(impl_count >= 30, "ripgrep should have >= 30 impls, got {}", impl_count);
}

// ============================================================
// bat — sharkdp/bat
// Cat clone with syntax highlighting — medium-sized project
// ============================================================

#[test]
#[ignore]
fn bat_extraction_succeeds() {
    let (_, results) = extract_project("bat");
    assert!(results.len() >= 20, "bat should have >= 20 .rs files");
    assert!(
        results.iter().all(|r| r.success),
        "All bat files should extract successfully"
    );
}

#[test]
#[ignore]
fn bat_functions() {
    let (db, _) = extract_project("bat");
    let func_count = table_count(&db, "rs_functions");
    assert!(func_count >= 50, "bat should have >= 50 functions, got {}", func_count);
}

#[test]
#[ignore]
fn bat_structs() {
    let (db, _) = extract_project("bat");
    let struct_count = table_count(&db, "rs_structs");
    assert!(struct_count >= 10, "bat should have >= 10 structs, got {}", struct_count);
}

#[test]
#[ignore]
fn bat_enums() {
    let (db, _) = extract_project("bat");
    let enum_count = table_count(&db, "rs_enums");
    assert!(enum_count >= 5, "bat should have >= 5 enums, got {}", enum_count);
}

#[test]
#[ignore]
fn bat_use_decls() {
    let (db, _) = extract_project("bat");
    let use_count = table_count(&db, "rs_use_decls");
    assert!(use_count >= 20, "bat should have >= 20 use declarations, got {}", use_count);
}

// ============================================================
// fd — sharkdp/fd
// Find alternative — smaller, clean Rust project
// ============================================================

#[test]
#[ignore]
fn fd_extraction_succeeds() {
    let (_, results) = extract_project("fd");
    assert!(results.len() >= 10, "fd should have >= 10 .rs files");
    assert!(
        results.iter().all(|r| r.success),
        "All fd files should extract successfully"
    );
}

#[test]
#[ignore]
fn fd_functions() {
    let (db, _) = extract_project("fd");
    let func_count = table_count(&db, "rs_functions");
    assert!(func_count >= 30, "fd should have >= 30 functions, got {}", func_count);
}

#[test]
#[ignore]
fn fd_structs() {
    let (db, _) = extract_project("fd");
    let struct_count = table_count(&db, "rs_structs");
    assert!(struct_count >= 5, "fd should have >= 5 structs, got {}", struct_count);
}

#[test]
#[ignore]
fn fd_enums() {
    let (db, _) = extract_project("fd");
    let enum_count = table_count(&db, "rs_enums");
    assert!(enum_count >= 3, "fd should have >= 3 enums, got {}", enum_count);
}

#[test]
#[ignore]
fn fd_attributes() {
    let (db, _) = extract_project("fd");
    let attr_count = table_count(&db, "rs_attributes");
    assert!(attr_count >= 10, "fd should have >= 10 attributes, got {}", attr_count);
}

// ============================================================
// Cross-project summary (for manual inspection)
// ============================================================

#[test]
#[ignore]
fn all_projects_summary() {
    let repos = ["ripgrep", "bat", "fd"];

    eprintln!("\n{:<12} {:>5} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}",
        "Project", "Files", "Mods", "Funcs", "Structs", "Enums", "Traits", "Impls", "Uses", "Attrs");
    eprintln!("{}", "-".repeat(90));

    for repo in &repos {
        let (db, results) = extract_project(repo);
        let files = results.len();
        let mods = table_count(&db, "rs_modules");
        let funcs = table_count(&db, "rs_functions");
        let structs = table_count(&db, "rs_structs");
        let enums = table_count(&db, "rs_enums");
        let traits = table_count(&db, "rs_traits");
        let impls = table_count(&db, "rs_impls");
        let uses = table_count(&db, "rs_use_decls");
        let attrs = table_count(&db, "rs_attributes");

        eprintln!("{:<12} {:>5} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}",
            repo, files, mods, funcs, structs, enums, traits, impls, uses, attrs);
    }
}
