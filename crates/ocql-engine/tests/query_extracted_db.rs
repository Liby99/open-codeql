//! End-to-end tests: extract real code, then query the database with Datalog.
//!
//! These tests exercise the full pipeline:
//!   source code -> tree-sitter -> extractor -> database -> engine -> results
//!
//! Tests are #[ignore]d by default since they require git submodules.
//! Run with:
//!   cargo test -p ocql-engine --test query_extracted_db -- --ignored

use ocql_database::{Database, Value};
use ocql_extractor_common::Extractor;
use ocql_extractor_java::{JavaExtractor, java_schema};
use ocql_engine::*;
use smallvec::smallvec;
use std::path::Path;

/// Extract a Java project and return the populated database.
fn extract_java_project(dir: &str) -> Database {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor/test-repos")
        .join(dir);
    if !path.is_dir() {
        panic!("Test repo not found: {:?}\nRun: git submodule update --init vendor/test-repos/{}", path, dir);
    }
    let schema = java_schema();
    let mut db = Database::from_schema(schema);
    let extractor = JavaExtractor::new();
    let project = extractor.extract_project(&mut db, &path, false);
    assert!(project.results.iter().all(|r| r.success), "extraction failed");
    db
}

// ============================================================
// Query 1: Find all public methods
//
// Schema:
//   methods(id, nodeName, signature, typeName, parentid, sourceid)
//   hasModifier(id1, id2)
//   modifiers(id, nodeName)
// ============================================================

#[test]
#[ignore]
fn query_public_methods() {
    let mut db = extract_java_project("jsoup");

    let public_str = db.intern_string("public");

    // public_method(mid, name) :-
    //   methods(mid, name, _sig, _type, _parent, _src),
    //   hasModifier(mid, mod_id),
    //   modifiers(mod_id, "public").
    let program = Program::new(vec![
        Rule::new(
            Atom::new("public_method", vec![var("mid"), var("name")]),
            vec![
                BodyElement::Positive(Atom::new("methods", vec![
                    var("mid"), var("name"), var("_sig"), var("_type"), var("_parent"), var("_src"),
                ])),
                BodyElement::Positive(Atom::new("hasModifier", vec![var("mid"), var("mod_id")])),
                BodyElement::Positive(Atom::new("modifiers", vec![var("mod_id"), Term::Const(public_str)])),
            ],
        ),
    ]);

    evaluate(&program, &mut db).unwrap();

    let count = db.relation("public_method").unwrap().len();
    eprintln!("Found {} public methods in jsoup", count);
    assert!(count > 100, "jsoup should have many public methods, got {}", count);
}

// ============================================================
// Query 2: Find methods with many parameters (>= 4)
//
// Schema:
//   methods(id, nodeName, signature, typeName, parentid, sourceid)
//   params(id, typeName, pos, parentid, sourceid)
// ============================================================

#[test]
#[ignore]
fn query_methods_with_many_params() {
    let mut db = extract_java_project("jsoup");

    let program = Program::new(vec![
        // param_count(callable, cnt) :-
        //   methods(callable, _n, _s, _t, _p, _src),
        //   cnt = count(params(_pid, _pt, _pos, callable, _ps)).
        Rule::new(
            Atom::new("param_count", vec![var("callable"), var("cnt")]),
            vec![
                BodyElement::Positive(Atom::new("methods", vec![
                    var("callable"), var("_n"), var("_s"), var("_t"), var("_p"), var("_src"),
                ])),
                BodyElement::Aggregate {
                    result_var: "cnt".to_string(),
                    function: AggFunction::Count,
                    sub_rule: Box::new(Rule::new(
                        Atom::new("_sub", vec![var("pid")]),
                        vec![BodyElement::Positive(Atom::new("params", vec![
                            var("pid"), var("_pt"), var("_pos"), var("callable"), var("_ps"),
                        ]))],
                    )),
                    group_by: vec!["callable".to_string()],
                    agg_var: "pid".to_string(),
                },
            ],
        ),
        // many_params(name, cnt) :-
        //   methods(callable, name, _, _, _, _),
        //   param_count(callable, cnt), cnt >= 4.
        Rule::new(
            Atom::new("many_params", vec![var("name"), var("cnt")]),
            vec![
                BodyElement::Positive(Atom::new("methods", vec![
                    var("callable"), var("name"), var("_s"), var("_t"), var("_p"), var("_src"),
                ])),
                BodyElement::Positive(Atom::new("param_count", vec![var("callable"), var("cnt")])),
                BodyElement::Guard(Guard {
                    left: var("cnt"),
                    op: CompOp::Ge,
                    right: int(4),
                }),
            ],
        ),
    ]);

    evaluate(&program, &mut db).unwrap();

    let results = db.relation("many_params").unwrap();
    eprintln!("Methods with >= 4 params in jsoup: {}", results.len());
    for tuple in results.scan() {
        let name = match &tuple[0] {
            Value::String(s) => db.strings.resolve(*s).to_string(),
            _ => format!("{:?}", tuple[0]),
        };
        let cnt = tuple[1].as_int().unwrap_or(0);
        eprintln!("  {} ({} params)", name, cnt);
    }
    assert!(results.len() > 0, "jsoup should have some methods with many params");
}

