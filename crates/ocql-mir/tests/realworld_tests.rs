//! Real-world tests: compile QL queries and evaluate against extracted C databases.

use std::collections::HashSet;

use ocql_database::{Database, Value};
use ocql_engine::evaluate;
use ocql_extractor_common::Extractor;
use ocql_extractor_cpp::{CppExtractor, cpp_schema};
use ocql_mir::compile_ql_to_engine;

fn extract_c(filename: &str) -> Database {
    let path = format!(
        "{}/../ocql-engine/tests/fixtures/{}",
        env!("CARGO_MANIFEST_DIR"),
        filename
    );
    let source = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path, e));

    let schema = cpp_schema();
    let mut db = Database::from_schema(schema);
    let extractor = CppExtractor::c();
    let result = extractor.extract_source(&mut db, &path, &source);
    assert!(result.success, "extraction failed: {:?}", result.error);
    db
}

fn collect_string_vals(db: &Database, table: &str) -> HashSet<String> {
    db.scan(table).unwrap()
        .map(|t| match &t[0] {
            Value::String(s) => db.strings.resolve(*s).to_string(),
            other => format!("{:?}", other),
        })
        .collect()
}

fn eval_ql(ql: &str, db: &mut Database) {
    let mut program = compile_ql_to_engine(ql).expect("compile_ql_to_engine failed");
    program.resolve_strings(db);
    evaluate(&program, db).expect("evaluate failed");
}

// ============================================================
// Tests against real extracted C code
// ============================================================

#[test]
fn ql_find_functions() {
    let mut db = extract_c("callgraph.c");

    // functions(id, name, kind) — find all function names
    eval_ql(r#"
        predicate func_names(string name) {
            functions(_, name, _)
        }
    "#, &mut db);

    let names = collect_string_vals(&db, "func_names");
    assert!(names.contains("main"));
    assert!(names.contains("foo"));
    assert!(names.contains("bar"));
}

#[test]
fn ql_leaf_functions() {
    let mut db = extract_c("callgraph.c");

    // Find functions that have at least one parameter
    // params(func_id, index, name, param_type)
    eval_ql(r#"
        predicate has_param(string name) {
            functions(fid, name, _) and
            params(fid, _, _, _)
        }
    "#, &mut db);

    let result = collect_string_vals(&db, "has_param");
    assert!(result.contains("bar")); // bar(int x)
}

#[test]
fn ql_find_function_by_name() {
    let mut db = extract_c("callgraph.c");

    eval_ql(r#"
        predicate is_main(string name) {
            functions(_, name, _) and name = "main"
        }
    "#, &mut db);

    let result = collect_string_vals(&db, "is_main");
    assert_eq!(result.len(), 1);
    assert!(result.contains("main"));
}

#[test]
fn ql_functions_without_params() {
    let mut db = extract_c("callgraph.c");

    // Functions without parameters (negation test)
    eval_ql(r#"
        predicate has_param_id(int fid) {
            params(fid, _, _, _)
        }
        predicate no_params(string name) {
            functions(fid, name, _) and not has_param_id(fid)
        }
    "#, &mut db);

    let result = collect_string_vals(&db, "no_params");
    // main() and foo() have no params in callgraph.c; bar(int x) does
    assert!(!result.contains("bar"));
}

#[test]
fn ql_class_over_functions() {
    let mut db = extract_c("callgraph.c");

    // Use a class to model "short-named functions"
    eval_ql(r#"
        predicate func_names(string name) {
            functions(_, name, _)
        }
        class ShortName extends string {
            ShortName() { func_names(this) and this != "main" }
        }
    "#, &mut db);

    let result = collect_string_vals(&db, "ShortName#char");
    assert!(result.contains("foo"));
    assert!(result.contains("bar"));
    assert!(!result.contains("main"));
}
