//! End-to-end tests for points-to analysis on function pointers.
//!
//! Covers:
//!   - Direct assignment: `fp = foo`
//!   - Multiple assignment (flow-insensitive): `fp = foo; fp = bar` → fp ↦ {foo, bar}
//!   - Copy propagation: `gp = fp` inherits fp's points-to set
//!   - Transitive copy chains: `gp = fp; hp = gp` → hp gets fp's targets
//!   - Branch merging: if/else assigning different targets
//!   - Parameter binding: `run(inc)` binds param f to inc at call site
//!   - Address-of: `fp = &foo`
//!   - No false positives for non-pointer variables

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

fn collect_string_pairs(db: &Database, table: &str) -> Vec<(String, String)> {
    db.scan(table).unwrap()
        .map(|t| {
            let resolve = |v: &Value| match v {
                Value::String(s) => db.strings.resolve(*s).to_string(),
                other => format!("{:?}", other),
            };
            (resolve(&t[0]), resolve(&t[1]))
        })
        .collect()
}

/// Get the set of targets a variable points to in a given function.
fn points_to_set<'a>(
    facts: &'a [(String, String, String)],
    func: &str,
    var: &str,
) -> HashSet<&'a str> {
    facts.iter()
        .filter(|(f, v, _)| f == func && v == var)
        .map(|(_, _, t)| t.as_str())
        .collect()
}

// ============================================================
// RULES
// ============================================================

