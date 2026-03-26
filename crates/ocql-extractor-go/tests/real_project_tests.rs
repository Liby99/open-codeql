//! Integration tests that extract real-world Go projects (git submodules).
//!
//! These tests validate that extraction succeeds on real code and produces
//! reasonable output. They're #[ignore]d by default since they require
//! the git submodules to be checked out:
//!
//!   git submodule update --init vendor/test-repos/*
//!
//! Run with:
//!   cargo test -p ocql-extractor-go --test real_project_tests -- --ignored

use std::path::Path;

use ocql_database::{Database, Value};
use ocql_extractor_common::Extractor;
use ocql_extractor_go::{GoExtractor, go_schema};

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
    let schema = go_schema();
    let mut db = Database::from_schema(schema);
    let extractor = GoExtractor::new();
    let results = extractor.extract_directory(&mut db, &path);
    (db, results)
}

fn table_count(db: &Database, table: &str) -> usize {
    db.relation(table).map_or(0, |r| r.len())
}

fn column_strings(db: &Database, table: &str, col: usize) -> Vec<String> {
    db.scan(table)
        .unwrap()
        .map(|t| match &t[col] {
            Value::String(s) => db.strings.resolve(*s).to_string(),
            other => format!("{:?}", other),
        })
        .collect()
}

// ============================================================
// fzf — junegunn/fzf
// Medium-sized fuzzy finder — clean, idiomatic Go
// ============================================================

#[test]
#[ignore]
fn fzf_extraction_succeeds() {
    let (_, results) = extract_project("fzf");
    assert!(results.len() >= 30, "fzf should have >= 30 Go files");
    assert!(
        results.iter().all(|r| r.success),
        "All fzf files should extract successfully"
    );
}

#[test]
#[ignore]
fn fzf_packages() {
    let (db, _) = extract_project("fzf");
    let names = column_strings(&db, "go_packages", 1);
    assert!(names.len() >= 5, "fzf should have >= 5 packages, got {}", names.len());
    assert!(names.iter().any(|n| n.contains("fzf")), "should have fzf package");
}

#[test]
#[ignore]
fn fzf_decls() {
    let (db, _) = extract_project("fzf");
    let decl_count = table_count(&db, "go_decls");
    assert!(decl_count >= 50, "fzf should have >= 50 decls, got {}", decl_count);
}

#[test]
#[ignore]
fn fzf_exprs_and_stmts() {
    let (db, _) = extract_project("fzf");
    let expr_count = table_count(&db, "go_exprs");
    let stmt_count = table_count(&db, "go_stmts");
    assert!(expr_count >= 100, "fzf should have >= 100 exprs, got {}", expr_count);
    assert!(stmt_count >= 100, "fzf should have >= 100 stmts, got {}", stmt_count);
}

// ============================================================
// lazygit — jesseduffield/lazygit
// Medium-sized git TUI — tests more complex Go code
// ============================================================

#[test]
#[ignore]
fn lazygit_extraction_succeeds() {
    let (_, results) = extract_project("lazygit");
    assert!(results.len() >= 50, "lazygit should have >= 50 Go files");
    assert!(
        results.iter().all(|r| r.success),
        "All lazygit files should extract successfully"
    );
}

#[test]
#[ignore]
fn lazygit_packages() {
    let (db, _) = extract_project("lazygit");
    let names = column_strings(&db, "go_packages", 1);
    assert!(names.len() >= 10, "lazygit should have >= 10 packages, got {}", names.len());
    assert!(names.iter().any(|n| n.contains("lazygit")), "should have lazygit package");
}

#[test]
#[ignore]
fn lazygit_decls() {
    let (db, _) = extract_project("lazygit");
    let decl_count = table_count(&db, "go_decls");
    assert!(decl_count >= 100, "lazygit should have >= 100 decls, got {}", decl_count);
}

