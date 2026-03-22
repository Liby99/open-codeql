//! Real-world tests: compile QL queries and evaluate against extracted C databases.

use std::collections::HashSet;

use ocql_database::{Database, Value};
use ocql_engine::{evaluate, parse_program};
use ocql_extractor_common::Extractor;
use ocql_extractor_cpp::{CppExtractor, cpp_schema};
use ocql_mir::compile_ql;

fn extract_c(filename: &str) -> Database {
    // Fixtures live in the engine crate
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

fn collect_string_pairs(db: &Database, table: &str) -> HashSet<(String, String)> {
    db.scan(table).unwrap()
        .map(|t| {
            let a = match &t[0] {
                Value::String(s) => db.strings.resolve(*s).to_string(),
                other => format!("{:?}", other),
            };
            let b = match &t[1] {
                Value::String(s) => db.strings.resolve(*s).to_string(),
                other => format!("{:?}", other),
            };
            (a, b)
        })
        .collect()
}

// ============================================================
// Tests against real extracted C code
// ============================================================

#[test]
fn ql_find_functions() {
    let mut db = extract_c("callgraph.c");

    let mut program = compile_ql(r#"
        predicate func_names(string name) {
            functions(_, name, _)
        }
    "#).unwrap();
    program.resolve_strings(&mut db);
    evaluate(&program, &mut db).unwrap();

    let names = collect_string_vals(&db, "func_names");
    assert!(names.contains("main"));
    assert!(names.contains("foo"));
    assert!(names.contains("bar"));
    assert!(names.contains("baz"));
}

#[test]
fn ql_direct_call_graph() {
    let mut db = extract_c("callgraph.c");

    let mut program = compile_ql(r#"
        predicate direct_call(string caller_name, string callee_name) {
            exprs(call_id, 74, _) and
            exprparents(callee_var, 0, call_id) and
            exprs(callee_var, 84, _) and
            valuetext(callee_var, callee_name) and
            enclosingfunction(call_id, caller_func) and
            functions(caller_func, caller_name, _)
        }
    "#).unwrap();
    program.resolve_strings(&mut db);
    evaluate(&program, &mut db).unwrap();

    let calls = collect_string_pairs(&db, "direct_call");
    assert!(calls.contains(&("main".into(), "foo".into())));
    assert!(calls.contains(&("main".into(), "bar".into())));
    assert!(calls.contains(&("foo".into(), "bar".into())));
    assert!(calls.contains(&("foo".into(), "baz".into())));
    assert!(calls.contains(&("bar".into(), "baz".into())));
    assert_eq!(calls.len(), 5);
}

#[test]
fn ql_transitive_calls() {
    let mut db = extract_c("callgraph.c");

    let mut program = compile_ql(r#"
        predicate direct_call(string caller, string callee) {
            exprs(call_id, 74, _) and
            exprparents(callee_var, 0, call_id) and
            exprs(callee_var, 84, _) and
            valuetext(callee_var, callee) and
            enclosingfunction(call_id, caller_func) and
            functions(caller_func, caller, _)
        }

        predicate transitive_call(string a, string b) {
            direct_call(a, b)
            or
            exists(string mid | transitive_call(a, mid) and direct_call(mid, b))
        }
    "#).unwrap();
    program.resolve_strings(&mut db);
    evaluate(&program, &mut db).unwrap();

    let calls = collect_string_pairs(&db, "transitive_call");

    // Direct
    assert!(calls.contains(&("main".into(), "foo".into())));
    assert!(calls.contains(&("main".into(), "bar".into())));

    // Transitive: main → foo → baz, main → bar → baz
    assert!(calls.contains(&("main".into(), "baz".into())));

    // main reaches all 3 functions
    let main_callees: HashSet<_> = calls.iter()
        .filter(|(a, _)| a == "main")
        .map(|(_, b)| b.as_str())
        .collect();
    assert!(main_callees.contains("foo"));
    assert!(main_callees.contains("bar"));
    assert!(main_callees.contains("baz"));
}

#[test]
fn ql_leaf_functions() {
    let mut db = extract_c("callgraph.c");

    let mut program = compile_ql(r#"
        predicate direct_call(string caller, string callee) {
            exprs(call_id, 74, _) and
            exprparents(callee_var, 0, call_id) and
            exprs(callee_var, 84, _) and
            valuetext(callee_var, callee) and
            enclosingfunction(call_id, caller_func) and
            functions(caller_func, caller, _)
        }

        predicate func_name(string name) { functions(_, name, _) }

        predicate is_caller(string name) { direct_call(name, _) }

        predicate leaf_func(string name) {
            func_name(name) and not is_caller(name)
        }
    "#).unwrap();
    program.resolve_strings(&mut db);
    evaluate(&program, &mut db).unwrap();

    let leaves = collect_string_vals(&db, "leaf_func");
    assert!(leaves.contains("baz"), "baz should be a leaf function");
    assert!(!leaves.contains("main"), "main calls others");
    assert!(!leaves.contains("foo"), "foo calls others");
    assert!(!leaves.contains("bar"), "bar calls others");
}

#[test]
fn ql_count_params() {
    let mut db = extract_c("callgraph.c");

    // Count parameters per function using arithmetic
    let mut program = compile_ql(r#"
        predicate param_info(string func_name, int idx) {
            params(func_id, idx, _, _) and
            functions(func_id, func_name, _)
        }
    "#).unwrap();
    program.resolve_strings(&mut db);
    evaluate(&program, &mut db).unwrap();

    // Check that param_info captured parameter information
    let results: HashSet<(String, i64)> = db.scan("param_info").unwrap()
        .map(|t| {
            let name = match &t[0] {
                Value::String(s) => db.strings.resolve(*s).to_string(),
                _ => panic!(),
            };
            let idx = match t[1] { Value::Int(v) => v, _ => panic!() };
            (name, idx)
        })
        .collect();

    // In callgraph.c: baz(int x), bar(int x), foo(int x) — each has 1 param
    assert!(results.contains(&("foo".into(), 0)));
    assert!(results.contains(&("bar".into(), 0)));
    assert!(results.contains(&("baz".into(), 0)));
    // main() has no params
    assert!(!results.iter().any(|(name, _)| name == "main"));
}

#[test]
fn ql_find_function_by_name() {
    let mut db = extract_c("callgraph.c");

    let mut program = compile_ql(r#"
        predicate is_main(int id) { functions(id, "main", _) }
    "#).unwrap();
    program.resolve_strings(&mut db);
    evaluate(&program, &mut db).unwrap();

    let results: Vec<_> = db.scan("is_main").unwrap().collect();
    assert_eq!(results.len(), 1, "should find exactly one 'main' function");
}
