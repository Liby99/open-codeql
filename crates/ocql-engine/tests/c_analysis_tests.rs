//! End-to-end tests: extract C files → populate database → run Datalog queries.
//!
//! Tests call graph resolution, local dataflow, and control flow analysis
//! on small C files with known expected answers.

use std::collections::HashSet;

use ocql_database::{Database, Value};
use ocql_extractor_common::Extractor;
use ocql_extractor_cpp::{CppExtractor, cpp_schema};
use ocql_engine::*;
use smallvec::smallvec;

// ============================================================
// Helpers
// ============================================================

/// Extract a single C fixture file into a fresh database.
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

/// Collect string values from a single-column relation.
fn collect_strings(db: &Database, table: &str) -> Vec<String> {
    db.scan(table).unwrap()
        .map(|t| match &t[0] {
            Value::String(s) => db.strings.resolve(*s).to_string(),
            other => format!("{:?}", other),
        })
        .collect()
}

/// Collect (col0_string, col1_string) pairs from a two-column relation.
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

/// Dump a table for debugging.
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
    }
}

// ============================================================
// CALL GRAPH RESOLUTION
// ============================================================

#[test]
fn callgraph_direct_calls() {
    let mut db = extract_c_fixture("callgraph.c");

    // Schema reminder:
    //   functions(id, name, kind)
    //   exprs(id, kind, location)          -- kind 74 = call
    //   exprparents(expr_id, child_index, parent_id)
    //   valuetext(id, text)                -- name of identifier/varaccess
    //   enclosingfunction(child, parent)
    //
    // Call graph resolution:
    //   A call expression (kind=74) has child 0 = the function identifier (kind=84, varaccess).
    //   The valuetext of that identifier is the callee name.
    //   The enclosingfunction of the call expr gives us the caller function.
    //
    // direct_call(caller_name, callee_name) :-
    //   exprs(call_id, 74, _),                    -- call expression
    //   exprparents(callee_var, 0, call_id),       -- child 0 = function name
    //   exprs(callee_var, 84, _),                  -- it's a varaccess
    //   valuetext(callee_var, callee_name),         -- get callee name
    //   enclosingfunction(call_id, caller_func),   -- get enclosing function
    //   functions(caller_func, caller_name, _).     -- get caller name

    let program = Program::new(vec![
        Rule::new(
            Atom::new("direct_call", vec![var("caller_name"), var("callee_name")]),
            vec![
                // call_id is a call expression (kind 74)
                BodyElement::Positive(Atom::new("exprs", vec![
                    var("call_id"), int(74), var("_loc1"),
                ])),
                // child 0 of call is the callee identifier
                BodyElement::Positive(Atom::new("exprparents", vec![
                    var("callee_var"), int(0), var("call_id"),
                ])),
                // callee_var is a varaccess (kind 84)
                BodyElement::Positive(Atom::new("exprs", vec![
                    var("callee_var"), int(84), var("_loc2"),
                ])),
                // get the callee name from valuetext
                BodyElement::Positive(Atom::new("valuetext", vec![
                    var("callee_var"), var("callee_name"),
                ])),
                // get enclosing function of the call
                BodyElement::Positive(Atom::new("enclosingfunction", vec![
                    var("call_id"), var("caller_func"),
                ])),
                // get the caller function's name
                BodyElement::Positive(Atom::new("functions", vec![
                    var("caller_func"), var("caller_name"), var("_kind"),
                ])),
            ],
        ),
    ]);

    evaluate(&program, &mut db).unwrap();

    let calls = collect_string_pairs(&db, "direct_call");
    eprintln!("Direct calls:");
    for (caller, callee) in &calls {
        eprintln!("  {} -> {}", caller, callee);
    }

    let call_set: HashSet<(&str, &str)> = calls.iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();

    // Expected direct calls:
    assert!(call_set.contains(&("main", "foo")), "main should call foo");
    assert!(call_set.contains(&("main", "bar")), "main should call bar");
    assert!(call_set.contains(&("foo", "bar")), "foo should call bar");
    assert!(call_set.contains(&("foo", "baz")), "foo should call baz");
    assert!(call_set.contains(&("bar", "baz")), "bar should call baz");

    // Negative: baz calls nothing
    assert!(!calls.iter().any(|(caller, _)| caller == "baz"), "baz should not call anything");
    assert_eq!(calls.len(), 5, "should have exactly 5 direct call edges");
}

