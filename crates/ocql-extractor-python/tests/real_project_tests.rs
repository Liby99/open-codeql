//! Integration tests that extract real-world Python projects (git submodules).
//!
//! These tests validate that extraction succeeds on real code and produces
//! reasonable output. They're #[ignore]d by default since they require
//! the git submodules to be checked out:
//!
//!   git submodule update --init vendor/test-repos/*
//!
//! Run with:
//!   cargo test -p ocql-extractor-python --test real_project_tests -- --ignored

use std::path::Path;

use ocql_database::{Database, Value};
use ocql_extractor_common::Extractor;
use ocql_extractor_python::{PythonExtractor, python_schema};

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
    let schema = python_schema();
    let mut db = Database::from_schema(schema);
    let extractor = PythonExtractor::new();
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
// flask — pallets/flask
// Popular web framework
// ============================================================

#[test]
#[ignore]
fn flask_extraction_succeeds() {
    let (_, results) = extract_project("flask");
    assert!(results.len() >= 30, "flask should have >= 30 Python files");
    assert!(
        results.iter().all(|r| r.success),
        "All flask files should extract successfully"
    );
}

#[test]
#[ignore]
fn flask_core_classes() {
    let (db, _) = extract_project("flask");
    let names = column_strings(&db, "py_Classes", 1);
    assert!(names.len() >= 20, "flask should have >= 20 classes, got {}", names.len());
    assert!(names.contains(&"Flask".into()), "should have Flask class");
    assert!(names.contains(&"Blueprint".into()), "should have Blueprint class");
}

#[test]
#[ignore]
fn flask_functions() {
    let (db, _) = extract_project("flask");
    let names = column_strings(&db, "py_Functions", 1);
    assert!(names.len() >= 100, "flask should have >= 100 functions, got {}", names.len());
    // Common Flask function names
    assert!(names.iter().any(|n| n.contains("render") || n.contains("route")),
        "should have render/route related functions");
}

#[test]
#[ignore]
fn flask_modules() {
    let (db, _) = extract_project("flask");
    let names = column_strings(&db, "py_Modules", 2);
    assert!(names.len() >= 15, "flask should have >= 15 modules, got {}", names.len());
}

#[test]
#[ignore]
fn flask_imports() {
    let (db, _) = extract_project("flask");
    let import_count = table_count(&db, "py_imports");
    assert!(import_count >= 50, "flask should have >= 50 imports, got {}", import_count);
}

// ============================================================
// httpie — httpie/cli
// HTTP client
// ============================================================

#[test]
#[ignore]
fn httpie_extraction_succeeds() {
    let (_, results) = extract_project("httpie");
    assert!(results.len() >= 20, "httpie should have >= 20 Python files");
    assert!(
        results.iter().all(|r| r.success),
        "All httpie files should extract successfully"
    );
}

#[test]
#[ignore]
fn httpie_core_classes() {
    let (db, _) = extract_project("httpie");
    let names = column_strings(&db, "py_Classes", 1);
    assert!(names.len() >= 10, "httpie should have >= 10 classes, got {}", names.len());
    // HTTPie has many HTTP-related classes
    assert!(names.iter().any(|n| n.contains("HTTP") || n.contains("Http")),
        "should have HTTP-related classes");
}

#[test]
#[ignore]
fn httpie_functions() {
    let (db, _) = extract_project("httpie");
    let names = column_strings(&db, "py_Functions", 1);
    assert!(names.len() >= 50, "httpie should have >= 50 functions, got {}", names.len());
    assert!(names.contains(&"main".into()), "should have main function");
}

#[test]
#[ignore]
fn httpie_statements() {
    let (db, _) = extract_project("httpie");
    let stmt_count = table_count(&db, "py_stmts");
    assert!(stmt_count >= 500, "httpie should have >= 500 statements, got {}", stmt_count);
}

// ============================================================
// black — psf/black
// Code formatter
// ============================================================

#[test]
#[ignore]
fn black_extraction_succeeds() {
    let (_, results) = extract_project("black");
    assert!(results.len() >= 20, "black should have >= 20 Python files");
    assert!(
        results.iter().all(|r| r.success),
        "All black files should extract successfully"
    );
}

#[test]
#[ignore]
fn black_core_classes() {
    let (db, _) = extract_project("black");
    let names = column_strings(&db, "py_Classes", 1);
    assert!(names.len() >= 15, "black should have >= 15 classes, got {}", names.len());
    // Black has formatter-related classes
    assert!(names.iter().any(|n| n.contains("Format") || n.contains("Line") || n.contains("Node")),
        "should have formatting-related classes, got: {:?}", names);
}

#[test]
#[ignore]
fn black_functions() {
    let (db, _) = extract_project("black");
    let names = column_strings(&db, "py_Functions", 1);
    assert!(names.len() >= 80, "black should have >= 80 functions, got {}", names.len());
    assert!(names.iter().any(|n| n.contains("format")),
        "should have format-related functions");
}

#[test]
#[ignore]
fn black_expressions() {
    let (db, _) = extract_project("black");
    let expr_count = table_count(&db, "py_exprs");
    assert!(expr_count >= 1000, "black should have >= 1000 expressions, got {}", expr_count);
}

#[test]
#[ignore]
fn black_parameters() {
    let (db, _) = extract_project("black");
    let param_count = table_count(&db, "py_parameters");
    assert!(param_count >= 150, "black should have >= 150 parameters, got {}", param_count);
}

// ============================================================
// Cross-project summary (for manual inspection)
// ============================================================

#[test]
#[ignore]
fn all_projects_summary() {
    let repos = ["flask", "httpie", "black"];

    eprintln!("\n{:<12} {:>5} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}",
        "Project", "Files", "Mods", "Funcs", "Class", "Stmts", "Exprs", "Params", "Imprs");
    eprintln!("{}", "-".repeat(80));

    for repo in &repos {
        let (db, results) = extract_project(repo);
        let files = results.len();
        let mods = table_count(&db, "py_Modules");
        let funcs = table_count(&db, "py_Functions");
        let classes = table_count(&db, "py_Classes");
        let stmts = table_count(&db, "py_stmts");
        let exprs = table_count(&db, "py_exprs");
        let params = table_count(&db, "py_parameters");
        let imports = table_count(&db, "py_imports");

        eprintln!("{:<12} {:>5} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}",
            repo, files, mods, funcs, classes, stmts, exprs, params, imports);
    }
}
