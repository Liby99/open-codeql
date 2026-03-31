//! Integration tests: QL source → MIR → LIR.

use ocql_lir::{lower_mir, pretty_print, LirPlan};
use ocql_mir::compile_ql;

fn ql_to_lir(source: &str) -> ocql_lir::LirProgram {
    let mir = compile_ql(source).expect("MIR compilation failed");
    lower_mir(&mir).expect("LIR lowering failed")
}

#[test]
fn simple_predicate() {
    let lir = ql_to_lir("predicate p(int x) { x > 0 }");
    assert_eq!(lir.strata.len(), 1);
    assert_eq!(lir.rule_count(), 1);

    let pp = pretty_print(&lir);
    assert!(pp.contains("non-recursive"));
    assert!(pp.contains("p("));
}

#[test]
fn predicate_with_join() {
    let lir = ql_to_lir("predicate p(int x) { val(x) and x < 10 }");
    assert_eq!(lir.rule_count(), 1);

    let rule = &lir.strata[0].rules[0];
    assert_eq!(rule.target, "p");
    // Should have a filter (x < 10) over a scan (val)
    let pp = pretty_print(&lir);
    assert!(pp.contains("Filter"));
    assert!(pp.contains("Scan val"));
}

#[test]
fn predicate_with_result_type() {
    let lir = ql_to_lir("int doubled(int x) { val(x) and result = x * 2 }");
    let pp = pretty_print(&lir);
    // MIR lowers `result = x * 2` as `_t0 = x * 2; result = _t0`
    // So LIR should have an Extend (for the arithmetic) and a Filter (for binding result)
    assert!(pp.contains("Extend"));
    assert!(pp.contains("*"));
}

#[test]
fn recursive_path_query() {
    let lir = ql_to_lir(r#"
        predicate path(int a, int b) {
            edge(a, b)
            or
            exists(int mid | edge(a, mid) and path(mid, b))
        }
    "#);

    // path is recursive (self-referencing)
    assert!(lir.strata.iter().any(|s| s.recursive));

    let pp = pretty_print(&lir);
    assert!(pp.contains("recursive"));
    assert!(pp.contains("path("));
}

#[test]
fn negation_produces_anti_join() {
    let lir = ql_to_lir("predicate nonsink(int x) { node(x) and not sink(x) }");
    let pp = pretty_print(&lir);
    assert!(pp.contains("AntiJoin"));
}

#[test]
fn class_produces_multiple_predicates() {
    let lir = ql_to_lir(r#"
        class SmallNum extends int {
            SmallNum() { this >= 1 and this <= 9 }
            int double() { result = this * 2 }
        }
    "#);

    let rels = lir.defined_relations();
    assert!(rels.contains(&"SmallNum#char"));
    assert!(rels.contains(&"SmallNum#double"));
}

#[test]
fn select_query() {
    let lir = ql_to_lir(r#"
        from int x where val(x) and x > 3 select x
    "#);

    let rels = lir.defined_relations();
    assert!(rels.iter().any(|r| r.starts_with("select_result")));
}

#[test]
fn disjunction_multiple_rules() {
    let lir = ql_to_lir("predicate ab(int x) { a(x) or b(x) }");
    // Disjunction produces auxiliary predicates in MIR,
    // which become separate rules/strata in LIR
    assert!(lir.rule_count() >= 2);
}

#[test]
fn multi_way_join() {
    let lir = ql_to_lir(r#"
        predicate triangle(int x, int y, int z) {
            edge(x, y) and edge(y, z) and edge(z, x)
        }
    "#);

    let pp = pretty_print(&lir);
    // Should produce a WCO join over 3 atoms
    assert!(pp.contains("WcoJoin"));
}

#[test]
fn wco_join_variable_order() {
    // Triangle query: edge(x,y), edge(y,z), edge(z,x)
    // Variables y, z, x all appear in 2 atoms each
    let lir = ql_to_lir(r#"
        predicate tri(int x, int y, int z) {
            edge(x, y) and edge(y, z) and edge(z, x)
        }
    "#);

    // Find the WCO join
    fn find_wco(plan: &LirPlan) -> Option<&LirPlan> {
        match plan {
            LirPlan::WcoJoin { .. } => Some(plan),
            LirPlan::Filter { input, .. } => find_wco(input),
            LirPlan::Project { input, .. } => find_wco(input),
            _ => None,
        }
    }

    let rule = &lir.strata.last().unwrap().rules[0];
    if let Some(LirPlan::WcoJoin { atoms, variable_order, .. }) = find_wco(&rule.body) {
        assert_eq!(atoms.len(), 3);
        // All variables appear in exactly 2 atoms, so any order is valid
        assert_eq!(variable_order.len(), 3);
    } else {
        panic!("expected WcoJoin in triangle query");
    }
}

#[test]
fn stratified_negation_ordering() {
    let lir = ql_to_lir(r#"
        predicate has_succ(int x) { edge(x, _) }
        predicate no_succ(int x) { node(x) and not has_succ(x) }
    "#);

    // has_succ must be in an earlier stratum than no_succ
    let strata = &lir.strata;
    let has_succ_stratum = strata.iter().position(|s| s.rules.iter().any(|r| r.target == "has_succ"));
    let no_succ_stratum = strata.iter().position(|s| s.rules.iter().any(|r| r.target == "no_succ"));
    assert!(has_succ_stratum.unwrap() < no_succ_stratum.unwrap(),
        "has_succ should be evaluated before no_succ");
}

#[test]
fn pretty_print_smoke() {
    let lir = ql_to_lir(r#"
        predicate path(int a, int b) {
            edge(a, b)
            or
            exists(int mid | edge(a, mid) and path(mid, b))
        }
        predicate unreachable(int x) { node(x) and not path(_, x) }
    "#);

    let pp = pretty_print(&lir);
    // Should be non-empty, well-formatted
    assert!(!pp.is_empty());
    assert!(pp.contains("stratum"));
    assert!(pp.contains("path"));
    assert!(pp.contains("unreachable"));
}