#[test]
fn callgraph_transitive() {
    let mut db = extract_c_fixture("callgraph.c");

    let program = Program::new(vec![
        // direct_call(caller_name, callee_name) — same as above
        Rule::new(
            Atom::new("direct_call", vec![var("caller_name"), var("callee_name")]),
            vec![
                BodyElement::Positive(Atom::new("exprs", vec![
                    var("call_id"), int(74), var("_loc1"),
                ])),
                BodyElement::Positive(Atom::new("exprparents", vec![
                    var("callee_var"), int(0), var("call_id"),
                ])),
                BodyElement::Positive(Atom::new("exprs", vec![
                    var("callee_var"), int(84), var("_loc2"),
                ])),
                BodyElement::Positive(Atom::new("valuetext", vec![
                    var("callee_var"), var("callee_name"),
                ])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![
                    var("call_id"), var("caller_func"),
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("caller_func"), var("caller_name"), var("_kind"),
                ])),
            ],
        ),
        // transitive_call(a, b) :- direct_call(a, b).
        Rule::new(
            Atom::new("transitive_call", vec![var("a"), var("b")]),
            vec![BodyElement::Positive(Atom::new("direct_call", vec![var("a"), var("b")]))],
        ),
        // transitive_call(a, b) :- transitive_call(a, c), direct_call(c, b).
        Rule::new(
            Atom::new("transitive_call", vec![var("a"), var("b")]),
            vec![
                BodyElement::Positive(Atom::new("transitive_call", vec![var("a"), var("c")])),
                BodyElement::Positive(Atom::new("direct_call", vec![var("c"), var("b")])),
            ],
        ),
    ]);

    evaluate(&program, &mut db).unwrap();

    let trans = collect_string_pairs(&db, "transitive_call");
    eprintln!("Transitive calls:");
    for (caller, callee) in &trans {
        eprintln!("  {} ->* {}", caller, callee);
    }

    let trans_set: HashSet<(&str, &str)> = trans.iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();

    // main transitively calls: foo, bar, baz
    assert!(trans_set.contains(&("main", "foo")));
    assert!(trans_set.contains(&("main", "bar")));
    assert!(trans_set.contains(&("main", "baz")));

    // foo transitively calls: bar, baz
    assert!(trans_set.contains(&("foo", "bar")));
    assert!(trans_set.contains(&("foo", "baz")));

    // bar transitively calls: baz
    assert!(trans_set.contains(&("bar", "baz")));

    // Total: main->{foo,bar,baz} + foo->{bar,baz} + bar->{baz} = 3+2+1 = 6
    assert_eq!(trans.len(), 6, "should have 6 transitive call pairs");
}

// ============================================================
// LOCAL DATAFLOW ANALYSIS
// ============================================================