/// Build comprehensive points-to analysis rules.
///
/// Level 1: Base points-to (direct assignment / init to a function name)
///   base_points_to(func, var, target)
///
/// Level 2: Copy propagation (transitive through variable copies)
///   var_copy(func, dst, src) — dst is assigned/initialized from src (both local vars)
///   points_to(func, var, target) — closed under copy chains
///
/// Level 3: Parameter binding (inter-procedural, one level)
///   param_points_to(callee_func, param_name, target)
fn pointsto_rules() -> Vec<Rule> {
    vec![
        // ================================================================
        // BASE POINTS-TO: local variable init with function name
        // ================================================================
        // base_points_to(func, var_name, target) :-
        //   localvariables(var_id, var_name, _),
        //   exprparents(init_expr, 0, var_id),
        //   exprs(init_expr, 84, _),              -- varaccess
        //   valuetext(init_expr, target),
        //   functions(_, target, _),               -- target is a function name
        //   enclosingfunction(var_id, fid),
        //   functions(fid, func, _).
        Rule::new(
            Atom::new("base_points_to", vec![var("func"), var("vn"), var("target")]),
            vec![
                BodyElement::Positive(Atom::new("localvariables", vec![
                    var("vid"), var("vn"), var("_t"),
                ])),
                BodyElement::Positive(Atom::new("exprparents", vec![
                    var("ie"), int(0), var("vid"),
                ])),
                BodyElement::Positive(Atom::new("exprs", vec![
                    var("ie"), int(84), var("_l"),
                ])),
                BodyElement::Positive(Atom::new("valuetext", vec![
                    var("ie"), var("target"),
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("_tf"), var("target"), var("_tk"),
                ])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![
                    var("vid"), var("fid"),
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("fid"), var("func"), var("_fk"),
                ])),
            ],
        ),

        // ================================================================
        // BASE POINTS-TO: assignment with function name RHS
        // ================================================================
        // base_points_to(func, lhs, target) :-
        //   exprs(aid, 52, _), exprparents(lv, 0, aid), exprs(lv, 84, _),
        //   valuetext(lv, lhs), exprparents(rv, 1, aid), exprs(rv, 84, _),
        //   valuetext(rv, target), functions(_, target, _),
        //   enclosingfunction(aid, fid), functions(fid, func, _).
        Rule::new(
            Atom::new("base_points_to", vec![var("func"), var("lhs"), var("target")]),
            vec![
                BodyElement::Positive(Atom::new("exprs", vec![var("aid"), int(52), var("_l1")])),
                BodyElement::Positive(Atom::new("exprparents", vec![var("lv"), int(0), var("aid")])),
                BodyElement::Positive(Atom::new("exprs", vec![var("lv"), int(84), var("_l2")])),
                BodyElement::Positive(Atom::new("valuetext", vec![var("lv"), var("lhs")])),
                BodyElement::Positive(Atom::new("exprparents", vec![var("rv"), int(1), var("aid")])),
                BodyElement::Positive(Atom::new("exprs", vec![var("rv"), int(84), var("_l3")])),
                BodyElement::Positive(Atom::new("valuetext", vec![var("rv"), var("target")])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("_tf2"), var("target"), var("_tk2"),
                ])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![
                    var("aid"), var("fid2"),
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("fid2"), var("func"), var("_fk2"),
                ])),
            ],
        ),

        // ================================================================
        // BASE POINTS-TO: address-of init  (`fp = &foo`)
        // ================================================================
        Rule::new(
            Atom::new("base_points_to", vec![var("func"), var("vn"), var("target")]),
            vec![
                BodyElement::Positive(Atom::new("localvariables", vec![
                    var("vid"), var("vn"), var("_t2"),
                ])),
                BodyElement::Positive(Atom::new("exprparents", vec![
                    var("aoe"), int(0), var("vid"),
                ])),
                BodyElement::Positive(Atom::new("exprs", vec![
                    var("aoe"), int(2), var("_l4"),  // address_of
                ])),
                BodyElement::Positive(Atom::new("exprparents", vec![
                    var("op"), int(0), var("aoe"),
                ])),
                BodyElement::Positive(Atom::new("exprs", vec![
                    var("op"), int(84), var("_l5"),
                ])),
                BodyElement::Positive(Atom::new("valuetext", vec![
                    var("op"), var("target"),
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("_tf3"), var("target"), var("_tk3"),
                ])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![
                    var("vid"), var("fid3"),
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("fid3"), var("func"), var("_fk3"),
                ])),
            ],
        ),

        // ================================================================
        // BASE POINTS-TO: address-of assignment  (`fp = &foo`)
        // ================================================================
        Rule::new(
            Atom::new("base_points_to", vec![var("func"), var("lhs"), var("target")]),
            vec![
                BodyElement::Positive(Atom::new("exprs", vec![var("aid3"), int(52), var("_l6")])),
                BodyElement::Positive(Atom::new("exprparents", vec![var("lv3"), int(0), var("aid3")])),
                BodyElement::Positive(Atom::new("exprs", vec![var("lv3"), int(84), var("_l7")])),
                BodyElement::Positive(Atom::new("valuetext", vec![var("lv3"), var("lhs")])),
                BodyElement::Positive(Atom::new("exprparents", vec![var("aoe3"), int(1), var("aid3")])),
                BodyElement::Positive(Atom::new("exprs", vec![var("aoe3"), int(2), var("_l8")])),
                BodyElement::Positive(Atom::new("exprparents", vec![var("op3"), int(0), var("aoe3")])),
                BodyElement::Positive(Atom::new("exprs", vec![var("op3"), int(84), var("_l9")])),
                BodyElement::Positive(Atom::new("valuetext", vec![var("op3"), var("target")])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("_tf4"), var("target"), var("_tk4"),
                ])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![
                    var("aid3"), var("fid4"),
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("fid4"), var("func"), var("_fk4"),
                ])),
            ],
        ),

        // ================================================================
        // VARIABLE COPY: declaration init from another local variable
        // ================================================================
        // var_copy(func, dst, src) :-
        //   localvariables(dst_id, dst, _),
        //   exprparents(ie, 0, dst_id),
        //   exprs(ie, 84, _),
        //   valuetext(ie, src),
        //   localvariables(_, src, _),  -- src is also a local variable (not a function)
        //   enclosingfunction(dst_id, fid),
        //   functions(fid, func, _).
        Rule::new(
            Atom::new("var_copy", vec![var("func"), var("dst"), var("src")]),
            vec![
                BodyElement::Positive(Atom::new("localvariables", vec![
                    var("did"), var("dst"), var("_dt"),
                ])),
                BodyElement::Positive(Atom::new("exprparents", vec![
                    var("ie2"), int(0), var("did"),
                ])),
                BodyElement::Positive(Atom::new("exprs", vec![
                    var("ie2"), int(84), var("_cl1"),
                ])),
                BodyElement::Positive(Atom::new("valuetext", vec![
                    var("ie2"), var("src"),
                ])),
                // Ensure src is a local variable in the same function
                BodyElement::Positive(Atom::new("localvariables", vec![
                    var("sid"), var("src"), var("_st"),
                ])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![
                    var("sid"), var("sfid"),
                ])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![
                    var("did"), var("sfid"),  // same function
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("sfid"), var("func"), var("_ck1"),
                ])),
            ],
        ),

        // ================================================================
        // VARIABLE COPY: assignment from another local variable
        // ================================================================
        // var_copy(func, dst, src) :-
        //   exprs(aid, 52, _), exprparents(lv, 0, aid), exprs(lv, 84, _),
        //   valuetext(lv, dst), exprparents(rv, 1, aid), exprs(rv, 84, _),
        //   valuetext(rv, src),
        //   localvariables(_, src, _),   -- src is a local var
        //   enclosingfunction(aid, fid), functions(fid, func, _).
        Rule::new(
            Atom::new("var_copy", vec![var("func"), var("dst"), var("src")]),
            vec![
                BodyElement::Positive(Atom::new("exprs", vec![var("ca"), int(52), var("_cl2")])),
                BodyElement::Positive(Atom::new("exprparents", vec![var("clv"), int(0), var("ca")])),
                BodyElement::Positive(Atom::new("exprs", vec![var("clv"), int(84), var("_cl3")])),
                BodyElement::Positive(Atom::new("valuetext", vec![var("clv"), var("dst")])),
                BodyElement::Positive(Atom::new("exprparents", vec![var("crv"), int(1), var("ca")])),
                BodyElement::Positive(Atom::new("exprs", vec![var("crv"), int(84), var("_cl4")])),
                BodyElement::Positive(Atom::new("valuetext", vec![var("crv"), var("src")])),
                // src must be a local variable in the same function
                BodyElement::Positive(Atom::new("localvariables", vec![
                    var("csid"), var("src"), var("_cst"),
                ])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![
                    var("csid"), var("cfid"),
                ])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![
                    var("ca"), var("cfid"),  // same function
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("cfid"), var("func"), var("_ck2"),
                ])),
            ],
        ),

        // ================================================================
        // POINTS-TO: base case
        // ================================================================
        Rule::new(
            Atom::new("points_to", vec![var("f"), var("v"), var("t")]),
            vec![BodyElement::Positive(Atom::new("base_points_to", vec![
                var("f"), var("v"), var("t"),
            ]))],
        ),

        // ================================================================
        // POINTS-TO: copy propagation (recursive)
        // ================================================================
        // points_to(func, dst, target) :-
        //   var_copy(func, dst, src),
        //   points_to(func, src, target).
        Rule::new(
            Atom::new("points_to", vec![var("f"), var("dst"), var("t")]),
            vec![
                BodyElement::Positive(Atom::new("var_copy", vec![
                    var("f"), var("dst"), var("mid"),
                ])),
                BodyElement::Positive(Atom::new("points_to", vec![
                    var("f"), var("mid"), var("t"),
                ])),
            ],
        ),

        // ================================================================
        // PARAMETER BINDING: at call sites, map actual args to formal params
        // ================================================================
        // param_points_to(callee_name, param_name, target) :-
        //   exprs(call_id, 74, _),                     -- call expression
        //   exprparents(callee_var, 0, call_id),        -- child 0 = function name
        //   exprs(callee_var, 84, _),
        //   valuetext(callee_var, callee_name),
        //   functions(callee_fid, callee_name, _),      -- callee is a real function
        //   params(callee_fid, idx, param_name, _),     -- formal param at idx
        //   actual_idx = idx + 1,                       -- actual args start at child 1
        //   exprparents(arg_expr, actual_idx, call_id), -- actual argument
        //   exprs(arg_expr, 84, _),                     -- it's a varaccess
        //   valuetext(arg_expr, target),
        //   functions(_, target, _).                    -- the argument is a function name
        //
        // NOTE: In our extracted call expressions, child 0 is the callee name,
        // and children 1, 2, ... are the arguments. So param at index 0 maps to
        // child 1 of the call, param at index 1 maps to child 2, etc.
        Rule::new(
            Atom::new("param_points_to", vec![var("callee_name"), var("pname"), var("target")]),
            vec![
                BodyElement::Positive(Atom::new("exprs", vec![
                    var("cid"), int(74), var("_p1"),
                ])),
                BodyElement::Positive(Atom::new("exprparents", vec![
                    var("cv"), int(0), var("cid"),
                ])),
                BodyElement::Positive(Atom::new("exprs", vec![
                    var("cv"), int(84), var("_p2"),
                ])),
                BodyElement::Positive(Atom::new("valuetext", vec![
                    var("cv"), var("callee_name"),
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("cfid"), var("callee_name"), var("_p3"),
                ])),
                BodyElement::Positive(Atom::new("params", vec![
                    var("cfid"), int(0), var("pname"), var("_pt"),
                ])),
                // Param index 0 → argument at child index 1
                BodyElement::Positive(Atom::new("exprparents", vec![
                    var("arg"), int(1), var("cid"),
                ])),
                BodyElement::Positive(Atom::new("exprs", vec![
                    var("arg"), int(84), var("_p4"),
                ])),
                BodyElement::Positive(Atom::new("valuetext", vec![
                    var("arg"), var("target"),
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("_tgt"), var("target"), var("_p5"),
                ])),
            ],
        ),

        // ================================================================
        // PARAMETER BINDING: propagate to intra-procedural points_to
        // ================================================================
        // When a parameter points to a function, add it to points_to for the
        // callee function so indirect call resolution can use it.
        // points_to(callee_name, param_name, target) :-
        //   param_points_to(callee_name, param_name, target).
        Rule::new(
            Atom::new("points_to", vec![var("fn"), var("pn"), var("tgt")]),
            vec![BodyElement::Positive(Atom::new("param_points_to", vec![
                var("fn"), var("pn"), var("tgt"),
            ]))],
        ),

        // ================================================================
        // INDIRECT CALL: call through a variable in points_to
        // ================================================================
        Rule::new(
            Atom::new("indirect_call", vec![var("caller"), var("callee")]),
            vec![
                BodyElement::Positive(Atom::new("exprs", vec![
                    var("cid"), int(74), var("_ic1"),
                ])),
                BodyElement::Positive(Atom::new("exprparents", vec![
                    var("cv"), int(0), var("cid"),
                ])),
                BodyElement::Positive(Atom::new("exprs", vec![
                    var("cv"), int(84), var("_ic2"),
                ])),
                BodyElement::Positive(Atom::new("valuetext", vec![
                    var("cv"), var("vname"),
                ])),
                BodyElement::Positive(Atom::new("points_to", vec![
                    var("caller"), var("vname"), var("callee"),
                ])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![
                    var("cid"), var("cfid"),
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("cfid"), var("caller"), var("_ick"),
                ])),
            ],
        ),
    ]
}

