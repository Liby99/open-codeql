//! End-to-end tests: extract source → build database → compile QL → evaluate → check results.
//!
//! These tests exercise the full pipeline:
//!   source file → extractor → Database → QL parser → MIR → engine rules → evaluate → assert

use std::collections::HashSet;

use ocql_database::{Database, Value};
use ocql_engine::evaluate;
use ocql_extractor_common::Extractor;
use ocql_extractor_cpp::{CppExtractor, cpp_schema};
use ocql_extractor_java::{JavaExtractor, java_schema};
use ocql_mir::compile_ql_to_engine;

// ============================================================
// Helpers
// ============================================================

fn fixture_path(filename: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), filename)
}

fn extract_c(filename: &str) -> Database {
    let path = fixture_path(filename);
    let source = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path, e));
    let schema = cpp_schema();
    let mut db = Database::from_schema(schema);
    let extractor = CppExtractor::c();
    let result = extractor.extract_source(&mut db, &path, &source);
    assert!(result.success, "C extraction failed: {:?}", result.error);
    db
}

fn extract_java(filename: &str) -> Database {
    let path = fixture_path(filename);
    let source = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path, e));
    let schema = java_schema();
    let mut db = Database::from_schema(schema);
    let extractor = JavaExtractor;
    let result = extractor.extract_source(&mut db, &path, &source);
    assert!(result.success, "Java extraction failed: {:?}", result.error);
    db
}

fn eval_ql(ql: &str, db: &mut Database) {
    let mut program = compile_ql_to_engine(ql).expect("compile_ql_to_engine failed");
    program.resolve_strings(db);
    evaluate(&program, db).expect("evaluate failed");
}

/// Collect column 0 as strings from a result table.
fn collect_strings(db: &Database, table: &str) -> HashSet<String> {
    db.scan(table).unwrap()
        .map(|t| match &t[0] {
            Value::String(s) => db.strings.resolve(*s).to_string(),
            other => format!("{:?}", other),
        })
        .collect()
}

/// Collect column 0 as i64 from a result table.
#[allow(dead_code)]
fn collect_ints(db: &Database, table: &str) -> HashSet<i64> {
    db.scan(table).unwrap()
        .filter_map(|t| t[0].as_int())
        .collect()
}

/// Count rows in a result table.
fn count_rows(db: &Database, table: &str) -> usize {
    db.scan(table).map(|iter| iter.count()).unwrap_or(0)
}

// ============================================================
// C: basic.c — functions, params, local variables
// ============================================================

