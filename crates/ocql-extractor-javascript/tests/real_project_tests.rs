//! Integration tests that extract real-world JavaScript/TypeScript projects (git submodules).
//!
//! These tests validate that extraction succeeds on real code and produces
//! reasonable output. They're #[ignore]d by default since they require
//! the git submodules to be checked out:
//!
//!   git submodule update --init vendor/test-repos/express vendor/test-repos/preact \
//!     vendor/test-repos/zod
//!
//! Run with:
//!   cargo test -p ocql-extractor-javascript --test real_project_tests -- --ignored

use std::path::Path;

use ocql_database::{Database, Value};
use ocql_extractor_common::Extractor;
use ocql_extractor_javascript::{JavaScriptExtractor, TypeScriptExtractor, javascript_schema};

/// Extract a JavaScript project directory, returning the database and results.
fn extract_js_project(dir: &str) -> (Database, Vec<ocql_extractor_common::ExtractionResult>) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor/test-repos")
        .join(dir);
    if !path.is_dir() {
        panic!(
            "Test repo not found: {:?}\nRun: git submodule update --init vendor/test-repos/{}",
            path, dir
        );
    }
    let schema = javascript_schema();
    let mut db = Database::from_schema(schema);
    let extractor = JavaScriptExtractor::new();
    let results = extractor.extract_directory(&mut db, &path);
    (db, results)
}

/// Extract a TypeScript project directory, returning the database and results.
fn extract_ts_project(dir: &str) -> (Database, Vec<ocql_extractor_common::ExtractionResult>) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor/test-repos")
        .join(dir);
    if !path.is_dir() {
        panic!(
            "Test repo not found: {:?}\nRun: git submodule update --init vendor/test-repos/{}",
            path, dir
        );
    }
    let schema = javascript_schema();
    let mut db = Database::from_schema(schema);
    let extractor = TypeScriptExtractor::new();
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
// express — expressjs/express (JS web framework)
// ============================================================

#[test]
#[ignore]
fn express_extraction_succeeds() {
    let (_, results) = extract_js_project("express");
    assert!(results.len() >= 40, "express should have >= 40 JS files, got {}", results.len());
    let failures: Vec<_> = results.iter().filter(|r| !r.success).collect();
    assert!(
        failures.is_empty(),
        "express should have no extraction failures, got {} failures: {:?}",
        failures.len(),
        failures.iter().map(|f| &f.file_path).collect::<Vec<_>>()
    );
}

#[test]
#[ignore]
fn express_toplevels() {
    let (db, _) = extract_js_project("express");
    let count = table_count(&db, "js_toplevels");
    assert!(count >= 40, "express should have >= 40 toplevels, got {}", count);
}

#[test]
#[ignore]
fn express_functions() {
    let (db, _) = extract_js_project("express");
    let count = table_count(&db, "js_functions");
    assert!(count >= 100, "express should have >= 100 functions, got {}", count);
}

#[test]
#[ignore]
fn express_scale() {
    let (db, _) = extract_js_project("express");
    let toplevel_count = table_count(&db, "js_toplevels");
    let stmt_count = table_count(&db, "js_stmts");
    let expr_count = table_count(&db, "js_exprs");
    let func_count = table_count(&db, "js_functions");

    eprintln!("express: {} toplevels, {} stmts, {} exprs, {} functions",
        toplevel_count, stmt_count, expr_count, func_count);

    assert!(toplevel_count >= 40, "express should have >= 40 toplevels, got {}", toplevel_count);
    assert!(stmt_count >= 800, "express should have >= 800 stmts, got {}", stmt_count);
    assert!(expr_count >= 1500, "express should have >= 1500 exprs, got {}", expr_count);
}

#[test]
#[ignore]
fn express_exports() {
    let (db, _) = extract_js_project("express");
    let count = table_count(&db, "js_exports");
    assert!(count >= 10, "express should have >= 10 exports, got {}", count);
}

// ============================================================
// preact — preactjs/preact (JS UI library)
// ============================================================

#[test]
#[ignore]
fn preact_extraction_succeeds() {
    let (_, results) = extract_js_project("preact");
    assert!(results.len() >= 50, "preact should have >= 50 JS files, got {}", results.len());
    let failures: Vec<_> = results.iter().filter(|r| !r.success).collect();
    assert!(
        failures.is_empty(),
        "preact should have no extraction failures, got {} failures: {:?}",
        failures.len(),
        failures.iter().map(|f| &f.file_path).collect::<Vec<_>>()
    );
}

#[test]
#[ignore]
fn preact_toplevels() {
    let (db, _) = extract_js_project("preact");
    let count = table_count(&db, "js_toplevels");
    assert!(count >= 50, "preact should have >= 50 toplevels, got {}", count);
}

#[test]
#[ignore]
fn preact_functions() {
    let (db, _) = extract_js_project("preact");
    let count = table_count(&db, "js_functions");
    assert!(count >= 80, "preact should have >= 80 functions, got {}", count);
}

#[test]
#[ignore]
fn preact_classes() {
    let (db, _) = extract_js_project("preact");
    let count = table_count(&db, "js_classes");
    assert!(count >= 5, "preact should have >= 5 classes, got {}", count);
}

#[test]
#[ignore]
fn preact_scale() {
    let (db, _) = extract_js_project("preact");
    let toplevel_count = table_count(&db, "js_toplevels");
    let stmt_count = table_count(&db, "js_stmts");
    let expr_count = table_count(&db, "js_exprs");
    let func_count = table_count(&db, "js_functions");
    let class_count = table_count(&db, "js_classes");

    eprintln!("preact: {} toplevels, {} stmts, {} exprs, {} functions, {} classes",
        toplevel_count, stmt_count, expr_count, func_count, class_count);

    assert!(toplevel_count >= 50, "preact should have >= 50 toplevels, got {}", toplevel_count);
    assert!(stmt_count >= 1000, "preact should have >= 1000 stmts, got {}", stmt_count);
    assert!(expr_count >= 2000, "preact should have >= 2000 exprs, got {}", expr_count);
}