/// Build the standard local dataflow rules.
///
/// Two sources of variable-to-variable flow:
/// 1. Assignment: `y = x` → assignment expr (kind=52), child 0=LHS varaccess, child 1=RHS varaccess
/// 2. Declaration init: `int b = a` → localvariables(var_id, "b", _), init expr parented to var_id
///    is a varaccess with valuetext "a"
fn dataflow_rules() -> Vec<Rule> {
    vec![
        // Source 1: assignment expression (kind 52)
        // assign_flow(func, lhs, rhs) :- exprs(aid, 52, _), exprparents(lv, 0, aid), ...
        Rule::new(
            Atom::new("var_flow", vec![var("func"), var("lhs"), var("rhs")]),
            vec![
                BodyElement::Positive(Atom::new("exprs", vec![var("aid"), int(52), var("_l1")])),
                BodyElement::Positive(Atom::new("exprparents", vec![var("lv"), int(0), var("aid")])),
                BodyElement::Positive(Atom::new("exprparents", vec![var("rv"), int(1), var("aid")])),
                BodyElement::Positive(Atom::new("exprs", vec![var("lv"), int(84), var("_l2")])),
                BodyElement::Positive(Atom::new("exprs", vec![var("rv"), int(84), var("_l3")])),
                BodyElement::Positive(Atom::new("valuetext", vec![var("lv"), var("lhs")])),
                BodyElement::Positive(Atom::new("valuetext", vec![var("rv"), var("rhs")])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![var("aid"), var("fid")])),
                BodyElement::Positive(Atom::new("functions", vec![var("fid"), var("func"), var("_k")])),
            ],
        ),
        // Source 2: declaration initializer
        // var_flow(func, var_name, init_name) :-
        //   localvariables(var_id, var_name, _),
        //   exprparents(init_expr, 0, var_id),       -- init expr parented to the var
        //   exprs(init_expr, 84, _),                  -- init is a varaccess
        //   valuetext(init_expr, init_name),
        //   enclosingfunction(var_id, fid),
        //   functions(fid, func, _).
        Rule::new(
            Atom::new("var_flow", vec![var("func"), var("var_name"), var("init_name")]),
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
                    var("init_expr"), var("init_name"),
                ])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![
                    var("var_id"), var("fid"),
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("fid"), var("func"), var("_k"),
                ])),
            ],
        ),
        // Source 3: declaration initializer with compound RHS (e.g. `int c = b + 1`)
        // The init expr is NOT a varaccess, but has a child that IS a varaccess.
        // var_flow(func, var_name, operand_name) :-
        //   localvariables(var_id, var_name, _),
        //   exprparents(init_expr, 0, var_id),
        //   exprparents(operand, _, init_expr),       -- child of init expr
        //   exprs(operand, 84, _),                    -- operand is a varaccess
        //   valuetext(operand, operand_name),
        //   enclosingfunction(var_id, fid),
        //   functions(fid, func, _).
        Rule::new(
            Atom::new("var_flow", vec![var("func"), var("var_name"), var("operand_name")]),
            vec![
                BodyElement::Positive(Atom::new("localvariables", vec![
                    var("var_id"), var("var_name"), var("_type2"),
                ])),
                BodyElement::Positive(Atom::new("exprparents", vec![
                    var("init_expr"), int(0), var("var_id"),
                ])),
                BodyElement::Positive(Atom::new("exprparents", vec![
                    var("operand"), var("_idx"), var("init_expr"),
                ])),
                BodyElement::Positive(Atom::new("exprs", vec![
                    var("operand"), int(84), var("_loc2"),
                ])),
                BodyElement::Positive(Atom::new("valuetext", vec![
                    var("operand"), var("operand_name"),
                ])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![
                    var("var_id"), var("fid2"),
                ])),
                BodyElement::Positive(Atom::new("functions", vec![
                    var("fid2"), var("func"), var("_k2"),
                ])),
            ],
        ),
        // Source 4: assignment with compound RHS (e.g. `y = x + 1`)
        // var_flow(func, lhs, operand_name) :-
        //   exprs(aid, 52, _),
        //   exprparents(lv, 0, aid), exprs(lv, 84, _), valuetext(lv, lhs),
        //   exprparents(rv, 1, aid),
        //   exprparents(operand, _, rv), exprs(operand, 84, _), valuetext(operand, operand_name),
        //   enclosingfunction(aid, fid), functions(fid, func, _).
        Rule::new(
            Atom::new("var_flow", vec![var("func"), var("lhs2"), var("operand_name2")]),
            vec![
                BodyElement::Positive(Atom::new("exprs", vec![var("aid2"), int(52), var("_l4")])),
                BodyElement::Positive(Atom::new("exprparents", vec![var("lv2"), int(0), var("aid2")])),
                BodyElement::Positive(Atom::new("exprs", vec![var("lv2"), int(84), var("_l5")])),
                BodyElement::Positive(Atom::new("valuetext", vec![var("lv2"), var("lhs2")])),
                BodyElement::Positive(Atom::new("exprparents", vec![var("rv2"), int(1), var("aid2")])),
                BodyElement::Positive(Atom::new("exprparents", vec![var("op2"), var("_idx2"), var("rv2")])),
                BodyElement::Positive(Atom::new("exprs", vec![var("op2"), int(84), var("_l6")])),
                BodyElement::Positive(Atom::new("valuetext", vec![var("op2"), var("operand_name2")])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![var("aid2"), var("fid3")])),
                BodyElement::Positive(Atom::new("functions", vec![var("fid3"), var("func"), var("_k3")])),
            ],
        ),
    ]
}

