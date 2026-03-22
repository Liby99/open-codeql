//! Integration tests for MIR lowering: compile QL → Datalog → evaluate.

use std::collections::HashSet;

use ocql_database::{Database, Value};
use ocql_engine::{evaluate, parse_program};
use ocql_mir::compile_ql;

// ============================================================
// Helpers
// ============================================================

fn eval_ql_with_db(ql_source: &str, db: &mut Database) {
    let mut program = compile_ql(ql_source).expect("compile_ql failed");
    program.resolve_strings(db);
    evaluate(&program, db).expect("evaluate failed");
}

fn collect_ints(db: &Database, table: &str) -> HashSet<i64> {
    db.scan(table).unwrap()
        .map(|t| match t[0] { Value::Int(v) => v, _ => panic!("expected int") })
        .collect()
}

fn collect_int_pairs(db: &Database, table: &str) -> HashSet<(i64, i64)> {
    db.scan(table).unwrap()
        .map(|t| {
            let a = match t[0] { Value::Int(v) => v, _ => panic!() };
            let b = match t[1] { Value::Int(v) => v, _ => panic!() };
            (a, b)
        })
        .collect()
}

// ============================================================
// Tests: Predicate definitions
// ============================================================

#[test]
fn simple_predicate_no_body() {
    // predicate p(int x) without body → no rules emitted
    let program = compile_ql("predicate p(int x) { x > 0 }").unwrap();
    assert!(!program.rules.is_empty());
    assert_eq!(program.rules[0].head.predicate, "p");
}