#[test]
#[ignore]
fn preact_imports_exports() {
    let (db, _) = extract_js_project("preact");
    let import_count = table_count(&db, "js_imports");
    let export_count = table_count(&db, "js_exports");
    assert!(import_count >= 20, "preact should have >= 20 imports, got {}", import_count);
    assert!(export_count >= 20, "preact should have >= 20 exports, got {}", export_count);
}

// ============================================================
// zod — colinhacks/zod (TypeScript schema validation)
// ============================================================

#[test]
#[ignore]
fn zod_extraction_succeeds() {
    let (_, results) = extract_ts_project("zod");
    assert!(results.len() >= 40, "zod should have >= 40 TS files, got {}", results.len());
    let failures: Vec<_> = results.iter().filter(|r| !r.success).collect();
    assert!(
        failures.is_empty(),
        "zod should have no extraction failures, got {} failures: {:?}",
        failures.len(),
        failures.iter().map(|f| &f.file_path).collect::<Vec<_>>()
    );
}

#[test]
#[ignore]
fn zod_toplevels() {
    let (db, _) = extract_ts_project("zod");
    let count = table_count(&db, "js_toplevels");
    assert!(count >= 40, "zod should have >= 40 toplevels, got {}", count);
}

#[test]
#[ignore]
fn zod_functions() {
    let (db, _) = extract_ts_project("zod");
    let count = table_count(&db, "js_functions");
    assert!(count >= 100, "zod should have >= 100 functions, got {}", count);
}

#[test]
#[ignore]
fn zod_classes() {
    let (db, _) = extract_ts_project("zod");
    let count = table_count(&db, "js_classes");
    let names = column_strings(&db, "js_classes", 1);
    assert!(count >= 20, "zod should have >= 20 classes, got {}", count);
    // Look for some core Zod schema classes
    let zod_classes: Vec<_> = names.iter().filter(|n| n.starts_with("Zod") || n == "Schema").collect();
    assert!(
        !zod_classes.is_empty(),
        "zod should have Zod* classes, got class names sample: {:?}",
        &names[..names.len().min(10)]
    );
}

#[test]
#[ignore]
fn zod_scale() {
    let (db, _) = extract_ts_project("zod");
    let toplevel_count = table_count(&db, "js_toplevels");
    let stmt_count = table_count(&db, "js_stmts");
    let expr_count = table_count(&db, "js_exprs");
    let func_count = table_count(&db, "js_functions");
    let class_count = table_count(&db, "js_classes");

    eprintln!("zod: {} toplevels, {} stmts, {} exprs, {} functions, {} classes",
        toplevel_count, stmt_count, expr_count, func_count, class_count);

    assert!(toplevel_count >= 40, "zod should have >= 40 toplevels, got {}", toplevel_count);
    assert!(stmt_count >= 1500, "zod should have >= 1500 stmts, got {}", stmt_count);
    assert!(expr_count >= 3000, "zod should have >= 3000 exprs, got {}", expr_count);
    assert!(class_count >= 20, "zod should have >= 20 classes, got {}", class_count);
}

#[test]
#[ignore]
fn zod_exports() {
    let (db, _) = extract_ts_project("zod");
    let count = table_count(&db, "js_exports");
    assert!(count >= 30, "zod should have >= 30 exports, got {}", count);
}

#[test]
#[ignore]
fn zod_locations_valid() {
    let (db, _) = extract_ts_project("zod");
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
    eprintln!("\n{:<12} {:>8} {:>5} {:>9} {:>6} {:>6} {:>7} {:>7} {:>7} {:>7}",
        "Project", "Lang", "Files", "Toplevels", "Funcs", "Classes", "Stmts", "Exprs", "Imports", "Exports");
    eprintln!("{}", "-".repeat(90));

    // JavaScript projects
    for repo in &["express", "preact"] {
        let (db, results) = extract_js_project(repo);
        let files = results.len();
        let toplevels = table_count(&db, "js_toplevels");
        let funcs = table_count(&db, "js_functions");
        let classes = table_count(&db, "js_classes");
        let stmts = table_count(&db, "js_stmts");
        let exprs = table_count(&db, "js_exprs");
        let imports = table_count(&db, "js_imports");
        let exports = table_count(&db, "js_exports");

        eprintln!("{:<12} {:>8} {:>5} {:>9} {:>6} {:>6} {:>7} {:>7} {:>7} {:>7}",
            repo, "JS", files, toplevels, funcs, classes, stmts, exprs, imports, exports);
    }

    // TypeScript projects
    for repo in &["zod"] {
        let (db, results) = extract_ts_project(repo);
        let files = results.len();
        let toplevels = table_count(&db, "js_toplevels");
        let funcs = table_count(&db, "js_functions");
        let classes = table_count(&db, "js_classes");
        let stmts = table_count(&db, "js_stmts");
        let exprs = table_count(&db, "js_exprs");
        let imports = table_count(&db, "js_imports");
        let exports = table_count(&db, "js_exports");

        eprintln!("{:<12} {:>8} {:>5} {:>9} {:>6} {:>6} {:>7} {:>7} {:>7} {:>7}",
            repo, "TS", files, toplevels, funcs, classes, stmts, exprs, imports, exports);
    }
}