// ============================================================
// Query 3: Transitive class hierarchy (extends chain)
//
// Schema:
//   extendsReftype(id1, id2)  -- id1 extends id2
// ============================================================

#[test]
#[ignore]
fn query_class_hierarchy() {
    let mut db = extract_java_project("jsoup");

    // extends_trans(sub, super) :- extendsReftype(sub, super).
    // extends_trans(sub, super) :- extends_trans(sub, mid), extendsReftype(mid, super).
    let program = Program::new(vec![
        Rule::new(
            Atom::new("extends_trans", vec![var("sub"), var("super")]),
            vec![BodyElement::Positive(Atom::new("extendsReftype", vec![
                var("sub"), var("super"),
            ]))],
        ),
        Rule::new(
            Atom::new("extends_trans", vec![var("sub"), var("super")]),
            vec![
                BodyElement::Positive(Atom::new("extends_trans", vec![var("sub"), var("mid")])),
                BodyElement::Positive(Atom::new("extendsReftype", vec![
                    var("mid"), var("super"),
                ])),
            ],
        ),
    ]);

    evaluate(&program, &mut db).unwrap();

    let results = db.relation("extends_trans").unwrap();
    let direct = db.relation("extendsReftype").unwrap().len();
    eprintln!("Direct extends: {}, Transitive: {}", direct, results.len());
    assert!(results.len() > 0, "jsoup should have extends relationships");
    assert!(results.len() >= direct);
}

// ============================================================
// Query 4: Find empty methods (no expressions in body)
//
// Schema:
//   callableEnclosingExpr(expr_id, callable_id)
// ============================================================

#[test]
#[ignore]
fn query_empty_methods() {
    let mut db = extract_java_project("gson");

    // has_expr(callable) :- callableEnclosingExpr(_eid, callable).
    // empty_method(name) :- methods(mid, name, _, _, _, _), not has_expr(mid).
    let program = Program::new(vec![
        Rule::new(
            Atom::new("has_expr", vec![var("callable")]),
            vec![BodyElement::Positive(Atom::new("callableEnclosingExpr", vec![
                var("_eid"), var("callable"),
            ]))],
        ),
        Rule::new(
            Atom::new("empty_method", vec![var("name")]),
            vec![
                BodyElement::Positive(Atom::new("methods", vec![
                    var("mid"), var("name"), var("_s"), var("_t"), var("_p"), var("_src"),
                ])),
                BodyElement::Negated(Atom::new("has_expr", vec![var("mid")])),
            ],
        ),
    ]);

    evaluate(&program, &mut db).unwrap();

    let results = db.relation("empty_method").unwrap();
    eprintln!("Empty methods (no expressions) in gson: {}", results.len());
    let mut names: Vec<String> = results.scan()
        .map(|t| match &t[0] {
            Value::String(s) => db.strings.resolve(*s).to_string(),
            _ => format!("{:?}", t[0]),
        })
        .collect();
    names.sort();
    names.dedup();
    for name in names.iter().take(20) {
        eprintln!("  {}", name);
    }
    // There should be at least some abstract/interface methods with no body
    assert!(results.len() > 0, "gson should have some empty methods, got 0");
}

// ============================================================
// Query 5: Find classes that implement interfaces
//
// Schema:
//   classes_or_interfaces(id, nodeName, parentid, sourceid)
//   implInterface(id1, id2)
// ============================================================

