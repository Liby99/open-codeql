//! End-to-end tests: indirect (function-pointer) call resolution.
//!
//! Extract C files with function pointer patterns, then run Datalog rules
//! to resolve indirect calls through points-to analysis.

use std::collections::HashSet;

use ocql_database::{Database, Value};
use ocql_extractor_common::Extractor;
use ocql_extractor_cpp::{CppExtractor, cpp_schema};
use ocql_engine::*;

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

fn collect_string_pairs(db: &Database, table: &str) -> Vec<(String, String)> {
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

fn collect_string_triples(db: &Database, table: &str) -> Vec<(String, String, String)> {
    db.scan(table).unwrap()
        .map(|t| {
            let resolve = |v: &Value| match v {
                Value::String(s) => db.strings.resolve(*s).to_string(),
                other => format!("{:?}", other),
            };
            (resolve(&t[0]), resolve(&t[1]), resolve(&t[2]))
        })
        .collect()
}

fn dump_table(db: &Database, table: &str) {
    if let Some(rel) = db.relation(table) {
        eprintln!("  {} ({} rows):", table, rel.len());
        for tuple in rel.scan() {
            let vals: Vec<String> = tuple.iter().map(|v| match v {
                Value::String(s) => format!("\"{}\"", db.strings.resolve(*s)),
                Value::Entity(e) => format!("#{}", e.0),
                Value::Int(i) => i.to_string(),
                other => format!("{:?}", other),
            }).collect();
            eprintln!("    ({})", vals.join(", "));
        }
    } else {
        eprintln!("  {} — not found", table);
    }
}

// ============================================================
// Diagnostic: dump extracted tables for function pointer code
// ============================================================

#[test]
fn dump_indirect_calls_extraction() {
    let db = extract_c_fixture("indirect_calls.c");

    eprintln!("\n=== Extracted tables for indirect_calls.c ===\n");
    dump_table(&db, "functions");
    dump_table(&db, "localvariables");
    dump_table(&db, "params");
    dump_table(&db, "exprs");
    dump_table(&db, "exprparents");
    dump_table(&db, "valuetext");
    dump_table(&db, "enclosingfunction");
    eprintln!();
}

// ============================================================
// RULES
// ============================================================

/// Build rules for points-to analysis and indirect call resolution.
///
/// points_to(func, var_name, target_func):
///   A local variable `var_name` in function `func` may point to `target_func`.
///
/// Sources of points-to:
///   1. Declaration init: `IntOp fp = foo;`  (varaccess init matching a function name)
///   2. Assignment: `fp = foo;`              (varaccess RHS matching a function name)
///   3. Address-of: `IntOp fp = &foo;`       (address_of expr whose operand is a function name)
///
/// direct_call(caller, callee):
///   Standard direct call resolution (callee varaccess name is a function).
///
/// indirect_call(caller, callee):
///   Call through a variable that points-to a function.
///
/// resolved_call(caller, callee):
///   Union of direct and indirect calls.
fn indirect_call_rules() -> Vec<Rule> {
    vec![
        // ---- Points-to: declaration initializer (varaccess) ----
        // points_to(func, var_name, target) :-
        //   localvariables(var_id, var_name, _),
        //   exprparents(init_expr, 0, var_id),
        //   exprs(init_expr, 84, _),              -- varaccess
        //   valuetext(init_expr, target),
        //   functions(_, target, _),               -- target is a function name
        //   enclosingfunction(var_id, fid),
        //   functions(fid, func, _).
        Rule::new(
            Atom::new("points_to", vec![var("func"), var("var_name"), var("target")]),
            vec![
                BodyElement::Positive(Atom::new("localvariables", vec![
                    var("var_id"), var("var_name"), var("_type"),
                ])),
                BodyElement::Positive(Atom::new("exprparents", vec![
                    var("init_expr"), int(0), var("var_id"),
                ])),
                BodyElement::Positive(Atom::new("exprs", vec![
                    var("init_expr"), int(84), var("_loc"),
                ])),
                BodyElement::Positive(Atom::new("valuetext", vec![
                    var("init_expr"), var("target"),
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("_target_fid"), var("target"), var("_tk"),
                ])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![
                    var("var_id"), var("fid"),
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("fid"), var("func"), var("_fk"),
                ])),
            ],
        ),

        // ---- Points-to: assignment (varaccess RHS) ----
        // points_to(func, lhs_name, target) :-
        //   exprs(aid, 52, _),                   -- assignment
        //   exprparents(lv, 0, aid),
        //   exprs(lv, 84, _),
        //   valuetext(lv, lhs_name),
        //   exprparents(rv, 1, aid),
        //   exprs(rv, 84, _),
        //   valuetext(rv, target),
        //   functions(_, target, _),
        //   enclosingfunction(aid, fid),
        //   functions(fid, func, _).
        Rule::new(
            Atom::new("points_to", vec![var("func"), var("lhs_name"), var("target")]),
            vec![
                BodyElement::Positive(Atom::new("exprs", vec![var("aid"), int(52), var("_l1")])),
                BodyElement::Positive(Atom::new("exprparents", vec![var("lv"), int(0), var("aid")])),
                BodyElement::Positive(Atom::new("exprs", vec![var("lv"), int(84), var("_l2")])),
                BodyElement::Positive(Atom::new("valuetext", vec![var("lv"), var("lhs_name")])),
                BodyElement::Positive(Atom::new("exprparents", vec![var("rv"), int(1), var("aid")])),
                BodyElement::Positive(Atom::new("exprs", vec![var("rv"), int(84), var("_l3")])),
                BodyElement::Positive(Atom::new("valuetext", vec![var("rv"), var("target")])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("_target_fid2"), var("target"), var("_tk2"),
                ])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![
                    var("aid"), var("fid2"),
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("fid2"), var("func"), var("_fk2"),
                ])),
            ],
        ),

        // ---- Points-to: address-of init (`IntOp fp = &foo;`) ----
        // points_to(func, var_name, target) :-
        //   localvariables(var_id, var_name, _),
        //   exprparents(addr_expr, 0, var_id),
        //   exprs(addr_expr, 2, _),               -- address_of (kind 2)
        //   exprparents(operand, 0, addr_expr),
        //   exprs(operand, 84, _),
        //   valuetext(operand, target),
        //   functions(_, target, _),
        //   enclosingfunction(var_id, fid),
        //   functions(fid, func, _).
        Rule::new(
            Atom::new("points_to", vec![var("func"), var("var_name"), var("target")]),
            vec![
                BodyElement::Positive(Atom::new("localvariables", vec![
                    var("var_id"), var("var_name"), var("_type3"),
                ])),
                BodyElement::Positive(Atom::new("exprparents", vec![
                    var("addr_expr"), int(0), var("var_id"),
                ])),
                BodyElement::Positive(Atom::new("exprs", vec![
                    var("addr_expr"), int(2), var("_loc3"),
                ])),
                BodyElement::Positive(Atom::new("exprparents", vec![
                    var("operand"), int(0), var("addr_expr"),
                ])),
                BodyElement::Positive(Atom::new("exprs", vec![
                    var("operand"), int(84), var("_loc4"),
                ])),
                BodyElement::Positive(Atom::new("valuetext", vec![
                    var("operand"), var("target"),
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("_target_fid3"), var("target"), var("_tk3"),
                ])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![
                    var("var_id"), var("fid3"),
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("fid3"), var("func"), var("_fk3"),
                ])),
            ],
        ),

        // ---- Points-to: address-of assignment (`fp = &foo;`) ----
        Rule::new(
            Atom::new("points_to", vec![var("func"), var("lhs_name"), var("target")]),
            vec![
                BodyElement::Positive(Atom::new("exprs", vec![var("aid3"), int(52), var("_l7")])),
                BodyElement::Positive(Atom::new("exprparents", vec![var("lv3"), int(0), var("aid3")])),
                BodyElement::Positive(Atom::new("exprs", vec![var("lv3"), int(84), var("_l8")])),
                BodyElement::Positive(Atom::new("valuetext", vec![var("lv3"), var("lhs_name")])),
                BodyElement::Positive(Atom::new("exprparents", vec![var("addr3"), int(1), var("aid3")])),
                BodyElement::Positive(Atom::new("exprs", vec![var("addr3"), int(2), var("_l9")])),
                BodyElement::Positive(Atom::new("exprparents", vec![var("op3"), int(0), var("addr3")])),
                BodyElement::Positive(Atom::new("exprs", vec![var("op3"), int(84), var("_l10")])),
                BodyElement::Positive(Atom::new("valuetext", vec![var("op3"), var("target")])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("_target_fid4"), var("target"), var("_tk4"),
                ])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![
                    var("aid3"), var("fid4"),
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("fid4"), var("func"), var("_fk4"),
                ])),
            ],
        ),

        // ---- Direct call ----
        // direct_call(caller, callee) :-
        //   exprs(call_id, 74, _),
        //   exprparents(callee_var, 0, call_id),
        //   exprs(callee_var, 84, _),
        //   valuetext(callee_var, callee),
        //   functions(_, callee, _),              -- callee name IS a function
        //   enclosingfunction(call_id, fid),
        //   functions(fid, caller, _).
        Rule::new(
            Atom::new("direct_call", vec![var("caller"), var("callee")]),
            vec![
                BodyElement::Positive(Atom::new("exprs", vec![
                    var("call_id"), int(74), var("_cl1"),
                ])),
                BodyElement::Positive(Atom::new("exprparents", vec![
                    var("callee_var"), int(0), var("call_id"),
                ])),
                BodyElement::Positive(Atom::new("exprs", vec![
                    var("callee_var"), int(84), var("_cl2"),
                ])),
                BodyElement::Positive(Atom::new("valuetext", vec![
                    var("callee_var"), var("callee"),
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("_callee_fid"), var("callee"), var("_ck"),
                ])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![
                    var("call_id"), var("caller_fid"),
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("caller_fid"), var("caller"), var("_ck2"),
                ])),
            ],
        ),

        // ---- Indirect call ----
        // indirect_call(caller, callee) :-
        //   exprs(call_id, 74, _),
        //   exprparents(callee_var, 0, call_id),
        //   exprs(callee_var, 84, _),
        //   valuetext(callee_var, var_name),
        //   points_to(caller, var_name, callee),
        //   enclosingfunction(call_id, fid),
        //   functions(fid, caller, _).
        Rule::new(
            Atom::new("indirect_call", vec![var("caller"), var("callee")]),
            vec![
                BodyElement::Positive(Atom::new("exprs", vec![
                    var("call_id"), int(74), var("_ic1"),
                ])),
                BodyElement::Positive(Atom::new("exprparents", vec![
                    var("callee_var"), int(0), var("call_id"),
                ])),
                BodyElement::Positive(Atom::new("exprs", vec![
                    var("callee_var"), int(84), var("_ic2"),
                ])),
                BodyElement::Positive(Atom::new("valuetext", vec![
                    var("callee_var"), var("var_name"),
                ])),
                BodyElement::Positive(Atom::new("points_to", vec![
                    var("caller"), var("var_name"), var("callee"),
                ])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![
                    var("call_id"), var("caller_fid"),
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("caller_fid"), var("caller"), var("_ick"),
                ])),
            ],
        ),

        // ---- Resolved call = direct | indirect ----
        Rule::new(
            Atom::new("resolved_call", vec![var("a"), var("b")]),
            vec![BodyElement::Positive(Atom::new("direct_call", vec![var("a"), var("b")]))],
        ),
        Rule::new(
            Atom::new("resolved_call", vec![var("a"), var("b")]),
            vec![BodyElement::Positive(Atom::new("indirect_call", vec![var("a"), var("b")]))],
        ),

        // ---- Transitive resolved calls ----
        Rule::new(
            Atom::new("transitive_call", vec![var("a"), var("b")]),
            vec![BodyElement::Positive(Atom::new("resolved_call", vec![var("a"), var("b")]))],
        ),
        Rule::new(
            Atom::new("transitive_call", vec![var("a"), var("b")]),
            vec![
                BodyElement::Positive(Atom::new("transitive_call", vec![var("a"), var("c")])),
                BodyElement::Positive(Atom::new("resolved_call", vec![var("c"), var("b")])),
            ],
        ),
    ]
}