#[test]
fn dataflow_local_variable_flow() {
    let mut db = extract_c_fixture("dataflow.c");

    let program = Program::new(dataflow_rules());
    evaluate(&program, &mut db).unwrap();

    let flows = db.scan("var_flow").unwrap()
        .map(|t| {
            let func = match &t[0] { Value::String(s) => db.strings.resolve(*s).to_string(), _ => "?".into() };
            let lhs = match &t[1] { Value::String(s) => db.strings.resolve(*s).to_string(), _ => "?".into() };
            let rhs = match &t[2] { Value::String(s) => db.strings.resolve(*s).to_string(), _ => "?".into() };
            (func, lhs, rhs)
        })
        .collect::<Vec<_>>();

    eprintln!("Variable flows:");
    for (func, lhs, rhs) in &flows {
        eprintln!("  {} : {} <- {}", func, lhs, rhs);
    }

    // In simple_flow: int b = a (b <- a via decl init)
    assert!(flows.contains(&("simple_flow".into(), "b".into(), "a".into())),
        "simple_flow should have b <- a");

    // In branching_flow: int x = input, y = x (assign), int z = y
    assert!(flows.contains(&("branching_flow".into(), "x".into(), "input".into())),
        "branching_flow should have x <- input");
    assert!(flows.contains(&("branching_flow".into(), "y".into(), "x".into())),
        "branching_flow should have y <- x");
    assert!(flows.contains(&("branching_flow".into(), "z".into(), "y".into())),
        "branching_flow should have z <- y");
}

#[test]
fn dataflow_transitive_local_flow() {
    let mut db = extract_c_fixture("dataflow.c");

    let mut rules = dataflow_rules();
    // flow(func, src, dst) :- var_flow(func, dst, src).
    rules.push(Rule::new(
        Atom::new("flow", vec![var("func"), var("src"), var("dst")]),
        vec![BodyElement::Positive(Atom::new("var_flow", vec![var("func"), var("dst"), var("src")]))],
    ));
    // flow(func, src, dst) :- flow(func, src, mid), var_flow(func, dst, mid).
    rules.push(Rule::new(
        Atom::new("flow", vec![var("func"), var("src"), var("dst")]),
        vec![
            BodyElement::Positive(Atom::new("flow", vec![var("func"), var("src"), var("mid")])),
            BodyElement::Positive(Atom::new("var_flow", vec![var("func"), var("dst"), var("mid")])),
        ],
    ));

    let program = Program::new(rules);
    evaluate(&program, &mut db).unwrap();

    let flows = db.scan("flow").unwrap()
        .map(|t| {
            let func = match &t[0] { Value::String(s) => db.strings.resolve(*s).to_string(), _ => "?".into() };
            let src = match &t[1] { Value::String(s) => db.strings.resolve(*s).to_string(), _ => "?".into() };
            let dst = match &t[2] { Value::String(s) => db.strings.resolve(*s).to_string(), _ => "?".into() };
            (func, src, dst)
        })
        .collect::<Vec<_>>();

    eprintln!("Transitive flows:");
    for (func, src, dst) in &flows {
        eprintln!("  {} : {} -> {}", func, src, dst);
    }

    // In simple_flow: a -> b -> c (transitive: a -> c)
    assert!(flows.contains(&("simple_flow".into(), "a".into(), "b".into())));
    assert!(flows.contains(&("simple_flow".into(), "a".into(), "c".into())),
        "simple_flow should have transitive flow a -> c (via b)");

    // In branching_flow: input -> x -> y -> z (transitive: input -> z)
    assert!(flows.contains(&("branching_flow".into(), "input".into(), "x".into())));
    assert!(flows.contains(&("branching_flow".into(), "input".into(), "z".into())),
        "branching_flow should have transitive flow input -> z");
    assert!(flows.contains(&("branching_flow".into(), "x".into(), "z".into())),
        "branching_flow should have transitive flow x -> z");
}

