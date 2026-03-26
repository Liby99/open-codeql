//! Integration tests that extract real-world Swift projects (git submodules).
//!
//! These tests validate that extraction succeeds on real code and produces
//! reasonable output. They're #[ignore]d by default since they require
//! the git submodules to be checked out:
//!
//!   git submodule update --init vendor/test-repos/*
//!
//! Run with:
//!   cargo test -p ocql-extractor-swift --test real_project_tests -- --ignored

use std::path::Path;

use ocql_database::{Database, Value};
use ocql_extractor_common::Extractor;
use ocql_extractor_swift::{SwiftExtractor, swift_schema};

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
    let schema = swift_schema();
    let mut db = Database::from_schema(schema);
    let extractor = SwiftExtractor::new();
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
// Alamofire — Alamofire/Alamofire (30+ files)
// HTTP networking library — clean, well-structured Swift
// ============================================================

#[test]
#[ignore]
fn alamofire_extraction_succeeds() {
    let (_, results) = extract_project("Alamofire");
    assert!(results.len() >= 30, "Alamofire should have >= 30 .swift files");
    assert!(
        results.iter().all(|r| r.success),
        "All Alamofire files should extract successfully"
    );
}

#[test]
#[ignore]
fn alamofire_classes_and_structs() {
    let (db, _) = extract_project("Alamofire");
    let class_count = table_count(&db, "swift_classes");
    let struct_count = table_count(&db, "swift_structs");
    assert!(class_count >= 10, "Alamofire should have >= 10 classes, got {}", class_count);
    assert!(struct_count >= 10, "Alamofire should have >= 10 structs, got {}", struct_count);
}

#[test]
#[ignore]
fn alamofire_protocols() {
    let (db, _) = extract_project("Alamofire");
    let names = column_strings(&db, "swift_protocols", 1);
    assert!(names.len() >= 5, "Alamofire should have >= 5 protocols, got {}", names.len());
}

#[test]
#[ignore]
fn alamofire_functions_and_methods() {
    let (db, _) = extract_project("Alamofire");
    let func_count = table_count(&db, "swift_functions");
    assert!(func_count >= 50, "Alamofire should have >= 50 functions, got {}", func_count);
}

#[test]
#[ignore]
fn alamofire_properties() {
    let (db, _) = extract_project("Alamofire");
    let prop_count = table_count(&db, "swift_properties");
    assert!(prop_count >= 30, "Alamofire should have >= 30 properties, got {}", prop_count);
}

// ============================================================
// vapor — vapor/vapor (50+ files)
// Web framework — tests extraction at larger scale
// ============================================================

#[test]
#[ignore]
fn vapor_extraction_succeeds() {
    let (_, results) = extract_project("vapor");
    assert!(results.len() >= 50, "vapor should have >= 50 .swift files");
    let failures: Vec<_> = results.iter().filter(|r| !r.success).collect();
    assert!(
        failures.is_empty(),
        "vapor should have no extraction failures, got {} failures: {:?}",
        failures.len(),
        failures.iter().map(|f| &f.file_path).collect::<Vec<_>>()
    );
}

#[test]
#[ignore]
fn vapor_scale() {
    let (db, _) = extract_project("vapor");
    let class_count = table_count(&db, "swift_classes");
    let struct_count = table_count(&db, "swift_structs");
    let func_count = table_count(&db, "swift_functions");
    let protocol_count = table_count(&db, "swift_protocols");

    eprintln!("vapor: {} classes, {} structs, {} functions, {} protocols",
        class_count, struct_count, func_count, protocol_count);

    assert!(class_count >= 15, "vapor should have >= 15 classes, got {}", class_count);
    assert!(struct_count >= 20, "vapor should have >= 20 structs, got {}", struct_count);
    assert!(func_count >= 80, "vapor should have >= 80 functions, got {}", func_count);
    assert!(protocol_count >= 10, "vapor should have >= 10 protocols, got {}", protocol_count);
}

#[test]
#[ignore]
fn vapor_extensions() {
    let (db, _) = extract_project("vapor");
    let ext_count = table_count(&db, "swift_extensions");
    assert!(ext_count >= 10, "vapor should have >= 10 extensions, got {}", ext_count);
}

#[test]
#[ignore]
fn vapor_enums() {
    let (db, _) = extract_project("vapor");
    let enum_count = table_count(&db, "swift_enums");
    let case_count = table_count(&db, "swift_enum_cases");
    assert!(enum_count >= 5, "vapor should have >= 5 enums, got {}", enum_count);
    assert!(case_count >= 10, "vapor should have >= 10 enum cases, got {}", case_count);
}

// ============================================================
// Kingfisher — onevcat/Kingfisher (30+ files)
// Image loading library — tests complex generic patterns
// ============================================================

#[test]
#[ignore]
fn kingfisher_extraction_succeeds() {
    let (_, results) = extract_project("Kingfisher");
    assert!(results.len() >= 30, "Kingfisher should have >= 30 .swift files");
    assert!(
        results.iter().all(|r| r.success),
        "All Kingfisher files should extract successfully"
    );
}

#[test]
#[ignore]
fn kingfisher_types() {
    let (db, _) = extract_project("Kingfisher");
    let class_count = table_count(&db, "swift_classes");
    let struct_count = table_count(&db, "swift_structs");
    let protocol_count = table_count(&db, "swift_protocols");

    assert!(class_count >= 5, "Kingfisher should have >= 5 classes, got {}", class_count);
    assert!(struct_count >= 10, "Kingfisher should have >= 10 structs, got {}", struct_count);
    assert!(protocol_count >= 5, "Kingfisher should have >= 5 protocols, got {}", protocol_count);
}

#[test]
#[ignore]
fn kingfisher_generics() {
    let (db, _) = extract_project("Kingfisher");
    let generic_count = table_count(&db, "swift_generics");
    assert!(generic_count >= 5, "Kingfisher should have >= 5 generic constraints, got {}", generic_count);
}

#[test]
#[ignore]
fn kingfisher_extensions() {
    let (db, _) = extract_project("Kingfisher");
    let ext_count = table_count(&db, "swift_extensions");
    assert!(ext_count >= 10, "Kingfisher should have >= 10 extensions, got {}", ext_count);
}

#[test]
#[ignore]
fn kingfisher_locations_valid() {
    let (db, _) = extract_project("Kingfisher");
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
    let repos = ["Alamofire", "vapor", "Kingfisher"];

    eprintln!("\n{:<12} {:>5} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}",
        "Project", "Files", "Class", "Struct", "Proto", "Func", "Prop", "Enum", "Ext");
    eprintln!("{}", "-".repeat(80));

    for repo in &repos {
        let (db, results) = extract_project(repo);
        let files = results.len();
        let classes = table_count(&db, "swift_classes");
        let structs = table_count(&db, "swift_structs");
        let protocols = table_count(&db, "swift_protocols");
        let funcs = table_count(&db, "swift_functions");
        let props = table_count(&db, "swift_properties");
        let enums = table_count(&db, "swift_enums");
        let exts = table_count(&db, "swift_extensions");

        eprintln!("{:<12} {:>5} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}",
            repo, files, classes, structs, protocols, funcs, props, enums, exts);
    }
}