// ============================================================
// TESTS
// ============================================================

#[test]
fn indirect_points_to() {
    let mut db = extract_c_fixture("indirect_calls.c");
    let program = Program::new(indirect_call_rules());
    evaluate(&program, &mut db).unwrap();

    let pts = collect_string_triples(&db, "points_to");
    eprintln!("Points-to facts:");
    for (func, var, target) in &pts {
        eprintln!("  {} : {} -> {}", func, var, target);
    }

    let pt_set: HashSet<(&str, &str, &str)> = pts.iter()
        .map(|(a, b, c)| (a.as_str(), b.as_str(), c.as_str()))
        .collect();

    // single_target: fp -> inc
    assert!(pt_set.contains(&("single_target", "fp", "inc")),
        "single_target: fp should point to inc");

    // multi_target: fp -> {add1, mul2}
    assert!(pt_set.contains(&("multi_target", "fp", "add1")),
        "multi_target: fp should point to add1");
    assert!(pt_set.contains(&("multi_target", "fp", "mul2")),
        "multi_target: fp should point to mul2");

    // addr_of: fp -> inc (via &inc)
    assert!(pt_set.contains(&("addr_of", "fp", "inc")),
        "addr_of: fp should point to inc");

    // conditional_fp: fp -> {inc, mul2}
    assert!(pt_set.contains(&("conditional_fp", "fp", "inc")),
        "conditional_fp: fp should point to inc");
    assert!(pt_set.contains(&("conditional_fp", "fp", "mul2")),
        "conditional_fp: fp should point to mul2");

    // chained_calls: fp -> inc, gp -> mul2
    assert!(pt_set.contains(&("chained_calls", "fp", "inc")),
        "chained_calls: fp should point to inc");
    assert!(pt_set.contains(&("chained_calls", "gp", "mul2")),
        "chained_calls: gp should point to mul2");
}