#[test]
#[ignore]
fn query_interface_implementations() {
    let mut db = extract_java_project("jsoup");

    // impl_name(class_name, iface_name) :-
    //   implInterface(class_id, iface_id),
    //   classes_or_interfaces(class_id, class_name, _, _),
    //   classes_or_interfaces(iface_id, iface_name, _, _).
    let program = Program::new(vec![
        Rule::new(
            Atom::new("impl_name", vec![var("class_name"), var("iface_name")]),
            vec![
                BodyElement::Positive(Atom::new("implInterface", vec![var("class_id"), var("iface_id")])),
                BodyElement::Positive(Atom::new("classes_or_interfaces", vec![
                    var("class_id"), var("class_name"), var("_p1"), var("_s1"),
                ])),
                BodyElement::Positive(Atom::new("classes_or_interfaces", vec![
                    var("iface_id"), var("iface_name"), var("_p2"), var("_s2"),
                ])),
            ],
        ),
    ]);

    evaluate(&program, &mut db).unwrap();

    let results = db.relation("impl_name").unwrap();
    eprintln!("Interface implementations in jsoup: {}", results.len());
    for tuple in results.scan().take(20) {
        let class_name = match &tuple[0] {
            Value::String(s) => db.strings.resolve(*s).to_string(),
            _ => format!("{:?}", tuple[0]),
        };
        let iface_name = match &tuple[1] {
            Value::String(s) => db.strings.resolve(*s).to_string(),
            _ => format!("{:?}", tuple[1]),
        };
        eprintln!("  {} implements {}", class_name, iface_name);
    }
    assert!(results.len() > 0, "jsoup should have interface implementations");
}

// ============================================================
// Query 6: Hand-crafted database (no submodules needed)
// ============================================================

#[test]
fn query_hand_crafted_database() {
    let mut db = Database::empty();
    db.add_relation("functions", ocql_database::RelationSchema {
        name: "functions".to_string(),
        columns: vec![
            ocql_database::ColumnDef { name: "id".to_string(), col_type: ocql_schema::ColumnType::Int },
            ocql_database::ColumnDef { name: "name".to_string(), col_type: ocql_schema::ColumnType::String },
        ],
    });
    db.add_relation("calls", ocql_database::RelationSchema {
        name: "calls".to_string(),
        columns: vec![
            ocql_database::ColumnDef { name: "caller".to_string(), col_type: ocql_schema::ColumnType::Int },
            ocql_database::ColumnDef { name: "callee".to_string(), col_type: ocql_schema::ColumnType::Int },
        ],
    });

    let main_name = db.intern_string("main");
    let foo_name = db.intern_string("foo");
    let bar_name = db.intern_string("bar");
    let baz_name = db.intern_string("baz");

    db.insert("functions", smallvec![Value::Int(1), main_name]).unwrap();
    db.insert("functions", smallvec![Value::Int(2), foo_name]).unwrap();
    db.insert("functions", smallvec![Value::Int(3), bar_name]).unwrap();
    db.insert("functions", smallvec![Value::Int(4), baz_name]).unwrap();

    // main -> foo -> bar -> baz
    db.insert("calls", smallvec![Value::Int(1), Value::Int(2)]).unwrap();
    db.insert("calls", smallvec![Value::Int(2), Value::Int(3)]).unwrap();
    db.insert("calls", smallvec![Value::Int(3), Value::Int(4)]).unwrap();

    // calls_trans(a, b) :- calls(a, b).
    // calls_trans(a, b) :- calls_trans(a, c), calls(c, b).
    // reachable_from_main(name) :- calls_trans(1, fid), functions(fid, name).
    let program = Program::new(vec![
        Rule::new(
            Atom::new("calls_trans", vec![var("a"), var("b")]),
            vec![BodyElement::Positive(Atom::new("calls", vec![var("a"), var("b")]))],
        ),
        Rule::new(
            Atom::new("calls_trans", vec![var("a"), var("b")]),
            vec![
                BodyElement::Positive(Atom::new("calls_trans", vec![var("a"), var("c")])),
                BodyElement::Positive(Atom::new("calls", vec![var("c"), var("b")])),
            ],
        ),
        Rule::new(
            Atom::new("reachable_from_main", vec![var("name")]),
            vec![
                BodyElement::Positive(Atom::new("calls_trans", vec![int(1), var("fid")])),
                BodyElement::Positive(Atom::new("functions", vec![var("fid"), var("name")])),
            ],
        ),
    ]);

    evaluate(&program, &mut db).unwrap();

    assert_eq!(db.relation("calls_trans").unwrap().len(), 6);

    let reachable = db.relation("reachable_from_main").unwrap();
    assert_eq!(reachable.len(), 3);

    let names: Vec<String> = reachable.scan()
        .map(|t| match &t[0] {
            Value::String(s) => db.strings.resolve(*s).to_string(),
            _ => format!("{:?}", t[0]),
        })
        .collect();
    assert!(names.contains(&"foo".to_string()));
    assert!(names.contains(&"bar".to_string()));
    assert!(names.contains(&"baz".to_string()));
}