#[test]
#[ignore]
fn lazygit_specs_and_fields() {
    let (db, _) = extract_project("lazygit");
    let spec_count = table_count(&db, "go_specs");
    let field_count = table_count(&db, "go_fields");
    assert!(spec_count >= 50, "lazygit should have >= 50 specs, got {}", spec_count);
    assert!(field_count >= 50, "lazygit should have >= 50 fields, got {}", field_count);
}

// ============================================================
// hugo — gohugoio/hugo
// Large static site generator — tests extraction at scale
// ============================================================

#[test]
#[ignore]
fn hugo_extraction_succeeds() {
    let (_, results) = extract_project("hugo");
    assert!(results.len() >= 200, "hugo should have >= 200 Go files");
    let failures: Vec<_> = results.iter().filter(|r| !r.success).collect();
    assert!(
        failures.is_empty(),
        "hugo should have no extraction failures, got {} failures: {:?}",
        failures.len(),
        failures.iter().map(|f| &f.file_path).collect::<Vec<_>>()
    );
}

#[test]
#[ignore]
fn hugo_packages() {
    let (db, _) = extract_project("hugo");
    let names = column_strings(&db, "go_packages", 1);
    assert!(names.len() >= 20, "hugo should have >= 20 packages, got {}", names.len());
    assert!(names.iter().any(|n| n.contains("hugo")), "should have hugo package");
}

#[test]
#[ignore]
fn hugo_scale() {
    let (db, _) = extract_project("hugo");
    let decl_count = table_count(&db, "go_decls");
    let expr_count = table_count(&db, "go_exprs");
    let stmt_count = table_count(&db, "go_stmts");
    let field_count = table_count(&db, "go_fields");

    eprintln!("hugo: {} decls, {} exprs, {} stmts, {} fields",
        decl_count, expr_count, stmt_count, field_count);

    assert!(decl_count >= 500, "hugo should have >= 500 decls, got {}", decl_count);
    assert!(expr_count >= 1000, "hugo should have >= 1000 exprs, got {}", expr_count);
    assert!(stmt_count >= 1000, "hugo should have >= 1000 stmts, got {}", stmt_count);
    assert!(field_count >= 200, "hugo should have >= 200 fields, got {}", field_count);
}

#[test]
#[ignore]
fn hugo_locations_valid() {
    let (db, _) = extract_project("hugo");
    // Every location should have positive line numbers
    for loc in db.scan("locations_default").unwrap() {
        let line = loc[2].as_int().unwrap();
        assert!(line > 0, "line should be positive, got {}", line);
        let col = loc[3].as_int().unwrap();
        assert!(col > 0, "column should be positive, got {}", col);
    }
    // Every hasLocation should reference an existing location
    let loc_count = table_count(&db, "locations_default");
    let has_loc_count = table_count(&db, "hasLocation");
    assert!(
        has_loc_count > 0 && has_loc_count <= loc_count * 2,
        "hasLocation count ({}) should be reasonable vs locations ({})",
        has_loc_count, loc_count
    );
}

// ============================================================
// Cross-project summary (for manual inspection)
// ============================================================

#[test]
#[ignore]
fn all_projects_summary() {
    let repos = ["fzf", "lazygit", "hugo"];

    eprintln!("\n{:<12} {:>5} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}",
        "Project", "Files", "Pkgs", "Decls", "Specs", "Exprs", "Stmts", "Fields");
    eprintln!("{}", "-".repeat(70));

    for repo in &repos {
        let (db, results) = extract_project(repo);
        let files = results.len();
        let pkgs = table_count(&db, "go_packages");
        let decls = table_count(&db, "go_decls");
        let specs = table_count(&db, "go_specs");
        let exprs = table_count(&db, "go_exprs");
        let stmts = table_count(&db, "go_stmts");
        let fields = table_count(&db, "go_fields");

        eprintln!("{:<12} {:>5} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}",
            repo, files, pkgs, decls, specs, exprs, stmts, fields);
    }
}
