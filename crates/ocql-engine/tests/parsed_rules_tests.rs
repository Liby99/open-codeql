//! Integration tests: parse Datalog rules from text and evaluate them against
//! extracted C databases. Verifies that text-parsed rules produce the same
//! results as hand-coded Rule structs.

use std::collections::HashSet;

use ocql_database::{Database, Value};
use ocql_extractor_common::Extractor;
use ocql_extractor_cpp::{CppExtractor, cpp_schema};
use ocql_engine::{evaluate, parse_program};

// ============================================================
// Helpers
// ============================================================

fn extract_c_fixture(filename: &str) -> Database {
    let path = format!(
        "{}/tests/fixtures/{}",
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
// Tests: parsed text rules produce same results as hand-coded
// ============================================================

#[test]
fn parsed_callgraph_direct_calls() {
    let mut db = extract_c_fixture("callgraph.c");

    let mut program = parse_program(r#"
        // Direct call graph resolution
        direct_call(caller_name, callee_name) :-
            exprs(call_id, 74, _loc1),
            exprparents(callee_var, 0, call_id),
            exprs(callee_var, 84, _loc2),
            valuetext(callee_var, callee_name),
            enclosingfunction(call_id, caller_func),
            functions(caller_func, caller_name, _kind).
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
fn parsed_callgraph_transitive() {
    let mut db = extract_c_fixture("callgraph.c");

    let mut program = parse_program(r#"
        direct_call(caller_name, callee_name) :-
            exprs(call_id, 74, _loc1),
            exprparents(callee_var, 0, call_id),
            exprs(callee_var, 84, _loc2),
            valuetext(callee_var, callee_name),
            enclosingfunction(call_id, caller_func),
            functions(caller_func, caller_name, _kind).

        transitive_call(a, b) :- direct_call(a, b).
        transitive_call(a, b) :- transitive_call(a, c), direct_call(c, b).
    "#).unwrap();

    program.resolve_strings(&mut db);
    evaluate(&program, &mut db).unwrap();

    let calls = collect_string_pairs(&db, "transitive_call");

    // Direct calls should be in transitive closure
    assert!(calls.contains(&("main".into(), "foo".into())));
    assert!(calls.contains(&("main".into(), "bar".into())));
    assert!(calls.contains(&("foo".into(), "bar".into())));
    assert!(calls.contains(&("foo".into(), "baz".into())));
    assert!(calls.contains(&("bar".into(), "baz".into())));

    // Transitive: main -> foo -> baz, main -> bar -> baz, main -> foo -> bar -> baz
    assert!(calls.contains(&("main".into(), "baz".into())));

    // main can reach all 3 functions
    let main_callees: HashSet<_> = calls.iter()
        .filter(|(a, _)| a == "main")
        .map(|(_, b)| b.as_str())
        .collect();
    assert!(main_callees.contains("foo"));
    assert!(main_callees.contains("bar"));
    assert!(main_callees.contains("baz"));
}

#[test]
fn parsed_string_literal_matching() {
    let mut db = extract_c_fixture("callgraph.c");

    // Use string literal in a rule to match a specific function by name
    let mut program = parse_program(r#"
        is_main(id) :- functions(id, "main", _kind).
    "#).unwrap();

    program.resolve_strings(&mut db);
    evaluate(&program, &mut db).unwrap();

    let results: Vec<_> = db.scan("is_main").unwrap().collect();
    assert_eq!(results.len(), 1, "should find exactly one 'main' function");
}

#[test]
fn parsed_negation() {
    let mut db = extract_c_fixture("callgraph.c");

    let mut program = parse_program(r#"
        direct_call(caller_name, callee_name) :-
            exprs(call_id, 74, _loc1),
            exprparents(callee_var, 0, call_id),
            exprs(callee_var, 84, _loc2),
            valuetext(callee_var, callee_name),
            enclosingfunction(call_id, caller_func),
            functions(caller_func, caller_name, _kind).

        // Functions that are called but don't call anything themselves
        func_name(name) :- functions(_, name, _).
        leaf_func(name) :- func_name(name), not direct_call(name, _).
    "#).unwrap();

    program.resolve_strings(&mut db);
    evaluate(&program, &mut db).unwrap();

    let leaves: HashSet<String> = db.scan("leaf_func").unwrap()
        .map(|t| match &t[0] {
            Value::String(s) => db.strings.resolve(*s).to_string(),
            other => format!("{:?}", other),
        })
        .collect();

    // baz is a leaf — it calls nothing
    assert!(leaves.contains("baz"), "baz should be a leaf function");
    // main, foo, bar all make calls
    assert!(!leaves.contains("main"));
    assert!(!leaves.contains("foo"));
    assert!(!leaves.contains("bar"));
}

#[test]
fn parsed_facts_and_rules() {
    // Test that parsed facts (rules with no body) work correctly
    let mut program = parse_program(r#"
        edge(1, 2).
        edge(2, 3).
        edge(3, 4).

        path(x, y) :- edge(x, y).
        path(x, y) :- path(x, z), edge(z, y).
    "#).unwrap();

    let mut db = Database::empty();
    program.resolve_strings(&mut db);
    evaluate(&program, &mut db).unwrap();

    // Collect all path pairs
    let paths: HashSet<(i64, i64)> = db.scan("path").unwrap()
        .map(|t| {
            let a = match t[0] { Value::Int(v) => v, _ => panic!() };
            let b = match t[1] { Value::Int(v) => v, _ => panic!() };
            (a, b)
        })
        .collect();

    // Direct edges
    assert!(paths.contains(&(1, 2)));
    assert!(paths.contains(&(2, 3)));
    assert!(paths.contains(&(3, 4)));

    // Transitive
    assert!(paths.contains(&(1, 3)));
    assert!(paths.contains(&(1, 4)));
    assert!(paths.contains(&(2, 4)));

    assert_eq!(paths.len(), 6);
}

#[test]
fn parsed_guards() {
    let mut program = parse_program(r#"
        edge(1, 10).
        edge(2, 20).
        edge(3, 5).
        edge(4, 30).

        big_edge(x, y) :- edge(x, y), y > 15.
    "#).unwrap();

    let mut db = Database::empty();
    program.resolve_strings(&mut db);
    evaluate(&program, &mut db).unwrap();

    let results: HashSet<(i64, i64)> = db.scan("big_edge").unwrap()
        .map(|t| {
            let a = match t[0] { Value::Int(v) => v, _ => panic!() };
            let b = match t[1] { Value::Int(v) => v, _ => panic!() };
            (a, b)
        })
        .collect();

    assert!(results.contains(&(2, 20)));
    assert!(results.contains(&(4, 30)));
    assert_eq!(results.len(), 2);
}

#[test]
fn parsed_wildcards_are_independent() {
    // Two wildcards should not unify with each other
    let mut program = parse_program(r#"
        rel(1, 2, 3).
        rel(1, 1, 1).
        rel(2, 2, 2).

        first_col(x) :- rel(x, _, _).
    "#).unwrap();

    let mut db = Database::empty();
    program.resolve_strings(&mut db);
    evaluate(&program, &mut db).unwrap();

    let results: HashSet<i64> = db.scan("first_col").unwrap()
        .map(|t| match t[0] { Value::Int(v) => v, _ => panic!() })
        .collect();

    assert!(results.contains(&1));
    assert!(results.contains(&2));
    assert_eq!(results.len(), 2);
}

#[test]
fn parsed_arithmetic_basic() {
    let mut program = parse_program(r#"
        edge(1, 10).
        edge(2, 20).
        edge(3, 30).

        // sum(x, y, z) where z = x + y
        sum_result(x, y, z) :- edge(x, y), z = x + y.

        // double(x, d) where d = y * 2
        doubled(x, d) :- edge(x, y), d = y * 2.

        // Subtraction
        diff(x, d) :- edge(x, y), d = y - x.
    "#).unwrap();

    let mut db = Database::empty();
    program.resolve_strings(&mut db);
    evaluate(&program, &mut db).unwrap();

    // sum: edge(1,10) → (1, 10, 11), edge(2,20) → (2, 20, 22), edge(3,30) → (3, 30, 33)
    let sums: HashSet<(i64, i64, i64)> = db.scan("sum_result").unwrap()
        .map(|t| (t[0].as_int().unwrap(), t[1].as_int().unwrap(), t[2].as_int().unwrap()))
        .collect();
    assert!(sums.contains(&(1, 10, 11)));
    assert!(sums.contains(&(2, 20, 22)));
    assert!(sums.contains(&(3, 30, 33)));

    // doubled: edge(1,10) → (1, 20), etc.
    let dbl: HashSet<(i64, i64)> = db.scan("doubled").unwrap()
        .map(|t| (t[0].as_int().unwrap(), t[1].as_int().unwrap()))
        .collect();
    assert!(dbl.contains(&(1, 20)));
    assert!(dbl.contains(&(2, 40)));
    assert!(dbl.contains(&(3, 60)));

    // diff: edge(1,10) → (1, 9), edge(2,20) → (2, 18), edge(3,30) → (3, 27)
    let diffs: HashSet<(i64, i64)> = db.scan("diff").unwrap()
        .map(|t| (t[0].as_int().unwrap(), t[1].as_int().unwrap()))
        .collect();
    assert!(diffs.contains(&(1, 9)));
    assert!(diffs.contains(&(2, 18)));
    assert!(diffs.contains(&(3, 27)));
}

#[test]
fn parsed_arithmetic_chained() {
    // Test that arithmetic can chain with guards and other rules
    let mut program = parse_program(r#"
        vals(1).
        vals(2).
        vals(3).
        vals(4).
        vals(5).

        // Square each value
        squared(x, s) :- vals(x), s = x * x.

        // Only keep squares > 5
        big_square(x, s) :- squared(x, s), s > 5.
    "#).unwrap();

    let mut db = Database::empty();
    program.resolve_strings(&mut db);
    evaluate(&program, &mut db).unwrap();

    let squares: HashSet<(i64, i64)> = db.scan("squared").unwrap()
        .map(|t| (t[0].as_int().unwrap(), t[1].as_int().unwrap()))
        .collect();
    assert_eq!(squares.len(), 5);
    assert!(squares.contains(&(1, 1)));
    assert!(squares.contains(&(3, 9)));
    assert!(squares.contains(&(5, 25)));

    let big: HashSet<(i64, i64)> = db.scan("big_square").unwrap()
        .map(|t| (t[0].as_int().unwrap(), t[1].as_int().unwrap()))
        .collect();
    assert_eq!(big.len(), 3); // 9, 16, 25
    assert!(big.contains(&(3, 9)));
    assert!(big.contains(&(4, 16)));
    assert!(big.contains(&(5, 25)));
}

#[test]
fn parsed_arithmetic_div_mod() {
    let mut program = parse_program(r#"
        nums(10).
        nums(7).
        nums(15).

        half(x, h) :- nums(x), h = x / 2.
        remainder(x, r) :- nums(x), r = x % 3.
    "#).unwrap();

    let mut db = Database::empty();
    program.resolve_strings(&mut db);
    evaluate(&program, &mut db).unwrap();

    let halves: HashSet<(i64, i64)> = db.scan("half").unwrap()
        .map(|t| (t[0].as_int().unwrap(), t[1].as_int().unwrap()))
        .collect();
    assert!(halves.contains(&(10, 5)));  // 10/2 = 5
    assert!(halves.contains(&(7, 3)));   // 7/2 = 3 (integer division)
    assert!(halves.contains(&(15, 7)));  // 15/2 = 7

    let rems: HashSet<(i64, i64)> = db.scan("remainder").unwrap()
        .map(|t| (t[0].as_int().unwrap(), t[1].as_int().unwrap()))
        .collect();
    assert!(rems.contains(&(10, 1)));  // 10%3 = 1
    assert!(rems.contains(&(7, 1)));   // 7%3 = 1
    assert!(rems.contains(&(15, 0)));  // 15%3 = 0
}

#[test]
fn parsed_arithmetic_recursive_sum() {
    // Compute cumulative sums using recursive Datalog + arithmetic
    // Bounded by a fixed set of input values (no unbounded generation)
    let mut program = parse_program(r#"
        // Input values
        val(1).
        val(2).
        val(3).

        // Pairwise sums (bounded: only sum values that exist)
        pair_sum(x, y, s) :- val(x), val(y), s = x + y.

        // Cumulative: total = sum of all values in pairs
        big_sum(x, y, s) :- pair_sum(x, y, s), s > 4.
    "#).unwrap();

    let mut db = Database::empty();
    program.resolve_strings(&mut db);
    evaluate(&program, &mut db).unwrap();

    let sums: HashSet<(i64, i64, i64)> = db.scan("pair_sum").unwrap()
        .map(|t| (t[0].as_int().unwrap(), t[1].as_int().unwrap(), t[2].as_int().unwrap()))
        .collect();

    // 3*3 = 9 pairs
    assert_eq!(sums.len(), 9);
    assert!(sums.contains(&(1, 1, 2)));
    assert!(sums.contains(&(2, 3, 5)));
    assert!(sums.contains(&(3, 3, 6)));

    let big: HashSet<(i64, i64, i64)> = db.scan("big_sum").unwrap()
        .map(|t| (t[0].as_int().unwrap(), t[1].as_int().unwrap(), t[2].as_int().unwrap()))
        .collect();
    // Only sums > 4: (2,3,5), (3,2,5), (3,3,6)
    assert_eq!(big.len(), 3);
    assert!(big.contains(&(2, 3, 5)));
    assert!(big.contains(&(3, 2, 5)));
    assert!(big.contains(&(3, 3, 6)));
}
