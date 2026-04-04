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

/// Collect a specific column as strings from a result table.
fn collect_strings_col(db: &Database, table: &str, col: usize) -> HashSet<String> {
    db.scan(table).unwrap()
        .map(|t| match &t[col] {
            Value::String(s) => db.strings.resolve(*s).to_string(),
            other => format!("{:?}", other),
        })
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
// QL class system: class definitions + member calls
// ============================================================

/// Define a Function class over the `functions` table and use member methods.
/// This tests: class characteristic predicate, member predicates, `from Class var`,
/// and `var.method()` member call resolution.
#[test]
fn ql_class_function_with_methods() {
    let mut db = extract_c("basic.c");
    eval_ql(r#"
        class Function extends int {
            Function() { functions(this, _, _) }
            string getName() { functions(this, result, _) }
            int getKind() { functions(this, _, result) }
        }
        from Function f
        select f.getName()
    "#, &mut db);

    // Select produces [f, _sel0] where _sel0 = f.getName(). Column 1 has the names.
    let result = collect_strings_col(&db, "select_result_0", 1);
    assert!(result.contains("main"), "got: {:?}", result);
    assert!(result.contains("helper"), "got: {:?}", result);
    assert!(result.contains("unused"), "got: {:?}", result);
    assert_eq!(result.len(), 3);
}

/// Test member predicate used as formula (no result variable).
#[test]
fn ql_class_member_predicate_as_formula() {
    let mut db = extract_c("basic.c");
    eval_ql(r#"
        class Function extends int {
            Function() { functions(this, _, _) }
            string getName() { functions(this, result, _) }
            predicate hasParams() { params(this, _, _, _) }
        }
        predicate funcs_with_params(string name) {
            exists(Function f | f.hasParams() | name = f.getName())
        }
    "#, &mut db);

    let result = collect_strings(&db, "funcs_with_params");
    assert!(result.contains("helper"), "helper has params, got: {:?}", result);
    assert!(!result.contains("main"), "main has no params, got: {:?}", result);
}

/// Test `from Class var where ... select var.method()` end-to-end.
#[test]
fn ql_class_with_where_clause() {
    let mut db = extract_c("basic.c");
    eval_ql(r#"
        class Function extends int {
            Function() { functions(this, _, _) }
            string getName() { functions(this, result, _) }
        }
        from Function f
        where f.getName() = "main"
        select f.getName()
    "#, &mut db);

    let result = collect_strings_col(&db, "select_result_0", 1);
    assert_eq!(result.len(), 1);
    assert!(result.contains("main"));
}

/// Test class negation: functions without parameters using explicit predicate.
/// Uses predicate-based approach since class inheritance dispatch is a future feature.
#[test]
fn ql_class_negation_no_params() {
    let mut db = extract_c("basic.c");
    eval_ql(r#"
        class Function extends int {
            Function() { functions(this, _, _) }
            string getName() { functions(this, result, _) }
            predicate hasParams() { params(this, _, _, _) }
        }
        predicate paramless_func_names(string name) {
            exists(Function f |
                not f.hasParams() |
                name = f.getName()
            )
        }
    "#, &mut db);

    let result = collect_strings(&db, "paramless_func_names");
    assert!(result.contains("main"), "got: {:?}", result);
    assert!(result.contains("unused"), "got: {:?}", result);
    assert!(!result.contains("helper"), "helper has params, got: {:?}", result);
}

/// Test Java: class wrapping methods table.
#[test]
fn ql_java_method_class() {
    let mut db = extract_java("Simple.java");
    eval_ql(r#"
        class Method extends int {
            Method() { methods(this, _, _, _, _, _) }
            string getName() { methods(this, result, _, _, _, _) }
            string getReturnType() { methods(this, _, _, result, _, _) }
        }
        from Method m
        select m.getName()
    "#, &mut db);

    let result = collect_strings_col(&db, "select_result_0", 1);
    assert!(result.contains("getValue"), "got: {:?}", result);
    assert!(result.contains("add"), "got: {:?}", result);
    assert!(result.contains("helper"), "got: {:?}", result);
    assert!(result.contains("main"), "got: {:?}", result);
}

/// Multi-file merge test: define library predicates and use them.
/// This simulates what happens when you import library classes.
#[test]
fn ql_multi_file_merge() {
    let mut db = extract_c("basic.c");
    // Simulate a library defining Function + query using it, all in one string
    // (this is what multi-file compilation would produce)
    eval_ql(r#"
        class Function extends int {
            Function() { functions(this, _, _) }
            string getName() { functions(this, result, _) }
            predicate hasParams() { params(this, _, _, _) }
        }
        class Param extends int {
            Param() { params(_, _, _, _) and this = this }
            string getName() { params(_, _, result, _) and this = this }
        }
        predicate func_param_count(string fname) {
            exists(Function f | f.hasParams() | fname = f.getName())
        }
    "#, &mut db);

    let result = collect_strings(&db, "func_param_count");
    assert!(result.contains("helper"));
    assert_eq!(result.len(), 1);
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

// ============================================================
// Gap fix tests: dispatch, transitive closure, set literals
// ============================================================

#[test]
fn ql_dispatch_inherited_method() {
    let mut db = extract_c("basic.c");
    eval_ql(r#"
        class Function extends int {
            Function() { functions(this, _, _) }
            string getName() { functions(this, result, _) }
        }
        class SpecialFunction extends Function {
            SpecialFunction() { functions(this, "main", _) }
        }
        from SpecialFunction f
        select f.getName()
    "#, &mut db);
    let result = collect_strings_col(&db, "select_result_0", 1);
    assert!(result.contains("main"), "got: {:?}", result);
}

#[test]
fn ql_transitive_closure() {
    let mut db = extract_c("basic.c");
    eval_ql(r#"
        predicate edge(int a, int b) {
            a = 1 and b = 2 or
            a = 2 and b = 3 or
            a = 3 and b = 4
        }
        predicate reachable(int a, int b) {
            edge+(a, b)
        }
    "#, &mut db);
    // 1->2, 1->3, 1->4, 2->3, 2->4, 3->4 = 6 pairs
    let n = count_rows(&db, "reachable");
    assert!(n >= 6, "expected at least 6 reachable pairs, got {}", n);
}

#[test]
fn ql_set_literal_multiple() {
    let mut db = extract_c("basic.c");
    eval_ql(r#"
        predicate target_funcs(string name) {
            functions(_, name, _) and name = ["main", "helper"]
        }
    "#, &mut db);
    let result = collect_strings(&db, "target_funcs");
    assert!(result.contains("main"), "got: {:?}", result);
    assert!(result.contains("helper"), "got: {:?}", result);
}

// ============================================================
// Type unions and database type classes
// ============================================================

/// Type union alias with OR semantics: class Both = A or B produces the union.
#[test]
fn ql_type_union_or_semantics() {
    let mut db = extract_c("basic.c");
    eval_ql(r#"
        class Func extends int {
            Func() { functions(this, _, _) }
        }
        class Loc extends int {
            Loc() { localvariables(this, _, _) }
        }
        class FuncOrLocal = Func or Loc;
        predicate union_count(int x) {
            FuncOrLocal(x)
        }
    "#, &mut db);

    let func_count = count_rows(&db, "Func#char");
    let local_count = count_rows(&db, "Loc#char");
    let union_count = count_rows(&db, "FuncOrLocal#char");
    // Union should have at least as many entities as each individual type
    assert!(union_count >= func_count, "union {} >= funcs {}", union_count, func_count);
    assert!(union_count >= local_count, "union {} >= locals {}", union_count, local_count);
    // Union should have entities from both (assuming no overlap)
    assert!(union_count > 0, "union should have at least 1 entity");
}

/// Database type class: extending @function directly seeds from schema #char.
#[test]
fn ql_database_type_char() {
    let mut db = extract_c("basic.c");
    // The engine seeds @function#char from the functions table.
    // A class extending @function should inherit those entities.
    eval_ql(r#"
        class MyFunc extends @function {
            string getName() { functions(this, result, _) }
        }
    "#, &mut db);

    let char_count = count_rows(&db, "MyFunc#char");
    assert_eq!(char_count, 3, "basic.c has 3 functions, MyFunc#char should have 3, got {}", char_count);
}

// ============================================================
// Security: dangerous function call detection via expression tree
// ============================================================

/// Standalone security query: find calls to dangerous C functions by examining
/// call expressions and their callee names in the expression tree.
/// This validates the query logic that the full vendor security test uses.
#[test]
fn c_security_dangerous_call_detection() {
    let mut db = extract_c("security.c");
    eval_ql(r#"
        predicate isDangerousName(string name) {
            name = "gets" or
            name = "strcpy" or
            name = "sprintf" or
            name = "strcat"
        }

        predicate dangerousCall(int call_id, string callee_name) {
            exprs(call_id, 74, _) and
            exprparents(callee_id, 0, call_id) and
            valuetext(callee_id, callee_name) and
            isDangerousName(callee_name)
        }

        predicate dangerousFinding(string callee_name, string in_function) {
            dangerousCall(call_id, callee_name) and
            enclosingfunction(call_id, func_id) and
            functions(func_id, in_function, _)
        }
    "#, &mut db);

    let findings: Vec<(String, String)> = db.scan("dangerousFinding").unwrap()
        .map(|row| {
            let callee = match &row[0] {
                Value::String(s) => db.strings.resolve(*s).to_string(),
                _ => "?".to_string(),
            };
            let caller = match &row[1] {
                Value::String(s) => db.strings.resolve(*s).to_string(),
                _ => "?".to_string(),
            };
            (callee, caller)
        })
        .collect();

    eprintln!("Security findings: {:?}", findings);

    let callee_names: HashSet<String> = findings.iter().map(|(c, _)| c.clone()).collect();
    assert!(callee_names.contains("gets"), "Should find gets() call, got: {:?}", findings);
    assert!(callee_names.contains("strcpy"), "Should find strcpy() call, got: {:?}", findings);

    // Verify context: gets is called from dangerous_gets, strcpy from dangerous_strcpy
    assert!(findings.contains(&("gets".to_string(), "dangerous_gets".to_string())),
        "gets() should be in dangerous_gets(), got: {:?}", findings);
    assert!(findings.contains(&("strcpy".to_string(), "dangerous_strcpy".to_string())),
        "strcpy() should be in dangerous_strcpy(), got: {:?}", findings);
}