// ============================================================
// LOCAL CONTROL FLOW ANALYSIS
// ============================================================

#[test]
fn controlflow_if_structure() {
    let mut db = extract_c_fixture("controlflow.c");

    // Query: find functions that have if statements, and their then/else branches.
    //
    // Schema:
    //   stmts(id, kind, location)        -- kind 2 = if
    //   if_then(if_stmt, then_id)
    //   if_else(if_stmt, else_id)
    //   enclosingfunction(child, parent)
    //   functions(id, name, kind)
    //
    // has_if(func_name) :- stmts(s, 2, _), enclosingfunction(s, f), functions(f, func_name, _).
    // has_else(func_name) :- stmts(s, 2, _), if_else(s, _), enclosingfunction(s, f), functions(f, func_name, _).

    let program = Program::new(vec![
        Rule::new(
            Atom::new("has_if", vec![var("fname")]),
            vec![
                BodyElement::Positive(Atom::new("stmts", vec![var("s"), int(2), var("_l")])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![var("s"), var("f")])),
                BodyElement::Positive(Atom::new("functions", vec![var("f"), var("fname"), var("_k")])),
            ],
        ),
        Rule::new(
            Atom::new("has_else", vec![var("fname")]),
            vec![
                BodyElement::Positive(Atom::new("stmts", vec![var("s"), int(2), var("_l")])),
                BodyElement::Positive(Atom::new("if_else", vec![var("s"), var("_e")])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![var("s"), var("f")])),
                BodyElement::Positive(Atom::new("functions", vec![var("f"), var("fname"), var("_k")])),
            ],
        ),
    ]);

    evaluate(&program, &mut db).unwrap();

    let if_funcs = collect_strings(&db, "has_if");
    let else_funcs = collect_strings(&db, "has_else");

    eprintln!("Functions with if: {:?}", if_funcs);
    eprintln!("Functions with if+else: {:?}", else_funcs);

    // if_branch has if+else, nested has if (no else)
    assert!(if_funcs.contains(&"if_branch".to_string()), "if_branch should have if");
    assert!(if_funcs.contains(&"nested".to_string()), "nested should have if");
    assert!(!if_funcs.contains(&"linear".to_string()), "linear should not have if");

    assert!(else_funcs.contains(&"if_branch".to_string()), "if_branch should have else");
    assert!(!else_funcs.contains(&"nested".to_string()), "nested should not have else");
}

#[test]
fn controlflow_loop_structure() {
    let mut db = extract_c_fixture("controlflow.c");

    // Query: find functions with while loops.
    //   stmts(id, kind, location)  -- kind 3 = while
    //   while_body(while_stmt, body_id)
    //
    // has_while(func_name) :- stmts(s, 3, _), enclosingfunction(s, f), functions(f, func_name, _).

    let program = Program::new(vec![
        Rule::new(
            Atom::new("has_while", vec![var("fname")]),
            vec![
                BodyElement::Positive(Atom::new("stmts", vec![var("s"), int(3), var("_l")])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![var("s"), var("f")])),
                BodyElement::Positive(Atom::new("functions", vec![var("f"), var("fname"), var("_k")])),
            ],
        ),
    ]);

    evaluate(&program, &mut db).unwrap();

    let while_funcs = collect_strings(&db, "has_while");
    eprintln!("Functions with while: {:?}", while_funcs);

    assert!(while_funcs.contains(&"loop_cfg".to_string()), "loop_cfg should have while");
    assert!(while_funcs.contains(&"nested".to_string()), "nested should have while");
    assert!(!while_funcs.contains(&"linear".to_string()), "linear should not have while");
    assert!(!while_funcs.contains(&"if_branch".to_string()), "if_branch should not have while");
}

