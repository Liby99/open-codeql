//! Integration tests that extract real-world Java projects (git submodules).
//!
//! These tests validate that extraction succeeds on real code and produces
//! reasonable output. They're #[ignore]d by default since they require
//! the git submodules to be checked out:
//!
//!   git submodule update --init vendor/test-repos/gson vendor/test-repos/jsoup \
//!     vendor/test-repos/GsonFactory vendor/test-repos/auto-value-gson \
//!     vendor/test-repos/auto-value-moshi
//!
//! Run with:
//!   cargo test -p ocql-extractor-java --test real_project_tests -- --ignored

use std::path::Path;

use ocql_database::{Database, Value};
use ocql_extractor_common::{Extractor, BuildSystem, ProjectExtractionResult};
use ocql_extractor_java::{JavaExtractor, java_schema};

fn extract_project(dir: &str) -> (Database, ProjectExtractionResult) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor/test-repos")
        .join(dir);
    if !path.is_dir() {
        panic!(
            "Test repo not found: {:?}\nRun: git submodule update --init vendor/test-repos/{}",
            path, dir
        );
    }
    let schema = java_schema();
    let mut db = Database::from_schema(schema);
    let extractor = JavaExtractor::new();
    let project = extractor.extract_project(&mut db, &path, true);
    (db, project)
}

fn extract_project_main_only(dir: &str) -> (Database, ProjectExtractionResult) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor/test-repos")
        .join(dir);
    if !path.is_dir() {
        panic!(
            "Test repo not found: {:?}\nRun: git submodule update --init vendor/test-repos/{}",
            path, dir
        );
    }
    let schema = java_schema();
    let mut db = Database::from_schema(schema);
    let extractor = JavaExtractor::new();
    let project = extractor.extract_project(&mut db, &path, false);
    (db, project)
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
// gson — google/gson (Maven, multi-module)
// ============================================================

#[test]
#[ignore]
fn gson_build_system_detected() {
    let (_, project) = extract_project("gson");
    assert_eq!(project.build_system, BuildSystem::Maven);
    let main_roots: Vec<_> = project.source_roots.iter()
        .filter(|r| !r.is_test)
        .collect();
    assert!(!main_roots.is_empty(), "Should find main source roots");
    eprintln!("gson source roots:");
    for root in &project.source_roots {
        let label = if root.is_test { "test" } else { "main" };
        eprintln!("  [{}] {} — {:?}", label, root.module, root.path);
    }
}

#[test]
#[ignore]
fn gson_extraction_succeeds() {
    let (_, project) = extract_project("gson");
    assert!(project.results.len() >= 200, "gson should have >= 200 Java files, got {}", project.results.len());
    let failures: Vec<_> = project.results.iter().filter(|r| !r.success).collect();
    assert!(
        failures.is_empty(),
        "gson should have no failures, got {}: {:?}",
        failures.len(),
        failures.iter().map(|f| &f.file_path).collect::<Vec<_>>()
    );
}

#[test]
#[ignore]
fn gson_classes() {
    let (db, _) = extract_project("gson");
    let names = column_strings(&db, "classes_or_interfaces", 1);
    assert!(names.contains(&"Gson".into()), "Should have Gson class");
    assert!(names.contains(&"JsonElement".into()), "Should have JsonElement class");
    assert!(names.contains(&"JsonObject".into()), "Should have JsonObject class");
}

#[test]
#[ignore]
fn gson_scale() {
    let (db, _) = extract_project("gson");
    let class_count = table_count(&db, "classes_or_interfaces");
    let method_count = table_count(&db, "methods");
    let field_count = table_count(&db, "fields");
    let stmt_count = table_count(&db, "stmts");
    let expr_count = table_count(&db, "exprs");

    eprintln!("gson: {} classes, {} methods, {} fields, {} stmts, {} exprs",
        class_count, method_count, field_count, stmt_count, expr_count);

    assert!(class_count >= 400, "gson should have >= 400 classes, got {}", class_count);
    assert!(method_count >= 2000, "gson should have >= 2000 methods, got {}", method_count);
    assert!(stmt_count >= 15000, "gson should have >= 15000 stmts, got {}", stmt_count);
}

#[test]
#[ignore]
fn gson_main_only_fewer_files() {
    let (_, all) = extract_project("gson");
    let (_, main_only) = extract_project_main_only("gson");
    eprintln!("gson: {} files (all) vs {} files (main only)",
        all.results.len(), main_only.results.len());
    assert!(
        main_only.results.len() < all.results.len(),
        "main-only should have fewer files than all"
    );
}

// ============================================================
// jsoup — jhy/jsoup (Maven, single module)
// ============================================================

#[test]
#[ignore]
fn jsoup_build_system_detected() {
    let (_, project) = extract_project("jsoup");
    assert_eq!(project.build_system, BuildSystem::Maven);
}

#[test]
#[ignore]
fn jsoup_extraction_succeeds() {
    let (_, project) = extract_project("jsoup");
    assert!(project.results.len() >= 150, "jsoup should have >= 150 Java files");
    let failures: Vec<_> = project.results.iter().filter(|r| !r.success).collect();
    assert!(
        failures.is_empty(),
        "jsoup should have no failures, got {}: {:?}",
        failures.len(),
        failures.iter().map(|f| &f.file_path).collect::<Vec<_>>()
    );
}

