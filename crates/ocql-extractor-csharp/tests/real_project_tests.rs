//! Integration tests that extract real-world C# projects (git submodules).
//!
//! These tests validate that extraction succeeds on real code and produces
//! reasonable output. They're #[ignore]d by default since they require
//! the git submodules to be checked out:
//!
//!   git submodule update --init vendor/test-repos/*
//!
//! Run with:
//!   cargo test -p ocql-extractor-csharp --test real_project_tests -- --ignored

use std::path::Path;

use ocql_database::{Database, Value};
use ocql_extractor_common::Extractor;
use ocql_extractor_csharp::{CSharpExtractor, csharp_schema};

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
    let schema = csharp_schema();
    let mut db = Database::from_schema(schema);
    let extractor = CSharpExtractor::new();
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
// Newtonsoft.Json — JamesNK/Newtonsoft.Json
// Popular JSON library for .NET
// ============================================================

#[test]
#[ignore]
fn newtonsoft_json_extraction_succeeds() {
    let (_, results) = extract_project("Newtonsoft.Json");
    assert!(results.len() >= 50, "Newtonsoft.Json should have >= 50 .cs files");
    let failures: Vec<_> = results.iter().filter(|r| !r.success).collect();
    assert!(
        failures.is_empty(),
        "All Newtonsoft.Json files should extract successfully, got {} failures: {:?}",
        failures.len(),
        failures.iter().map(|f| &f.file_path).collect::<Vec<_>>()
    );
}

#[test]
#[ignore]
fn newtonsoft_json_namespaces() {
    let (db, _) = extract_project("Newtonsoft.Json");
    let names = column_strings(&db, "csharp_namespaces", 1);
    assert!(names.len() >= 5, "Newtonsoft.Json should have >= 5 namespaces, got {}", names.len());
    assert!(
        names.iter().any(|n| n.contains("Newtonsoft")),
        "should have Newtonsoft.* namespace"
    );
}

#[test]
#[ignore]
fn newtonsoft_json_types() {
    let (db, _) = extract_project("Newtonsoft.Json");
    let type_count = table_count(&db, "csharp_types");
    let names = column_strings(&db, "csharp_types", 1);
    assert!(type_count >= 100, "Newtonsoft.Json should have >= 100 types, got {}", type_count);
    assert!(
        names.contains(&"JsonConvert".into()),
        "should have JsonConvert class"
    );
    assert!(
        names.contains(&"JsonSerializer".into()),
        "should have JsonSerializer class"
    );
}

#[test]
#[ignore]
fn newtonsoft_json_methods() {
    let (db, _) = extract_project("Newtonsoft.Json");
    let method_count = table_count(&db, "csharp_methods");
    assert!(
        method_count >= 500,
        "Newtonsoft.Json should have >= 500 methods, got {}",
        method_count
    );
}

#[test]
#[ignore]
fn newtonsoft_json_properties() {
    let (db, _) = extract_project("Newtonsoft.Json");
    let prop_count = table_count(&db, "csharp_properties");
    assert!(
        prop_count >= 100,
        "Newtonsoft.Json should have >= 100 properties, got {}",
        prop_count
    );
}

#[test]
#[ignore]
fn newtonsoft_json_fields() {
    let (db, _) = extract_project("Newtonsoft.Json");
    let field_count = table_count(&db, "csharp_fields");
    assert!(
        field_count >= 50,
        "Newtonsoft.Json should have >= 50 fields, got {}",
        field_count
    );
}

// ============================================================
// Humanizer — Humanizr/Humanizer
// String manipulation library
// ============================================================

#[test]
#[ignore]
fn humanizer_extraction_succeeds() {
    let (_, results) = extract_project("Humanizer");
    assert!(results.len() >= 50, "Humanizer should have >= 50 .cs files");
    let failures: Vec<_> = results.iter().filter(|r| !r.success).collect();
    assert!(
        failures.is_empty(),
        "All Humanizer files should extract successfully, got {} failures: {:?}",
        failures.len(),
        failures.iter().map(|f| &f.file_path).collect::<Vec<_>>()
    );
}

#[test]
#[ignore]
fn humanizer_namespaces() {
    let (db, _) = extract_project("Humanizer");
    let names = column_strings(&db, "csharp_namespaces", 1);
    assert!(names.len() >= 3, "Humanizer should have >= 3 namespaces, got {}", names.len());
    assert!(
        names.iter().any(|n| n.contains("Humanizer")),
        "should have Humanizer.* namespace"
    );
}

#[test]
#[ignore]
fn humanizer_types() {
    let (db, _) = extract_project("Humanizer");
    let type_count = table_count(&db, "csharp_types");
    let names = column_strings(&db, "csharp_types", 1);
    assert!(type_count >= 50, "Humanizer should have >= 50 types, got {}", type_count);
    // Check for common extension methods classes
    assert!(
        names.iter().any(|n| n.contains("Extensions")),
        "should have extension methods classes"
    );
}

#[test]
#[ignore]
fn humanizer_methods() {
    let (db, _) = extract_project("Humanizer");
    let method_count = table_count(&db, "csharp_methods");
    assert!(
        method_count >= 200,
        "Humanizer should have >= 200 methods, got {}",
        method_count
    );
}

#[test]
#[ignore]
fn humanizer_properties() {
    let (db, _) = extract_project("Humanizer");
    let prop_count = table_count(&db, "csharp_properties");
    assert!(
        prop_count >= 20,
        "Humanizer should have >= 20 properties, got {}",
        prop_count
    );
}

#[test]
#[ignore]
fn humanizer_scale() {
    let (db, _) = extract_project("Humanizer");
    let type_count = table_count(&db, "csharp_types");
    let method_count = table_count(&db, "csharp_methods");
    let prop_count = table_count(&db, "csharp_properties");
    let field_count = table_count(&db, "csharp_fields");

    eprintln!("Humanizer: {} types, {} methods, {} properties, {} fields",
        type_count, method_count, prop_count, field_count);

    assert!(type_count >= 50, "Humanizer should have >= 50 types, got {}", type_count);
    assert!(method_count >= 200, "Humanizer should have >= 200 methods, got {}", method_count);
}

// ============================================================
// Cross-project summary (for manual inspection)
// ============================================================

#[test]
#[ignore]
fn all_projects_summary() {
    let repos = ["Newtonsoft.Json", "Humanizer"];

    eprintln!("\n{:<18} {:>5} {:>6} {:>8} {:>8} {:>8} {:>6} {:>6}",
        "Project", "Files", "Types", "Methods", "Props", "Fields", "Params", "Vars");
    eprintln!("{}", "-".repeat(80));

    for repo in &repos {
        let (db, results) = extract_project(repo);
        let files = results.len();
        let types = table_count(&db, "csharp_types");
        let methods = table_count(&db, "csharp_methods");
        let props = table_count(&db, "csharp_properties");
        let fields = table_count(&db, "csharp_fields");
        let params = table_count(&db, "csharp_params");
        let vars = table_count(&db, "csharp_local_vars");

        eprintln!("{:<18} {:>5} {:>6} {:>8} {:>8} {:>8} {:>6} {:>6}",
            repo, files, types, methods, props, fields, params, vars);
    }
}