#[test]
fn predicate_with_comparison() {
    // Set up: inject some data via Datalog, then run QL predicate
    let mut db = Database::empty();

    // First populate with Datalog facts
    let mut datalog = parse_program(r#"
        nums(1). nums(5). nums(10). nums(15). nums(20).
    "#).unwrap();
    datalog.resolve_strings(&mut db);
    evaluate(&datalog, &mut db).unwrap();

    // Now compile and run QL predicate
    eval_ql_with_db(
        r#"predicate big(int x) { nums(x) and x > 10 }"#,
        &mut db,
    );

    let results = collect_ints(&db, "big");
    assert!(results.contains(&15));
    assert!(results.contains(&20));
    assert_eq!(results.len(), 2);
}

#[test]
fn predicate_with_result_type() {
    let mut db = Database::empty();

    let mut datalog = parse_program(r#"
        vals(1). vals(2). vals(3).
    "#).unwrap();
    datalog.resolve_strings(&mut db);
    evaluate(&datalog, &mut db).unwrap();

    // int doubleIt(int x) { vals(x) and result = x + x }
    // → doubleIt(x, result) :- vals(x), result = x + x.
    eval_ql_with_db(
        r#"int doubleIt(int x) { vals(x) and result = x + x }"#,
        &mut db,
    );

    let results = collect_int_pairs(&db, "doubleIt");
    assert!(results.contains(&(1, 2)));
    assert!(results.contains(&(2, 4)));
    assert!(results.contains(&(3, 6)));
    assert_eq!(results.len(), 3);
}

#[test]
fn multiple_predicates() {
    let mut db = Database::empty();

    let mut datalog = parse_program(r#"
        edge(1, 2). edge(2, 3). edge(3, 4).
    "#).unwrap();
    datalog.resolve_strings(&mut db);
    evaluate(&datalog, &mut db).unwrap();

    eval_ql_with_db(r#"
        predicate reachable(int x, int y) {
            edge(x, y)
            or
            exists(int z | reachable(x, z) and edge(z, y))
        }
    "#, &mut db);

    let results = collect_int_pairs(&db, "reachable");
    // Direct
    assert!(results.contains(&(1, 2)));
    assert!(results.contains(&(2, 3)));
    assert!(results.contains(&(3, 4)));
    // Transitive
    assert!(results.contains(&(1, 3)));
    assert!(results.contains(&(1, 4)));
    assert!(results.contains(&(2, 4)));
    assert_eq!(results.len(), 6);
}

#[test]
fn predicate_with_negation() {
    let mut db = Database::empty();

    let mut datalog = parse_program(r#"
        node(1). node(2). node(3). node(4).
        edge(1, 2). edge(2, 3).
    "#).unwrap();
    datalog.resolve_strings(&mut db);
    evaluate(&datalog, &mut db).unwrap();

    eval_ql_with_db(r#"
        predicate hasOutEdge(int x) { edge(x, _) }
        predicate sink(int x) { node(x) and not hasOutEdge(x) }
    "#, &mut db);

    let sinks = collect_ints(&db, "sink");
    assert!(sinks.contains(&3));
    assert!(sinks.contains(&4));
    assert_eq!(sinks.len(), 2);
}

// ============================================================
// Tests: Select queries
// ============================================================

#[test]
fn simple_select() {
    let mut db = Database::empty();

    let mut datalog = parse_program(r#"
        nums(1). nums(5). nums(10).
    "#).unwrap();
    datalog.resolve_strings(&mut db);
    evaluate(&datalog, &mut db).unwrap();

    eval_ql_with_db(r#"
        from int x
        where nums(x) and x > 3
        select x
    "#, &mut db);

    // Find the select_result table
    let tables: Vec<_> = db.relation_names().filter(|n| n.starts_with("select_result")).collect();
    assert_eq!(tables.len(), 1);

    let results = collect_ints(&db, &tables[0]);
    assert!(results.contains(&5));
    assert!(results.contains(&10));
    assert_eq!(results.len(), 2);
}

#[test]
fn select_multiple_vars() {
    let mut db = Database::empty();

    let mut datalog = parse_program(r#"
        edge(1, 2). edge(2, 3). edge(3, 4).
    "#).unwrap();
    datalog.resolve_strings(&mut db);
    evaluate(&datalog, &mut db).unwrap();

    eval_ql_with_db(r#"
        from int x, int y
        where edge(x, y) and y > 2
        select x, y
    "#, &mut db);

    let tables: Vec<_> = db.relation_names().filter(|n| n.starts_with("select_result")).collect();
    assert_eq!(tables.len(), 1);

    let results = collect_int_pairs(&db, &tables[0]);
    assert!(results.contains(&(2, 3)));
    assert!(results.contains(&(3, 4)));
    assert_eq!(results.len(), 2);
}

// ============================================================
// Tests: Class lowering
// ============================================================

#[test]
fn class_characteristic_predicate() {
    let mut db = Database::empty();

    let mut datalog = parse_program(r#"
        nums(1). nums(5). nums(10). nums(50). nums(100).
    "#).unwrap();
    datalog.resolve_strings(&mut db);
    evaluate(&datalog, &mut db).unwrap();

    eval_ql_with_db(r#"
        class SmallNum extends int {
            SmallNum() { nums(this) and this >= 1 and this <= 10 }
        }
    "#, &mut db);

    let results = collect_ints(&db, "SmallNum#char");
    assert!(results.contains(&1));
    assert!(results.contains(&5));
    assert!(results.contains(&10));
    assert_eq!(results.len(), 3);
}

#[test]
fn class_with_member_predicate() {
    let mut db = Database::empty();

    let mut datalog = parse_program(r#"
        nums(1). nums(2). nums(3).
    "#).unwrap();
    datalog.resolve_strings(&mut db);
    evaluate(&datalog, &mut db).unwrap();

    eval_ql_with_db(r#"
        class MyNum extends int {
            MyNum() { nums(this) }
            int doubled() { result = this * 2 }
        }
    "#, &mut db);

    let chars = collect_ints(&db, "MyNum#char");
    assert_eq!(chars.len(), 3);

    let doubled = collect_int_pairs(&db, "MyNum#doubled");
    assert!(doubled.contains(&(1, 2)));
    assert!(doubled.contains(&(2, 4)));
    assert!(doubled.contains(&(3, 6)));
}

// ============================================================
// Tests: Formula lowering
// ============================================================

#[test]
fn disjunction_lowering() {
    let mut db = Database::empty();

    let mut datalog = parse_program(r#"
        a(1). a(2). b(3). b(4).
    "#).unwrap();
    datalog.resolve_strings(&mut db);
    evaluate(&datalog, &mut db).unwrap();

    eval_ql_with_db(r#"
        predicate either(int x) { a(x) or b(x) }
    "#, &mut db);

    let results = collect_ints(&db, "either");
    assert_eq!(results, [1, 2, 3, 4].into_iter().collect());
}

#[test]
fn exists_quantifier() {
    let mut db = Database::empty();

    let mut datalog = parse_program(r#"
        edge(1, 2). edge(2, 3). edge(3, 4).
        node(1). node(2). node(3). node(4). node(5).
    "#).unwrap();
    datalog.resolve_strings(&mut db);
    evaluate(&datalog, &mut db).unwrap();

    eval_ql_with_db(r#"
        predicate hasSuccessor(int x) {
            node(x) and exists(int y | edge(x, y))
        }
    "#, &mut db);

    let results = collect_ints(&db, "hasSuccessor");
    assert!(results.contains(&1));
    assert!(results.contains(&2));
    assert!(results.contains(&3));
    assert_eq!(results.len(), 3);
}

#[test]
fn implies_lowering() {
    let mut db = Database::empty();

    let mut datalog = parse_program(r#"
        p(1). p(2). p(3).
        q(2). q(3).
    "#).unwrap();
    datalog.resolve_strings(&mut db);
    evaluate(&datalog, &mut db).unwrap();

    // r(x) if: p(x) implies q(x) ≡ not p(x) or q(x)
    eval_ql_with_db(r#"
        predicate r(int x) { p(x) implies q(x) }
    "#, &mut db);

    let results = collect_ints(&db, "r");
    // p(1) implies q(1): p(1) is true, q(1) is false → false. NOT in r.
    // p(2) implies q(2): both true → true. In r.
    // p(3) implies q(3): both true → true. In r.
    assert!(results.contains(&2));
    assert!(results.contains(&3));
    assert!(!results.contains(&1));
}

#[test]
fn if_then_else_lowering() {
    let mut db = Database::empty();

    let mut datalog = parse_program(r#"
        val(1). val(2). val(3). val(4).
        big(3). big(4).
    "#).unwrap();
    datalog.resolve_strings(&mut db);
    evaluate(&datalog, &mut db).unwrap();

    // classified(x, label): if big(x) then label = 100 else label = 0
    eval_ql_with_db(r#"
        predicate classified(int x, int label) {
            val(x) and (if big(x) then label = 100 else label = 0)
        }
    "#, &mut db);

    let results = collect_int_pairs(&db, "classified");
    assert!(results.contains(&(1, 0)));
    assert!(results.contains(&(2, 0)));
    assert!(results.contains(&(3, 100)));
    assert!(results.contains(&(4, 100)));
}

// ============================================================
// Tests: Expression lowering
// ============================================================

#[test]
fn arithmetic_expression() {
    let mut db = Database::empty();

    let mut datalog = parse_program(r#"
        vals(1). vals(2). vals(3).
    "#).unwrap();
    datalog.resolve_strings(&mut db);
    evaluate(&datalog, &mut db).unwrap();

    eval_ql_with_db(r#"
        predicate computed(int x, int y) { vals(x) and y = x * x + 1 }
    "#, &mut db);

    let results = collect_int_pairs(&db, "computed");
    assert!(results.contains(&(1, 2)));  // 1*1+1 = 2
    assert!(results.contains(&(2, 5)));  // 2*2+1 = 5
    assert!(results.contains(&(3, 10))); // 3*3+1 = 10
}

#[test]
fn predicate_call_with_result_in_expr() {
    let mut db = Database::empty();

    let mut datalog = parse_program(r#"
        vals(1). vals(2). vals(3).
    "#).unwrap();
    datalog.resolve_strings(&mut db);
    evaluate(&datalog, &mut db).unwrap();

    eval_ql_with_db(r#"
        int square(int x) { vals(x) and result = x * x }
        predicate big_square(int x, int s) {
            s = square(x) and s > 3
        }
    "#, &mut db);

    let results = collect_int_pairs(&db, "big_square");
    assert!(results.contains(&(2, 4)));
    assert!(results.contains(&(3, 9)));
    assert_eq!(results.len(), 2);
}

// ============================================================
// Tests: QL with existing Datalog facts (real-world pattern)
// ============================================================

#[test]
fn ql_over_datalog_facts() {
    // Simulate a database with extracted facts, then query with QL
    let mut db = Database::empty();

    let mut datalog = parse_program(r#"
        functions(1, "main", 0).
        functions(2, "helper", 0).
        functions(3, "init", 0).
        calls(1, 2).
        calls(1, 3).
        calls(2, 3).
    "#).unwrap();
    datalog.resolve_strings(&mut db);
    evaluate(&datalog, &mut db).unwrap();

    // QL query: find functions that call more than one other function
    eval_ql_with_db(r#"
        predicate calls_target(int caller, int target) {
            calls(caller, target)
        }
    "#, &mut db);

    let calls = collect_int_pairs(&db, "calls_target");
    assert!(calls.contains(&(1, 2)));
    assert!(calls.contains(&(1, 3)));
    assert!(calls.contains(&(2, 3)));
    assert_eq!(calls.len(), 3);
}

// ============================================================
// Tests: Compile errors
// ============================================================

#[test]
fn parse_error_reported() {
    let result = compile_ql("this is not valid ql %%%");
    assert!(result.is_err());
    match result {
        Err(ocql_mir::CompileError::Parse(_)) => {}
        other => panic!("expected parse error, got {:?}", other),
    }
}

// ============================================================
// Tests: Edge cases
// ============================================================

#[test]
fn dont_care_wildcards() {
    let mut db = Database::empty();

    let mut datalog = parse_program(r#"
        rel(1, 2, 3). rel(4, 5, 6). rel(7, 8, 9).
    "#).unwrap();
    datalog.resolve_strings(&mut db);
    evaluate(&datalog, &mut db).unwrap();

    eval_ql_with_db(r#"
        predicate first(int x) { rel(x, _, _) }
    "#, &mut db);

    let results = collect_ints(&db, "first");
    assert_eq!(results, [1, 4, 7].into_iter().collect());
}

#[test]
fn conjunction_chain() {
    let mut db = Database::empty();

    let mut datalog = parse_program(r#"
        a(1). a(2). a(3). a(4). a(5).
    "#).unwrap();
    datalog.resolve_strings(&mut db);
    evaluate(&datalog, &mut db).unwrap();

    eval_ql_with_db(r#"
        predicate filtered(int x) {
            a(x) and x > 1 and x < 5 and x != 3
        }
    "#, &mut db);

    let results = collect_ints(&db, "filtered");
    assert_eq!(results, [2, 4].into_iter().collect());
}

#[test]
fn empty_result() {
    let mut db = Database::empty();

    let mut datalog = parse_program(r#"
        a(1). a(2).
    "#).unwrap();
    datalog.resolve_strings(&mut db);
    evaluate(&datalog, &mut db).unwrap();

    eval_ql_with_db(r#"
        predicate impossible(int x) { a(x) and x > 100 }
    "#, &mut db);

    let results = collect_ints(&db, "impossible");
    assert!(results.is_empty());
}

#[test]
fn recursive_transitive_closure_via_ql() {
    let mut db = Database::empty();

    let mut datalog = parse_program(r#"
        edge(1, 2). edge(2, 3). edge(3, 4). edge(4, 5).
    "#).unwrap();
    datalog.resolve_strings(&mut db);
    evaluate(&datalog, &mut db).unwrap();

    eval_ql_with_db(r#"
        predicate path(int a, int b) {
            edge(a, b) or exists(int mid | path(a, mid) and edge(mid, b))
        }
    "#, &mut db);

    let results = collect_int_pairs(&db, "path");
    // All reachable pairs in a chain 1→2→3→4→5
    assert!(results.contains(&(1, 2)));
    assert!(results.contains(&(1, 5))); // transitive
    assert!(results.contains(&(2, 5)));
    assert!(results.contains(&(3, 5)));
    assert_eq!(results.len(), 10); // 4+3+2+1 = 10
}

#[test]
fn string_literal_in_predicate() {
    let mut db = Database::empty();

    let mut datalog = parse_program(r#"
        names(1, "alice").
        names(2, "bob").
        names(3, "charlie").
    "#).unwrap();
    datalog.resolve_strings(&mut db);
    evaluate(&datalog, &mut db).unwrap();

    eval_ql_with_db(r#"
        predicate isBob(int id) { names(id, "bob") }
    "#, &mut db);

    let results = collect_ints(&db, "isBob");
    assert_eq!(results, [2].into_iter().collect());
}