// ============================================================
// TESTS
// ============================================================

/// Helper: run the analysis and return points_to facts.
fn run_analysis(db: &mut Database) -> Vec<(String, String, String)> {
    let program = Program::new(pointsto_rules());
    evaluate(&program, db).unwrap();
    collect_string_triples(db, "points_to")
}

#[test]
fn pt_basic_assign() {
    let mut db = extract_c_fixture("pointsto.c");
    let pts = run_analysis(&mut db);

    let set = points_to_set(&pts, "basic_assign", "fp");
    eprintln!("basic_assign fp: {:?}", set);
    assert_eq!(set, HashSet::from(["inc"]), "fp should point to exactly {{inc}}");
}

#[test]
fn pt_multi_assign() {
    let mut db = extract_c_fixture("pointsto.c");
    let pts = run_analysis(&mut db);

    let set = points_to_set(&pts, "multi_assign", "fp");
    eprintln!("multi_assign fp: {:?}", set);
    // Flow-insensitive: fp was assigned both inc and mul2
    assert!(set.contains("inc"), "fp should point to inc");
    assert!(set.contains("mul2"), "fp should point to mul2");
    assert_eq!(set.len(), 2, "fp should point to exactly 2 targets");
}

#[test]
fn pt_copy_propagation() {
    let mut db = extract_c_fixture("pointsto.c");
    let pts = run_analysis(&mut db);

    let fp_set = points_to_set(&pts, "copy_propagation", "fp");
    let gp_set = points_to_set(&pts, "copy_propagation", "gp");
    eprintln!("copy_propagation fp: {:?}, gp: {:?}", fp_set, gp_set);

    assert_eq!(fp_set, HashSet::from(["inc"]), "fp should point to {{inc}}");
    assert_eq!(gp_set, HashSet::from(["inc"]),
        "gp should inherit fp's points-to set → {{inc}}");
}