#[test]
fn indirect_direct_calls() {
    let mut db = extract_c_fixture("indirect_calls.c");
    let program = Program::new(indirect_call_rules());
    evaluate(&program, &mut db).unwrap();

    let direct = collect_string_pairs(&db, "direct_call");
    eprintln!("Direct calls:");
    for (caller, callee) in &direct {
        eprintln!("  {} -> {}", caller, callee);
    }

    let dc_set: HashSet<(&str, &str)> = direct.iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();

    // no_indirect calls inc directly
    assert!(dc_set.contains(&("no_indirect", "inc")),
        "no_indirect should directly call inc");

    // use_callback calls apply directly
    assert!(dc_set.contains(&("use_callback", "apply")),
        "use_callback should directly call apply");

    // single_target does NOT directly call inc (it calls through fp)
    assert!(!dc_set.contains(&("single_target", "inc")),
        "single_target should NOT directly call inc (it uses fp)");
}

#[test]
fn indirect_call_resolution() {
    let mut db = extract_c_fixture("indirect_calls.c");
    let program = Program::new(indirect_call_rules());
    evaluate(&program, &mut db).unwrap();

    let indirect = collect_string_pairs(&db, "indirect_call");
    eprintln!("Indirect calls:");
    for (caller, callee) in &indirect {
        eprintln!("  {} ~> {}", caller, callee);
    }

    let ic_set: HashSet<(&str, &str)> = indirect.iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();

    // single_target: fp(5) → inc
    assert!(ic_set.contains(&("single_target", "inc")),
        "single_target should indirectly call inc via fp");

    // multi_target: fp → {add1, mul2}
    assert!(ic_set.contains(&("multi_target", "add1")),
        "multi_target should indirectly call add1 via fp");
    assert!(ic_set.contains(&("multi_target", "mul2")),
        "multi_target should indirectly call mul2 via fp");

    // addr_of: fp → inc
    assert!(ic_set.contains(&("addr_of", "inc")),
        "addr_of should indirectly call inc via fp");

    // conditional_fp: fp → {inc, mul2}
    assert!(ic_set.contains(&("conditional_fp", "inc")),
        "conditional_fp should indirectly call inc via fp");
    assert!(ic_set.contains(&("conditional_fp", "mul2")),
        "conditional_fp should indirectly call mul2 via fp");

    // chained_calls: fp → inc, gp → mul2
    assert!(ic_set.contains(&("chained_calls", "inc")),
        "chained_calls should indirectly call inc via fp");
    assert!(ic_set.contains(&("chained_calls", "mul2")),
        "chained_calls should indirectly call mul2 via gp");

    // no_indirect should have NO indirect calls
    assert!(!indirect.iter().any(|(caller, _)| caller == "no_indirect"),
        "no_indirect should have no indirect calls");
}