#[test]
fn controlflow_stmt_nesting() {
    let mut db = extract_c_fixture("controlflow.c");

    // Query: find the depth of statement nesting for each function.
    // This is a simpler version: count the number of statements per function.
    //
    // stmt_in_func(func_name, stmt_id, kind) :-
    //   stmts(stmt_id, kind, _),
    //   enclosingfunction(stmt_id, func_id),
    //   functions(func_id, func_name, _).
    //
    // stmt_count(func_name, cnt) :-
    //   functions(fid, func_name, _),
    //   cnt = count(stmts(sid, _, _), enclosingfunction(sid, fid)).

    let program = Program::new(vec![
        Rule::new(
            Atom::new("stmt_count", vec![var("fname"), var("cnt")]),
            vec![
                BodyElement::Positive(Atom::new("functions", vec![var("fid"), var("fname"), var("_k")])),
                BodyElement::Aggregate {
                    result_var: "cnt".to_string(),
                    function: AggFunction::Count,
                    sub_rule: Box::new(Rule::new(
                        Atom::new("_sub", vec![var("sid")]),
                        vec![
                            BodyElement::Positive(Atom::new("stmts", vec![var("sid"), var("_kind"), var("_loc")])),
                            BodyElement::Positive(Atom::new("enclosingfunction", vec![var("sid"), var("fid")])),
                        ],
                    )),
                    group_by: vec!["fid".to_string()],
                    agg_var: "sid".to_string(),
                },
            ],
        ),
    ]);

    evaluate(&program, &mut db).unwrap();

    let counts = db.scan("stmt_count").unwrap()
        .map(|t| {
            let name = match &t[0] { Value::String(s) => db.strings.resolve(*s).to_string(), _ => "?".into() };
            let cnt = t[1].as_int().unwrap_or(0);
            (name, cnt)
        })
        .collect::<Vec<_>>();

    eprintln!("Statement counts per function:");
    for (name, cnt) in &counts {
        eprintln!("  {}: {} stmts", name, cnt);
    }

    // nested should have the most statements (while, if, break, two assignments, return = lots)
    // linear should have fewer (3 decl stmts + block + return)
    let linear_cnt = counts.iter().find(|(n, _)| n == "linear").map(|(_, c)| *c).unwrap_or(0);
    let nested_cnt = counts.iter().find(|(n, _)| n == "nested").map(|(_, c)| *c).unwrap_or(0);
    assert!(nested_cnt > linear_cnt,
        "nested ({}) should have more stmts than linear ({})", nested_cnt, linear_cnt);
}

#[test]
fn controlflow_break_in_loop() {
    let mut db = extract_c_fixture("controlflow.c");

    // Find functions that have a break statement inside a while loop.
    //
    // has_break_in_while(func_name) :-
    //   stmts(break_id, 28, _),             -- break statement
    //   enclosingfunction(break_id, fid),
    //   stmts(while_id, 3, _),              -- while statement
    //   enclosingfunction(while_id, fid),    -- same function
    //   functions(fid, func_name, _).
    //
    // Note: this is approximate — it checks that both break and while
    // exist in the same function, not that break is inside the while.
    // For a real analysis we'd need parent-chain tracking.

    let program = Program::new(vec![
        Rule::new(
            Atom::new("has_break_in_while", vec![var("fname")]),
            vec![
                BodyElement::Positive(Atom::new("stmts", vec![var("bid"), int(28), var("_l1")])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![var("bid"), var("fid")])),
                BodyElement::Positive(Atom::new("stmts", vec![var("wid"), int(3), var("_l2")])),
                BodyElement::Positive(Atom::new("enclosingfunction", vec![var("wid"), var("fid")])),
                BodyElement::Positive(Atom::new("functions", vec![var("fid"), var("fname"), var("_k")])),
            ],
        ),
    ]);

    evaluate(&program, &mut db).unwrap();

    let funcs = collect_strings(&db, "has_break_in_while");
    eprintln!("Functions with break-in-while: {:?}", funcs);

    assert!(funcs.contains(&"nested".to_string()), "nested should have break in while");
    assert!(!funcs.contains(&"linear".to_string()), "linear should not have break in while");
    assert!(!funcs.contains(&"loop_cfg".to_string()), "loop_cfg has no break");
}