#[test]
fn pt_chain_copy() {
    let mut db = extract_c_fixture("pointsto.c");
    let pts = run_analysis(&mut db);

    let fp_set = points_to_set(&pts, "chain_copy", "fp");
    let gp_set = points_to_set(&pts, "chain_copy", "gp");
    let hp_set = points_to_set(&pts, "chain_copy", "hp");
    eprintln!("chain_copy fp: {:?}, gp: {:?}, hp: {:?}", fp_set, gp_set, hp_set);

    assert_eq!(fp_set, HashSet::from(["inc"]));
    assert_eq!(gp_set, HashSet::from(["inc"]),
        "gp = fp → gp should point to {{inc}}");
    assert_eq!(hp_set, HashSet::from(["inc"]),
        "hp = gp = fp → hp should point to {{inc}}");
}

#[test]
fn pt_branch_merge() {
    let mut db = extract_c_fixture("pointsto.c");
    let pts = run_analysis(&mut db);

    let fp_set = points_to_set(&pts, "branch_merge", "fp");
    let gp_set = points_to_set(&pts, "branch_merge", "gp");
    eprintln!("branch_merge fp: {:?}, gp: {:?}", fp_set, gp_set);

    // fp assigned inc in then-branch and mul2 in else-branch
    assert_eq!(fp_set, HashSet::from(["inc", "mul2"]),
        "fp should point to {{inc, mul2}} from both branches");
    // gp = fp, so inherits both targets
    assert_eq!(gp_set, HashSet::from(["inc", "mul2"]),
        "gp = fp → gp should point to {{inc, mul2}}");
}