#[test]
#[ignore]
fn jsoup_classes() {
    let (db, _) = extract_project("jsoup");
    let names = column_strings(&db, "classes_or_interfaces", 1);
    assert!(names.contains(&"Jsoup".into()), "Should have Jsoup class");
    assert!(names.contains(&"Element".into()), "Should have Element class");
    assert!(names.contains(&"Document".into()), "Should have Document class");
}

#[test]
#[ignore]
fn jsoup_scale() {
    let (db, _) = extract_project("jsoup");
    let class_count = table_count(&db, "classes_or_interfaces");
    let method_count = table_count(&db, "methods");
    let field_count = table_count(&db, "fields");

    eprintln!("jsoup: {} classes, {} methods, {} fields",
        class_count, method_count, field_count);

    assert!(class_count >= 200, "jsoup should have >= 200 classes, got {}", class_count);
    assert!(method_count >= 2000, "jsoup should have >= 2000 methods, got {}", method_count);
}

// ============================================================
// GsonFactory — getActivity/GsonFactory (Gradle, multi-module, Android)
// ============================================================

#[test]
#[ignore]
fn gsonfactory_build_system_detected() {
    let (_, project) = extract_project("GsonFactory");
    assert_eq!(project.build_system, BuildSystem::Gradle);
    eprintln!("GsonFactory source roots:");
    for root in &project.source_roots {
        let label = if root.is_test { "test" } else { "main" };
        eprintln!("  [{}] {} — {:?}", label, root.module, root.path);
    }
}

#[test]
#[ignore]
fn gsonfactory_extraction_succeeds() {
    let (_, project) = extract_project("GsonFactory");
    assert!(project.results.len() >= 10, "GsonFactory should have >= 10 Java files, got {}", project.results.len());
    let failures: Vec<_> = project.results.iter().filter(|r| !r.success).collect();
    assert!(
        failures.is_empty(),
        "GsonFactory should have no failures, got {}: {:?}",
        failures.len(),
        failures.iter().map(|f| &f.file_path).collect::<Vec<_>>()
    );
}

#[test]
#[ignore]
fn gsonfactory_classes() {
    let (db, _) = extract_project("GsonFactory");
    let names = column_strings(&db, "classes_or_interfaces", 1);
    assert!(names.contains(&"GsonFactory".into()), "Should have GsonFactory class");
}

// ============================================================
// auto-value-gson — rharter/auto-value-gson (Gradle, multi-module)
// ============================================================

#[test]
#[ignore]
fn auto_value_gson_build_system_detected() {
    let (_, project) = extract_project("auto-value-gson");
    assert_eq!(project.build_system, BuildSystem::Gradle);
    eprintln!("auto-value-gson source roots:");
    for root in &project.source_roots {
        let label = if root.is_test { "test" } else { "main" };
        eprintln!("  [{}] {} — {:?}", label, root.module, root.path);
    }
}

#[test]
#[ignore]
fn auto_value_gson_extraction_succeeds() {
    let (_, project) = extract_project("auto-value-gson");
    assert!(project.results.len() >= 10, "auto-value-gson should have >= 10 Java files, got {}", project.results.len());
    let failures: Vec<_> = project.results.iter().filter(|r| !r.success).collect();
    assert!(
        failures.is_empty(),
        "auto-value-gson should have no failures, got {}: {:?}",
        failures.len(),
        failures.iter().map(|f| &f.file_path).collect::<Vec<_>>()
    );
}

// ============================================================
// auto-value-moshi — rharter/auto-value-moshi (Gradle, multi-module)
// ============================================================

#[test]
#[ignore]
fn auto_value_moshi_build_system_detected() {
    let (_, project) = extract_project("auto-value-moshi");
    assert_eq!(project.build_system, BuildSystem::Gradle);
    eprintln!("auto-value-moshi source roots:");
    for root in &project.source_roots {
        let label = if root.is_test { "test" } else { "main" };
        eprintln!("  [{}] {} — {:?}", label, root.module, root.path);
    }
}

#[test]
#[ignore]
fn auto_value_moshi_extraction_succeeds() {
    let (_, project) = extract_project("auto-value-moshi");
    assert!(project.results.len() >= 10, "auto-value-moshi should have >= 10 Java files, got {}", project.results.len());
    let failures: Vec<_> = project.results.iter().filter(|r| !r.success).collect();
    assert!(
        failures.is_empty(),
        "auto-value-moshi should have no failures, got {}: {:?}",
        failures.len(),
        failures.iter().map(|f| &f.file_path).collect::<Vec<_>>()
    );
}

// ============================================================
// Cross-project summary
// ============================================================

#[test]
#[ignore]
fn all_java_projects_summary() {
    let repos = ["gson", "jsoup", "GsonFactory", "auto-value-gson", "auto-value-moshi"];

    eprintln!("\n{:<20} {:>8} {:>5} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7}",
        "Project", "Build", "Files", "Classes", "Methods", "Constrs", "Fields", "Params", "Stmts", "Exprs");
    eprintln!("{}", "-".repeat(105));

    for repo in &repos {
        let (db, project) = extract_project(repo);
        let files = project.results.len();
        let build = format!("{:?}", project.build_system);
        let classes = table_count(&db, "classes_or_interfaces");
        let methods = table_count(&db, "methods");
        let constrs = table_count(&db, "constrs");
        let fields = table_count(&db, "fields");
        let params = table_count(&db, "params");
        let stmts = table_count(&db, "stmts");
        let exprs = table_count(&db, "exprs");

        eprintln!("{:<20} {:>8} {:>5} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7}",
            repo, build, files, classes, methods, constrs, fields, params, stmts, exprs);
    }
}
