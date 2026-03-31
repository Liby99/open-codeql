//! Integration tests that extract real-world Ruby projects (git submodules).
//!
//! These tests validate that extraction succeeds on real code and produces
//! reasonable output. They're #[ignore]d by default since they require
//! the git submodules to be checked out:
//!
//!   git submodule update --init vendor/test-repos/*
//!
//! Run with:
//!   cargo test -p ocql-extractor-ruby --test real_project_tests -- --ignored

use std::path::Path;

use ocql_database::{Database, Value};
use ocql_extractor_common::Extractor;
use ocql_extractor_ruby::{RubyExtractor, ruby_schema};

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
    let schema = ruby_schema();
    let mut db = Database::from_schema(schema);
    let extractor = RubyExtractor::new();
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
// jekyll — jekyll/jekyll (200+ files)
// Static site generator — many classes, modules, and methods
// ============================================================

#[test]
#[ignore]
fn jekyll_extraction_succeeds() {
    let (_, results) = extract_project("jekyll");
    assert!(results.len() >= 30, "jekyll should have >= 30 .rb files");
    assert!(
        results.iter().all(|r| r.success),
        "All jekyll files should extract successfully"
    );
}

#[test]
#[ignore]
fn jekyll_classes() {
    let (db, _) = extract_project("jekyll");
    let names = column_strings(&db, "rb_classes", 1);
    assert!(names.len() >= 15, "jekyll should have >= 15 classes, got {}", names.len());
    // Jekyll has core classes like Site, Page, Document, etc.
    assert!(names.iter().any(|n| n.contains("Site")), "should have Site class");
}

#[test]
#[ignore]
fn jekyll_modules() {
    let (db, _) = extract_project("jekyll");
    let names = column_strings(&db, "rb_modules", 1);
    assert!(names.len() >= 5, "jekyll should have >= 5 modules, got {}", names.len());
    // Jekyll defines the Jekyll module as namespace
    assert!(names.iter().any(|n| n.contains("Jekyll")), "should have Jekyll module");
}

#[test]
#[ignore]
fn jekyll_methods() {
    let (db, _) = extract_project("jekyll");
    let method_count = table_count(&db, "rb_methods");
    assert!(method_count >= 50, "jekyll should have >= 50 methods, got {}", method_count);
}

#[test]
#[ignore]
fn jekyll_requires() {
    let (db, _) = extract_project("jekyll");
    let paths = column_strings(&db, "rb_requires", 1);
    assert!(paths.len() >= 10, "jekyll should have >= 10 requires, got {}", paths.len());
}

// ============================================================
// devise — heartcombo/devise (100+ files)
// Authentication solution — tests complex DSL patterns
// ============================================================

#[test]
#[ignore]
fn devise_extraction_succeeds() {
    let (_, results) = extract_project("devise");
    assert!(results.len() >= 20, "devise should have >= 20 .rb files");
    assert!(
        results.iter().all(|r| r.success),
        "All devise files should extract successfully"
    );
}

#[test]
#[ignore]
fn devise_classes() {
    let (db, _) = extract_project("devise");
    let names = column_strings(&db, "rb_classes", 1);
    assert!(names.len() >= 10, "devise should have >= 10 classes, got {}", names.len());
}

#[test]
#[ignore]
fn devise_modules() {
    let (db, _) = extract_project("devise");
    let names = column_strings(&db, "rb_modules", 1);
    assert!(names.len() >= 5, "devise should have >= 5 modules, got {}", names.len());
    // Devise is the main module
    assert!(names.iter().any(|n| n.contains("Devise")), "should have Devise module");
}

#[test]
#[ignore]
fn devise_methods_and_params() {
    let (db, _) = extract_project("devise");
    let method_count = table_count(&db, "rb_methods");
    let param_count = table_count(&db, "rb_params");
    assert!(method_count >= 30, "devise should have >= 30 methods, got {}", method_count);
    assert!(param_count >= 10, "devise should have >= 10 params, got {}", param_count);
}

// ============================================================
// rack — rack/rack (50+ files)
// Web server interface — clean, well-structured Ruby
// ============================================================

#[test]
#[ignore]
fn rack_extraction_succeeds() {
    let (_, results) = extract_project("rack");
    assert!(results.len() >= 10, "rack should have >= 10 .rb files");
    assert!(
        results.iter().all(|r| r.success),
        "All rack files should extract successfully"
    );
}

#[test]
#[ignore]
fn rack_classes() {
    let (db, _) = extract_project("rack");
    let names = column_strings(&db, "rb_classes", 1);
    assert!(names.len() >= 5, "rack should have >= 5 classes, got {}", names.len());
}

#[test]
#[ignore]
fn rack_modules() {
    let (db, _) = extract_project("rack");
    let names = column_strings(&db, "rb_modules", 1);
    assert!(names.len() >= 3, "rack should have >= 3 modules, got {}", names.len());
    // Rack is the main module
    assert!(names.iter().any(|n| n.contains("Rack")), "should have Rack module");
}

#[test]
#[ignore]
fn rack_methods() {
    let (db, _) = extract_project("rack");
    let method_count = table_count(&db, "rb_methods");
    assert!(method_count >= 20, "rack should have >= 20 methods, got {}", method_count);
}

#[test]
#[ignore]
fn rack_blocks() {
    let (db, _) = extract_project("rack");
    let block_count = table_count(&db, "rb_blocks");
    assert!(block_count >= 5, "rack should have >= 5 blocks, got {}", block_count);
}

// ============================================================
// Cross-project content validation
// ============================================================

#[test]
#[ignore]
fn all_projects_have_locations() {
    let repos = ["jekyll", "devise", "rack"];
    for repo in &repos {
        let (db, _) = extract_project(repo);
        let loc_count = table_count(&db, "locations_default");
        let has_loc_count = table_count(&db, "hasLocation");
        assert!(loc_count > 0, "{} should have locations", repo);
        assert!(has_loc_count > 0, "{} should have hasLocation entries", repo);
    }
}

#[test]
#[ignore]
fn all_projects_have_files() {
    let repos = ["jekyll", "devise", "rack"];
    for repo in &repos {
        let (db, _) = extract_project(repo);
        let file_count = table_count(&db, "files");
        assert!(file_count > 0, "{} should have files", repo);
    }
}

// ============================================================
// Cross-project summary (for manual inspection)
// ============================================================

#[test]
#[ignore]
fn all_projects_summary() {
    let repos = ["jekyll", "devise", "rack"];

    eprintln!("\n{:<12} {:>5} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7}",
        "Project", "Files", "Classes", "Modules", "Methods", "Params", "Stmts", "Exprs", "Blocks");
    eprintln!("{}", "-".repeat(80));

    for repo in &repos {
        let (db, results) = extract_project(repo);
        let files = results.len();
        let classes = table_count(&db, "rb_classes");
        let modules = table_count(&db, "rb_modules");
        let methods = table_count(&db, "rb_methods");
        let params = table_count(&db, "rb_params");
        let stmts = table_count(&db, "rb_stmts");
        let exprs = table_count(&db, "rb_exprs");
        let blocks = table_count(&db, "rb_blocks");

        eprintln!("{:<12} {:>5} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7}",
            repo, files, classes, modules, methods, params, stmts, exprs, blocks);
    }
}