#[test]
fn pt_overwrite_copy() {
    let mut db = extract_c_fixture("pointsto.c");
    let pts = run_analysis(&mut db);

    let fp_set = points_to_set(&pts, "overwrite_copy", "fp");
    let gp_set = points_to_set(&pts, "overwrite_copy", "gp");
    eprintln!("overwrite_copy fp: {:?}, gp: {:?}", fp_set, gp_set);

    // Flow-insensitive: fp was assigned inc then mul2 → {inc, mul2}
    assert_eq!(fp_set, HashSet::from(["inc", "mul2"]));
    // gp = fp (flow-insensitive) → gp gets all of fp's targets
    assert_eq!(gp_set, HashSet::from(["inc", "mul2"]),
        "gp = fp (flow-insensitive) → gp should point to {{inc, mul2}}");
}

#[test]
fn pt_swap_pattern() {
    let mut db = extract_c_fixture("pointsto.c");
    let pts = run_analysis(&mut db);

    let fp_set = points_to_set(&pts, "swap_pattern", "fp");
    let gp_set = points_to_set(&pts, "swap_pattern", "gp");
    let tmp_set = points_to_set(&pts, "swap_pattern", "tmp");
    eprintln!("swap_pattern fp: {:?}, gp: {:?}, tmp: {:?}", fp_set, gp_set, tmp_set);

    // Flow-insensitive: everything merges
    // fp: initially inc, then assigned gp (which has mul2, and transitively inc via tmp)
    // gp: initially mul2, then assigned tmp (which has inc, and transitively mul2 via fp)
    // tmp: assigned from fp (which has {inc, mul2})
    // All should have {inc, mul2}
    assert!(fp_set.contains("inc") && fp_set.contains("mul2"),
        "fp should point to {{inc, mul2}}");
    assert!(gp_set.contains("inc") && gp_set.contains("mul2"),
        "gp should point to {{inc, mul2}}");
    assert!(tmp_set.contains("inc") && tmp_set.contains("mul2"),
        "tmp should point to {{inc, mul2}}");
}

#[test]
fn pt_param_binding() {
    let mut db = extract_c_fixture("pointsto.c");
    let program = Program::new(pointsto_rules());
    evaluate(&program, &mut db).unwrap();

    let param_pts = collect_string_triples(&db, "param_points_to");
    eprintln!("Parameter points-to:");
    for (func, param, target) in &param_pts {
        eprintln!("  {}({}) -> {}", func, param, target);
    }

    let pp_set: HashSet<(&str, &str, &str)> = param_pts.iter()
        .map(|(a, b, c)| (a.as_str(), b.as_str(), c.as_str()))
        .collect();

    // param_binding calls run(inc, 10)
    assert!(pp_set.contains(&("run", "f", "inc")),
        "run's param f should point to inc (from param_binding call)");
}

