//! Integration tests for MIR lowering: compile QL → MIR → engine rules → evaluate.

use std::collections::HashSet;

use ocql_database::{Database, Value};
use ocql_engine::{evaluate, parse_program};
use ocql_mir::{compile_ql, compile_ql_to_engine, print_mir};
use ocql_mir::nodes::*;

// ============================================================
// Helpers
// ============================================================

fn eval_ql_with_db(ql_source: &str, db: &mut Database) {
    let mut program = compile_ql_to_engine(ql_source).expect("compile_ql_to_engine failed");
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
// Tests: MIR structure
// ============================================================

#[test]
fn compile_simple_predicate_to_mir() {
    let mir = compile_ql("predicate p(int x) { x > 0 }").unwrap();
    assert_eq!(mir.predicates.len(), 1);
    assert_eq!(mir.predicates[0].name, "p");
    assert_eq!(mir.predicates[0].params.len(), 1);
    assert_eq!(mir.predicates[0].params[0].name, "x");
    assert_eq!(mir.predicates[0].params[0].ty, MirType::Int);
}

#[test]
fn compile_predicate_with_result() {
    let mir = compile_ql("int double(int x) { result = x * 2 }").unwrap();
    assert_eq!(mir.predicates.len(), 1);
    assert_eq!(mir.predicates[0].name, "double");
    // params: x, result
    assert_eq!(mir.predicates[0].params.len(), 2);
    assert_eq!(mir.predicates[0].params[0].name, "x");
    assert_eq!(mir.predicates[0].params[1].name, "result");
}

#[test]
fn compile_class_to_mir() {
    let mir = compile_ql(r#"
        class SmallInt extends int {
            SmallInt() { this >= 1 and this <= 9 }
            int double() { result = this * 2 }
        }
    "#).unwrap();

    let names: Vec<&str> = mir.predicate_names();
    assert!(names.contains(&"SmallInt#char"));
    assert!(names.contains(&"SmallInt#double"));
}

#[test]
fn compile_select_to_mir() {
    let mir = compile_ql(r#"
        from int x
        where x > 0
        select x
    "#).unwrap();

    assert!(!mir.predicates.is_empty());
    assert!(mir.predicates[0].name.starts_with("select_result"));
}

#[test]
fn mir_sexpr_round_trip() {
    let mir = compile_ql("predicate p(int x) { x > 0 }").unwrap();
    let text = print_mir(&mir);
    let parsed = ocql_mir::parse_mir(&text).unwrap();
    assert_eq!(parsed.predicates.len(), 1);
    assert_eq!(parsed.predicates[0].name, "p");
}

// ============================================================
// Tests: Predicate lowering + evaluation
// ============================================================

#[test]
fn predicate_with_comparison() {
    let mut db = Database::empty();
    let mut seed = parse_program("val(1). val(5). val(10). val(20).").unwrap();
    seed.resolve_strings(&mut db);
    evaluate(&seed, &mut db).unwrap();

    eval_ql_with_db("predicate small(int x) { val(x) and x < 10 }", &mut db);

    let smalls = collect_ints(&db, "small");
    assert!(smalls.contains(&1));
    assert!(smalls.contains(&5));
    assert!(!smalls.contains(&10));
    assert!(!smalls.contains(&20));
}

#[test]
fn predicate_with_result_type() {
    let mut db = Database::empty();
    let mut seed = parse_program("val(3). val(7).").unwrap();
    seed.resolve_strings(&mut db);
    evaluate(&seed, &mut db).unwrap();

    eval_ql_with_db("int doubled(int x) { val(x) and result = x * 2 }", &mut db);

    let pairs = collect_int_pairs(&db, "doubled");
    assert!(pairs.contains(&(3, 6)));
    assert!(pairs.contains(&(7, 14)));
}

#[test]
fn multiple_predicates_recursive() {
    let mut db = Database::empty();
    let mut seed = parse_program("edge(1, 2). edge(2, 3). edge(3, 4).").unwrap();
    seed.resolve_strings(&mut db);
    evaluate(&seed, &mut db).unwrap();

    eval_ql_with_db(r#"
        predicate path(int a, int b) {
            edge(a, b)
            or
            exists(int mid | edge(a, mid) and path(mid, b))
        }
    "#, &mut db);

    let paths = collect_int_pairs(&db, "path");
    assert!(paths.contains(&(1, 2)));
    assert!(paths.contains(&(1, 3)));
    assert!(paths.contains(&(1, 4)));
    assert!(paths.contains(&(2, 3)));
    assert!(paths.contains(&(2, 4)));
    assert!(paths.contains(&(3, 4)));
}

#[test]
fn predicate_with_negation() {
    let mut db = Database::empty();
    let mut seed = parse_program("node(1). node(2). node(3). sink(2).").unwrap();
    seed.resolve_strings(&mut db);
    evaluate(&seed, &mut db).unwrap();

    eval_ql_with_db("predicate nonsink(int x) { node(x) and not sink(x) }", &mut db);

    let result = collect_ints(&db, "nonsink");
    assert!(result.contains(&1));
    assert!(!result.contains(&2));
    assert!(result.contains(&3));
}

// ============================================================
// Tests: Select queries
// ============================================================

#[test]
fn simple_select() {
    let mut db = Database::empty();
    let mut seed = parse_program("val(1). val(5). val(10).").unwrap();
    seed.resolve_strings(&mut db);
    evaluate(&seed, &mut db).unwrap();

    eval_ql_with_db(r#"
        from int x where val(x) and x > 3 select x
    "#, &mut db);

    // The select_result predicate should exist
    let names: Vec<String> = db.relation_names().map(|s| s.to_string()).collect();
    let select_rel = names.iter().find(|n| n.starts_with("select_result")).unwrap();
    let results = collect_ints(&db, select_rel);
    assert!(results.contains(&5));
    assert!(results.contains(&10));
    assert!(!results.contains(&1));
}

// ============================================================
// Tests: Class lowering + evaluation
// ============================================================

#[test]
fn class_characteristic_predicate() {
    let mut db = Database::empty();
    let mut seed = parse_program("val(1). val(5). val(10). val(20).").unwrap();
    seed.resolve_strings(&mut db);
    evaluate(&seed, &mut db).unwrap();

    eval_ql_with_db(r#"
        class SmallNum extends int {
            SmallNum() { val(this) and this < 10 }
        }
    "#, &mut db);

    let smalls = collect_ints(&db, "SmallNum#char");
    assert!(smalls.contains(&1));
    assert!(smalls.contains(&5));
    assert!(!smalls.contains(&10));
}

#[test]
fn class_with_member_predicate() {
    let mut db = Database::empty();
    let mut seed = parse_program("val(1). val(3). val(5).").unwrap();
    seed.resolve_strings(&mut db);
    evaluate(&seed, &mut db).unwrap();

    eval_ql_with_db(r#"
        class SmallNum extends int {
            SmallNum() { val(this) and this < 10 }
            int doubled() { result = this * 2 }
        }
    "#, &mut db);

    let pairs = collect_int_pairs(&db, "SmallNum#doubled");
    assert!(pairs.contains(&(1, 2)));
    assert!(pairs.contains(&(3, 6)));
    assert!(pairs.contains(&(5, 10)));
}

// ============================================================
// Tests: Formula variants
// ============================================================

#[test]
fn disjunction_lowering() {
    let mut db = Database::empty();
    let mut seed = parse_program("a(1). a(2). b(3). b(4).").unwrap();
    seed.resolve_strings(&mut db);
    evaluate(&seed, &mut db).unwrap();

    eval_ql_with_db("predicate ab(int x) { a(x) or b(x) }", &mut db);

    let result = collect_ints(&db, "ab");
    assert_eq!(result.len(), 4);
    assert!(result.contains(&1));
    assert!(result.contains(&4));
}

#[test]
fn exists_quantifier() {
    let mut db = Database::empty();
    let mut seed = parse_program("edge(1, 2). edge(2, 3). node(1). node(2). node(3). node(4).").unwrap();
    seed.resolve_strings(&mut db);
    evaluate(&seed, &mut db).unwrap();

    eval_ql_with_db(r#"
        predicate hasSucc(int x) { node(x) and exists(int y | edge(x, y)) }
    "#, &mut db);

    let result = collect_ints(&db, "hasSucc");
    assert!(result.contains(&1));
    assert!(result.contains(&2));
    assert!(!result.contains(&3));
    assert!(!result.contains(&4));
}

#[test]
fn implies_lowering() {
    let mut db = Database::empty();
    let mut seed = parse_program("val(1). val(5). val(10). big(10).").unwrap();
    seed.resolve_strings(&mut db);
    evaluate(&seed, &mut db).unwrap();

    // x is "safe" if big(x) implies val(x)
    // For x=10: big(10) is true, val(10) is true → safe
    // For x not big: big(x) is false → implies is vacuously true
    eval_ql_with_db(r#"
        predicate safe(int x) { val(x) and (big(x) implies val(x)) }
    "#, &mut db);

    let result = collect_ints(&db, "safe");
    assert!(result.contains(&1));
    assert!(result.contains(&5));
    assert!(result.contains(&10));
}

#[test]
fn if_then_else_lowering() {
    let mut db = Database::empty();
    let mut seed = parse_program("val(1). val(5). val(10). val(20).").unwrap();
    seed.resolve_strings(&mut db);
    evaluate(&seed, &mut db).unwrap();

    eval_ql_with_db(r#"
        predicate labeled(int x, int label) {
            val(x) and (if x > 9 then label = 100 else label = 0)
        }
    "#, &mut db);

    let pairs = collect_int_pairs(&db, "labeled");
    assert!(pairs.contains(&(1, 0)));
    assert!(pairs.contains(&(5, 0)));
    assert!(pairs.contains(&(10, 100)));
    assert!(pairs.contains(&(20, 100)));
}

// ============================================================
// Tests: Arithmetic expressions
// ============================================================

#[test]
fn arithmetic_expression() {
    let mut db = Database::empty();
    let mut seed = parse_program("val(3). val(7).").unwrap();
    seed.resolve_strings(&mut db);
    evaluate(&seed, &mut db).unwrap();

    eval_ql_with_db(r#"
        int tripled(int x) { val(x) and result = x * 3 }
    "#, &mut db);

    let pairs = collect_int_pairs(&db, "tripled");
    assert!(pairs.contains(&(3, 9)));
    assert!(pairs.contains(&(7, 21)));
}

// ============================================================
// Tests: Predicate calls with result
// ============================================================

#[test]
fn predicate_call_with_result_in_expr() {
    let mut db = Database::empty();
    let mut seed = parse_program("val(3). val(7).").unwrap();
    seed.resolve_strings(&mut db);
    evaluate(&seed, &mut db).unwrap();

    eval_ql_with_db(r#"
        int double(int x) { val(x) and result = x * 2 }
        predicate bigDouble(int x) { double(x) > 10 }
    "#, &mut db);

    let result = collect_ints(&db, "bigDouble");
    assert!(!result.contains(&3)); // double(3) = 6, not > 10
    assert!(result.contains(&7));  // double(7) = 14, > 10
}

// ============================================================
// Tests: String literals
// ============================================================

#[test]
fn string_literal_in_predicate() {
    let mut db = Database::empty();
    let mut seed = parse_program(r#"msg(1, "hello"). msg(2, "world"). msg(3, "hello")."#).unwrap();
    seed.resolve_strings(&mut db);
    evaluate(&seed, &mut db).unwrap();

    eval_ql_with_db(r#"
        predicate helloMsg(int id) { msg(id, "hello") }
    "#, &mut db);

    let result = collect_ints(&db, "helloMsg");
    assert!(result.contains(&1));
    assert!(!result.contains(&2));
    assert!(result.contains(&3));
}

// ============================================================
// Tests: Don't-care wildcards
// ============================================================

#[test]
fn dont_care_wildcards() {
    let mut db = Database::empty();
    let mut seed = parse_program("rel(1, 10, 100). rel(2, 20, 200). rel(3, 30, 300).").unwrap();
    seed.resolve_strings(&mut db);
    evaluate(&seed, &mut db).unwrap();

    eval_ql_with_db(r#"
        predicate first(int x) { rel(x, _, _) }
    "#, &mut db);

    let result = collect_ints(&db, "first");
    assert_eq!(result.len(), 3);
    assert!(result.contains(&1));
    assert!(result.contains(&2));
    assert!(result.contains(&3));
}

// ============================================================
// Tests: Conjunction chains
// ============================================================

#[test]
fn conjunction_chain() {
    let mut db = Database::empty();
    let mut seed = parse_program("val(1). val(2). val(3). val(4). val(5).").unwrap();
    seed.resolve_strings(&mut db);
    evaluate(&seed, &mut db).unwrap();

    eval_ql_with_db(r#"
        predicate mid(int x) { val(x) and x > 1 and x < 5 }
    "#, &mut db);

    let result = collect_ints(&db, "mid");
    assert_eq!(result, HashSet::from([2, 3, 4]));
}

// ============================================================
// Tests: Empty result
// ============================================================

#[test]
fn empty_result() {
    let mut db = Database::empty();
    let mut seed = parse_program("val(1). val(2).").unwrap();
    seed.resolve_strings(&mut db);
    evaluate(&seed, &mut db).unwrap();

    eval_ql_with_db(r#"
        predicate impossible(int x) { val(x) and x > 100 }
    "#, &mut db);

    let result = collect_ints(&db, "impossible");
    assert!(result.is_empty());
}

// ============================================================
// Tests: Parse errors
// ============================================================

#[test]
fn type_union_or_semantics() {
    // Type union alias: `class Both = A or B;` should use OR semantics,
    // not AND. Both#char should contain entities from A OR B.
    let mir = compile_ql(r#"
        class Both = @stmt or @expr;
    "#).expect("compile failed");
    // Should produce 2 separate rules (one per variant), not 1 conjunctive rule
    let char_preds: Vec<_> = mir.predicates.iter()
        .filter(|p| p.name == "Both#char")
        .collect();
    assert_eq!(char_preds.len(), 2, "type union should produce 2 rules (one per OR variant)");
    // Each rule should have a conjunction of exactly 1 atom (the variant's #char)
    for pred in &char_preds {
        match &pred.body {
            MirBody::Conjunction(atoms) => {
                assert_eq!(atoms.len(), 1, "each union variant rule should have 1 atom");
            }
            _ => panic!("expected Conjunction body for union variant rule"),
        }
    }
}

#[test]
fn parse_error_reported() {
    let result = compile_ql("this is not valid QL !!!");
    assert!(result.is_err());
}