#[test]
fn c_find_all_functions() {
    let mut db = extract_c("basic.c");
    eval_ql(r#"
        predicate func_names(string name) {
            functions(_, name, _)
        }
    "#, &mut db);

    let names = collect_strings(&db, "func_names");
    assert!(names.contains("main"), "should find main, got: {:?}", names);
    assert!(names.contains("helper"), "should find helper, got: {:?}", names);
    assert!(names.contains("unused"), "should find unused, got: {:?}", names);
    assert_eq!(names.len(), 3);
}

#[test]
fn c_find_function_by_name() {
    let mut db = extract_c("basic.c");
    eval_ql(r#"
        predicate is_helper(string name) {
            functions(_, name, _) and name = "helper"
        }
    "#, &mut db);

    let result = collect_strings(&db, "is_helper");
    assert_eq!(result.len(), 1);
    assert!(result.contains("helper"));
}

#[test]
fn c_functions_with_params() {
    let mut db = extract_c("basic.c");
    eval_ql(r#"
        predicate has_params(string name) {
            functions(fid, name, _) and params(fid, _, _, _)
        }
    "#, &mut db);

    let result = collect_strings(&db, "has_params");
    assert!(result.contains("helper"), "helper(int x, int y) has params");
    assert!(!result.contains("main"), "main() has no params");
    assert!(!result.contains("unused"), "unused() has no params");
}

#[test]
fn c_negation_functions_without_params() {
    let mut db = extract_c("basic.c");
    eval_ql(r#"
        predicate has_param_id(int fid) {
            params(fid, _, _, _)
        }
        predicate no_params(string name) {
            functions(fid, name, _) and not has_param_id(fid)
        }
    "#, &mut db);

    let result = collect_strings(&db, "no_params");
    assert!(result.contains("main"));
    assert!(result.contains("unused"));
    assert!(!result.contains("helper"));
}

#[test]
fn c_param_names() {
    let mut db = extract_c("basic.c");
    eval_ql(r#"
        predicate param_names(string name) {
            params(_, _, name, _)
        }
    "#, &mut db);

    let result = collect_strings(&db, "param_names");
    assert!(result.contains("x"), "helper has param x");
    assert!(result.contains("y"), "helper has param y");
}

#[test]
fn c_local_variable_names() {
    let mut db = extract_c("basic.c");
    eval_ql(r#"
        predicate local_names(string name) {
            localvariables(_, name, _)
        }
    "#, &mut db);

    let result = collect_strings(&db, "local_names");
    assert!(result.contains("a"), "main has local a");
    assert!(result.contains("b"), "main has local b");
    assert!(result.contains("c"), "main has local c");
}

// ============================================================
// C: classes.c — structs, typedefs
// ============================================================

#[test]
fn c_find_struct_types() {
    let mut db = extract_c("classes.c");
    eval_ql(r#"
        predicate type_names(string name) {
            usertypes(_, name, _)
        }
    "#, &mut db);

    let result = collect_strings(&db, "type_names");
    assert!(result.contains("Point"), "should find Point struct, got: {:?}", result);
}

#[test]
fn c_struct_functions() {
    let mut db = extract_c("classes.c");
    eval_ql(r#"
        predicate func_names(string name) {
            functions(_, name, _)
        }
    "#, &mut db);

    let names = collect_strings(&db, "func_names");
    assert!(names.contains("make_point"));
    assert!(names.contains("distance"));
    assert!(names.contains("add_points"));
    assert!(names.contains("origin"));
    assert!(names.contains("main"));
}

// ============================================================
// C: security.c — dangerous function patterns
// ============================================================

#[test]
fn c_find_all_security_functions() {
    let mut db = extract_c("security.c");
    eval_ql(r#"
        predicate func_names(string name) {
            functions(_, name, _)
        }
    "#, &mut db);

    let names = collect_strings(&db, "func_names");
    assert!(names.contains("dangerous_gets"));
    assert!(names.contains("dangerous_strcpy"));
    assert!(names.contains("unchecked_malloc"));
    assert!(names.contains("format_string"));
    assert!(names.contains("safe_function"));
    assert!(names.contains("main"));
}

#[test]
fn c_find_dangerous_functions_by_name_pattern() {
    let mut db = extract_c("security.c");
    // Find functions whose name starts with "dangerous_"
    // Since we don't have string operations in our QL yet, use a join pattern:
    // find functions that take a char-pointer-like param (simplistic pattern)
    eval_ql(r#"
        predicate functions_with_string_param(string fname) {
            functions(fid, fname, _) and params(fid, _, _, "char *")
        }
    "#, &mut db);

    let result = collect_strings(&db, "functions_with_string_param");
    // dangerous_strcpy and format_string take char* params
    assert!(result.contains("dangerous_strcpy") || result.contains("format_string"),
        "should find functions with char* params, got: {:?}", result);
}

// ============================================================
// C: class as predicate (string domain)
// ============================================================

#[test]
fn c_class_over_function_names() {
    let mut db = extract_c("basic.c");
    eval_ql(r#"
        predicate func_names(string name) {
            functions(_, name, _)
        }
        class NonMain extends string {
            NonMain() { func_names(this) and this != "main" }
        }
    "#, &mut db);

    let result = collect_strings(&db, "NonMain#char");
    assert!(result.contains("helper"));
    assert!(result.contains("unused"));
    assert!(!result.contains("main"));
}

// ============================================================
// C: join across multiple tables
// ============================================================

#[test]
fn c_function_with_local_vars() {
    let mut db = extract_c("basic.c");
    // Join functions with their enclosing local variables
    eval_ql(r#"
        predicate func_locals(string fname, string vname) {
            functions(fid, fname, _) and
            enclosingfunction(vid, fid) and
            localvariables(vid, vname, _)
        }
    "#, &mut db);

    // Check we can find locals of main
    let rows: Vec<_> = db.scan("func_locals").unwrap()
        .map(|t| {
            let fname = match &t[0] {
                Value::String(s) => db.strings.resolve(*s).to_string(),
                other => format!("{:?}", other),
            };
            let vname = match &t[1] {
                Value::String(s) => db.strings.resolve(*s).to_string(),
                other => format!("{:?}", other),
            };
            (fname, vname)
        })
        .collect();

    let main_locals: HashSet<_> = rows.iter()
        .filter(|(f, _)| f == "main")
        .map(|(_, v)| v.as_str())
        .collect();
    assert!(main_locals.contains("a"), "main should have local 'a', got: {:?}", main_locals);
    assert!(main_locals.contains("b"), "main should have local 'b', got: {:?}", main_locals);
    assert!(main_locals.contains("c"), "main should have local 'c', got: {:?}", main_locals);
}

// ============================================================
// Java: Simple.java — classes, methods, constructors
// ============================================================

#[test]
fn java_find_classes() {
    let mut db = extract_java("Simple.java");
    eval_ql(r#"
        predicate class_names(string name) {
            classes_or_interfaces(_, name, _, _)
        }
    "#, &mut db);

    let names = collect_strings(&db, "class_names");
    assert!(names.contains("Simple"), "should find Simple class, got: {:?}", names);
}

#[test]
fn java_find_methods() {
    let mut db = extract_java("Simple.java");
    eval_ql(r#"
        predicate method_names(string name) {
            methods(_, name, _, _, _, _)
        }
    "#, &mut db);

    let names = collect_strings(&db, "method_names");
    assert!(names.contains("getValue"), "should find getValue, got: {:?}", names);
    assert!(names.contains("add"), "should find add, got: {:?}", names);
    assert!(names.contains("helper"), "should find helper, got: {:?}", names);
    assert!(names.contains("main"), "should find main, got: {:?}", names);
}

#[test]
fn java_find_constructors() {
    let mut db = extract_java("Simple.java");
    eval_ql(r#"
        predicate constr_names(string name) {
            constrs(_, name, _, _, _, _)
        }
    "#, &mut db);

    let names = collect_strings(&db, "constr_names");
    assert!(names.contains("Simple"), "should find Simple constructor, got: {:?}", names);
}

#[test]
fn java_find_fields() {
    let mut db = extract_java("Simple.java");
    eval_ql(r#"
        predicate field_names(string name) {
            fields(_, name, _, _)
        }
    "#, &mut db);

    let names = collect_strings(&db, "field_names");
    assert!(names.contains("value"), "should find 'value' field, got: {:?}", names);
}

#[test]
fn java_method_of_class() {
    let mut db = extract_java("Simple.java");
    // Join methods with their parent class
    eval_ql(r#"
        predicate class_method(string cname, string mname) {
            classes_or_interfaces(cid, cname, _, _) and
            methods(_, mname, _, _, cid, _)
        }
    "#, &mut db);

    let rows: Vec<_> = db.scan("class_method").unwrap()
        .map(|t| {
            let cname = match &t[0] {
                Value::String(s) => db.strings.resolve(*s).to_string(),
                other => format!("{:?}", other),
            };
            let mname = match &t[1] {
                Value::String(s) => db.strings.resolve(*s).to_string(),
                other => format!("{:?}", other),
            };
            (cname, mname)
        })
        .collect();

    let simple_methods: HashSet<_> = rows.iter()
        .filter(|(c, _)| c == "Simple")
        .map(|(_, m)| m.as_str())
        .collect();
    assert!(simple_methods.contains("getValue"));
    assert!(simple_methods.contains("add"));
    assert!(simple_methods.contains("helper"));
    assert!(simple_methods.contains("main"));
}

#[test]
fn java_param_names() {
    let mut db = extract_java("Simple.java");
    eval_ql(r#"
        predicate pnames(string name) {
            paramName(pid, name) and params(pid, _, _, _, _)
        }
    "#, &mut db);

    let names = collect_strings(&db, "pnames");
    // Constructor param "v", add param "x", helper params "a" and "b", main param "args"
    assert!(names.contains("v"), "should find param 'v', got: {:?}", names);
    assert!(names.contains("x"), "should find param 'x', got: {:?}", names);
    assert!(names.contains("a"), "should find param 'a', got: {:?}", names);
    assert!(names.contains("b"), "should find param 'b', got: {:?}", names);
}

// ============================================================
// Cross-cutting: counting, aggregation-like patterns
// ============================================================

#[test]
fn c_count_functions() {
    let mut db = extract_c("basic.c");
    eval_ql(r#"
        predicate all_funcs(int fid, string name) {
            functions(fid, name, _)
        }
    "#, &mut db);

    let n = count_rows(&db, "all_funcs");
    assert_eq!(n, 3, "basic.c has 3 functions: main, helper, unused");
}

#[test]
fn c_count_security_functions() {
    let mut db = extract_c("security.c");
    eval_ql(r#"
        predicate all_funcs(int fid, string name) {
            functions(fid, name, _)
        }
    "#, &mut db);

    let n = count_rows(&db, "all_funcs");
    assert_eq!(n, 6, "security.c has 6 functions");
}

// ============================================================
// Pipeline: QL compilation error handling
// ============================================================

#[test]
fn ql_compile_error_is_reported() {
    let result = compile_ql_to_engine(r#"
        predicate bad( {
            this is not valid QL
        }
    "#);
    assert!(result.is_err(), "invalid QL should produce compile error");
}

#[test]
fn ql_eval_nonexistent_table() {
    let mut db = extract_c("basic.c");
    // Query referencing a table that exists in the schema but has no data shouldn't crash
    eval_ql(r#"
        predicate glob_names(string name) {
            globalvariables(_, name, _)
        }
    "#, &mut db);

    let n = count_rows(&db, "glob_names");
    assert_eq!(n, 0, "basic.c has no global variables");
}