#[test]
fn pt_multi_caller() {
    let mut db = extract_c_fixture("pointsto.c");
    let program = Program::new(pointsto_rules());
    evaluate(&program, &mut db).unwrap();

    let param_pts = collect_string_triples(&db, "param_points_to");
    let run_f: HashSet<&str> = param_pts.iter()
        .filter(|(func, param, _)| func == "run" && param == "f")
        .map(|(_, _, t)| t.as_str())
        .collect();
    eprintln!("run.f targets: {:?}", run_f);

    // multi_caller1 calls run(inc), multi_caller2 calls run(mul2)
    assert!(run_f.contains("inc"), "run.f should include inc (from multi_caller1)");
    assert!(run_f.contains("mul2"), "run.f should include mul2 (from multi_caller2)");
}

#[test]
fn pt_param_indirect_call() {
    let mut db = extract_c_fixture("pointsto.c");
    let program = Program::new(pointsto_rules());
    evaluate(&program, &mut db).unwrap();

    // Within `run`, f(x) is an indirect call.
    // Since param_points_to propagates to points_to, f should resolve.
    let indirect = collect_string_pairs(&db, "indirect_call");
    eprintln!("Indirect calls:");
    for (caller, callee) in &indirect {
        eprintln!("  {} ~> {}", caller, callee);
    }

    let ic_set: HashSet<(&str, &str)> = indirect.iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();

    // run calls through f, which points to {inc, mul2}
    assert!(ic_set.contains(&("run", "inc")),
        "run should indirectly call inc (via param f)");
    assert!(ic_set.contains(&("run", "mul2")),
        "run should indirectly call mul2 (via param f)");
}

#[test]
fn pt_addr_of_copy() {
    let mut db = extract_c_fixture("pointsto.c");
    let pts = run_analysis(&mut db);

    let fp_set = points_to_set(&pts, "addr_of_copy", "fp");
    let gp_set = points_to_set(&pts, "addr_of_copy", "gp");
    eprintln!("addr_of_copy fp: {:?}, gp: {:?}", fp_set, gp_set);

    assert_eq!(fp_set, HashSet::from(["inc"]),
        "fp = &inc → fp should point to {{inc}}");
    assert_eq!(gp_set, HashSet::from(["inc"]),
        "gp = fp → gp should inherit {{inc}}");
}

#[test]
fn pt_no_false_positives() {
    let mut db = extract_c_fixture("pointsto.c");
    let pts = run_analysis(&mut db);

    // no_points_to has no function pointers
    let no_pts: Vec<_> = pts.iter()
        .filter(|(f, _, _)| f == "no_points_to")
        .collect();
    eprintln!("no_points_to facts: {:?}", no_pts);
    assert!(no_pts.is_empty(),
        "no_points_to should have no points-to facts");

    // Target functions (inc, mul2, neg) should have no points-to facts
    for func in &["inc", "mul2", "neg"] {
        let f_pts: Vec<_> = pts.iter()
            .filter(|(f, _, _)| f == *func)
            .collect();
        assert!(f_pts.is_empty(),
            "{} should have no points-to facts", func);
    }
}

#[test]
fn pt_neg_never_referenced() {
    let mut db = extract_c_fixture("pointsto.c");
    let program = Program::new(pointsto_rules());
    evaluate(&program, &mut db).unwrap();

    // neg is defined but never assigned to any pointer or passed as argument
    let pts = collect_string_triples(&db, "points_to");
    let neg_targets: Vec<_> = pts.iter()
        .filter(|(_, _, t)| t == "neg")
        .collect();
    eprintln!("References to neg: {:?}", neg_targets);
    assert!(neg_targets.is_empty(),
        "neg should never appear as a points-to target");

    let param_pts = collect_string_triples(&db, "param_points_to");
    let neg_params: Vec<_> = param_pts.iter()
        .filter(|(_, _, t)| t == "neg")
        .collect();
    assert!(neg_params.is_empty(),
        "neg should never appear in param_points_to");
}