#[test]
fn indirect_resolved_complete() {
    let mut db = extract_c_fixture("indirect_calls.c");
    let program = Program::new(indirect_call_rules());
    evaluate(&program, &mut db).unwrap();

    let resolved = collect_string_pairs(&db, "resolved_call");
    eprintln!("Resolved calls (direct + indirect):");
    for (caller, callee) in &resolved {
        eprintln!("  {} => {}", caller, callee);
    }

    let rc_set: HashSet<(&str, &str)> = resolved.iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();

    // no_indirect: direct call to inc
    assert!(rc_set.contains(&("no_indirect", "inc")));

    // single_target: indirect call to inc
    assert!(rc_set.contains(&("single_target", "inc")));

    // multi_target: indirect calls to add1, mul2
    assert!(rc_set.contains(&("multi_target", "add1")));
    assert!(rc_set.contains(&("multi_target", "mul2")));

    // use_callback: direct call to apply, plus inc passed as arg
    assert!(rc_set.contains(&("use_callback", "apply")));

    // chained_calls: indirect inc + mul2
    assert!(rc_set.contains(&("chained_calls", "inc")));
    assert!(rc_set.contains(&("chained_calls", "mul2")));
}

#[test]
fn indirect_transitive_reachability() {
    let mut db = extract_c_fixture("indirect_calls.c");
    let program = Program::new(indirect_call_rules());
    evaluate(&program, &mut db).unwrap();

    let trans = collect_string_pairs(&db, "transitive_call");
    eprintln!("Transitive calls:");
    for (caller, callee) in &trans {
        eprintln!("  {} =>* {}", caller, callee);
    }

    let tc_set: HashSet<(&str, &str)> = trans.iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();

    // use_callback → apply → (f resolves within apply if we had inter-proc)
    // At minimum, use_callback transitively reaches apply
    assert!(tc_set.contains(&("use_callback", "apply")));

    // no_indirect → inc (direct, thus transitive)
    assert!(tc_set.contains(&("no_indirect", "inc")));

    // single_target → inc (via indirect)
    assert!(tc_set.contains(&("single_target", "inc")));
}

#[test]
fn indirect_no_false_positives() {
    let mut db = extract_c_fixture("indirect_calls.c");
    let program = Program::new(indirect_call_rules());
    evaluate(&program, &mut db).unwrap();

    let indirect = collect_string_pairs(&db, "indirect_call");
    let ic_set: HashSet<(&str, &str)> = indirect.iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();

    // Target functions (inc, add1, mul2, negate) should not have indirect calls
    // (they contain no function pointer calls)
    for target in &["inc", "add1", "mul2", "negate"] {
        assert!(!indirect.iter().any(|(caller, _)| caller == *target),
            "{} should not have indirect calls", target);
    }

    // negate is never used — should not appear as a callee anywhere
    let resolved = collect_string_pairs(&db, "resolved_call");
    assert!(!resolved.iter().any(|(_, callee)| callee == "negate"),
        "negate should never be called (not used in any test function)");
}
