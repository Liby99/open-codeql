//! Java parity tests: verify our extraction and query results match CodeQL.
//!
//! These tests extract Java files and run raw-table QL queries to verify
//! that our extractor produces the same relational facts as CodeQL's extractor.
//!
//! Run with: cargo test -p ocql-e2e-tests --test java_parity -- --nocapture

use std::collections::BTreeSet;

use ocql_database::{Database, Value};
use ocql_extractor_common::Extractor;
use ocql_extractor_java::{JavaExtractor, java_schema};
use ocql_mir::compile_ql_to_engine;
use ocql_engine::evaluate;

fn fixture_path(filename: &str) -> String {
    format!(
        "{}/../../tests/java-comparison/projects/{}",
        env!("CARGO_MANIFEST_DIR"),
        filename
    )
}

fn extract_java_file(filename: &str) -> Database {
    let path = fixture_path(filename);
    let source = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path, e));
    let schema = java_schema();
    let mut db = Database::from_schema(schema);
    let extractor = JavaExtractor::new();
    let result = extractor.extract_source(&mut db, &path, &source);
    assert!(result.success, "Java extraction failed: {:?}", result.error);
    db
}

fn eval_ql(ql: &str, db: &mut Database) {
    let mut program = compile_ql_to_engine(ql).expect("compile_ql_to_engine failed");
    program.resolve_strings(db);
    evaluate(&program, db).expect("evaluate failed");
}

fn collect_strings(db: &Database, relation: &str, col: usize) -> BTreeSet<String> {
    db.scan(relation)
        .map(|iter| {
            iter.filter_map(|row| match &row[col] {
                Value::String(s) => Some(db.strings.resolve(*s).to_string()),
                _ => None,
            })
            .collect()
        })
        .unwrap_or_default()
}

fn collect_string_pairs(db: &Database, relation: &str, col1: usize, col2: usize) -> BTreeSet<(String, String)> {
    db.scan(relation)
        .map(|iter| {
            iter.filter_map(|row| {
                let a = match &row[col1] {
                    Value::String(s) => db.strings.resolve(*s).to_string(),
                    _ => return None,
                };
                let b = match &row[col2] {
                    Value::String(s) => db.strings.resolve(*s).to_string(),
                    _ => return None,
                };
                Some((a, b))
            })
            .collect()
        })
        .unwrap_or_default()
}

fn count_rows(db: &Database, table: &str) -> usize {
    db.scan(table).map(|i| i.count()).unwrap_or(0)
}

// =============================================================================
// BasicStructure.java tests
// =============================================================================

#[test]
fn parity_basic_classes() {
    let db = extract_java_file("BasicStructure.java");
    let classes = collect_strings(&db, "classes_or_interfaces", 1);
    eprintln!("Classes: {:?}", classes);

    // Expected: Printable, Animal, Dog, Cat, Color, StringUtils, BasicStructure
    assert!(classes.contains("Printable"), "missing Printable");
    assert!(classes.contains("Animal"), "missing Animal");
    assert!(classes.contains("Dog"), "missing Dog");
    assert!(classes.contains("Cat"), "missing Cat");
    assert!(classes.contains("Color"), "missing Color");
    assert!(classes.contains("StringUtils"), "missing StringUtils");
    assert!(classes.contains("BasicStructure"), "missing BasicStructure");
    assert_eq!(classes.len(), 7, "expected 7 classes/interfaces, got {:?}", classes);
}

#[test]
fn parity_basic_interfaces() {
    let db = extract_java_file("BasicStructure.java");

    // Check isInterface marker
    let class_ids: std::collections::HashMap<i64, String> = db.scan("classes_or_interfaces")
        .unwrap()
        .map(|row| {
            let id = match &row[0] { Value::Entity(e) => e.0 as i64, Value::Int(i) => *i, _ => -1 };
            let name = match &row[1] { Value::String(s) => db.strings.resolve(*s).to_string(), _ => String::new() };
            (id, name)
        })
        .collect();

    let interface_ids: BTreeSet<i64> = db.scan("isInterface")
        .map(|iter| iter.map(|row| match &row[0] { Value::Entity(e) => e.0 as i64, Value::Int(i) => *i, _ => -1 }).collect())
        .unwrap_or_default();

    let interfaces: BTreeSet<String> = interface_ids.iter()
        .filter_map(|id| class_ids.get(id).cloned())
        .collect();

    eprintln!("Interfaces: {:?}", interfaces);
    assert!(interfaces.contains("Printable"), "Printable should be an interface");
}

#[test]
fn parity_basic_inheritance() {
    let db = extract_java_file("BasicStructure.java");

    // Build ID→name map
    let id_to_name: std::collections::HashMap<i64, String> = db.scan("classes_or_interfaces")
        .unwrap()
        .map(|row| {
            let id = match &row[0] { Value::Entity(e) => e.0 as i64, Value::Int(i) => *i, _ => -1 };
            let name = match &row[1] { Value::String(s) => db.strings.resolve(*s).to_string(), _ => String::new() };
            (id, name)
        })
        .collect();

    // extendsReftype: sub → super
    let extends: BTreeSet<(String, String)> = db.scan("extendsReftype")
        .map(|iter| {
            iter.filter_map(|row| {
                let sub_id = match &row[0] { Value::Entity(e) => e.0 as i64, Value::Int(i) => *i, _ => return None };
                let sup_id = match &row[1] { Value::Entity(e) => e.0 as i64, Value::Int(i) => *i, _ => return None };
                let sub = id_to_name.get(&sub_id)?;
                let sup = id_to_name.get(&sup_id)?;
                Some((sub.clone(), sup.clone()))
            })
            .collect()
        })
        .unwrap_or_default();

    eprintln!("Extends: {:?}", extends);
    assert!(extends.contains(&("Dog".into(), "Animal".into())), "Dog should extend Animal");
    assert!(extends.contains(&("Cat".into(), "Animal".into())), "Cat should extend Animal");

    // implInterface: class → interface
    let impls: BTreeSet<(String, String)> = db.scan("implInterface")
        .map(|iter| {
            iter.filter_map(|row| {
                let cls_id = match &row[0] { Value::Entity(e) => e.0 as i64, Value::Int(i) => *i, _ => return None };
                let ifc_id = match &row[1] { Value::Entity(e) => e.0 as i64, Value::Int(i) => *i, _ => return None };
                let cls = id_to_name.get(&cls_id)?;
                let ifc = id_to_name.get(&ifc_id)?;
                Some((cls.clone(), ifc.clone()))
            })
            .collect()
        })
        .unwrap_or_default();

    eprintln!("Implements: {:?}", impls);
    assert!(impls.contains(&("Dog".into(), "Printable".into())), "Dog should implement Printable");
    assert!(impls.contains(&("Cat".into(), "Printable".into())), "Cat should implement Printable");
}

#[test]
fn parity_basic_methods() {
    let db = extract_java_file("BasicStructure.java");

    // Collect (class_name, method_name) pairs
    let class_ids: std::collections::HashMap<i64, String> = db.scan("classes_or_interfaces")
        .unwrap()
        .map(|row| {
            let id = match &row[0] { Value::Entity(e) => e.0 as i64, Value::Int(i) => *i, _ => -1 };
            let name = match &row[1] { Value::String(s) => db.strings.resolve(*s).to_string(), _ => String::new() };
            (id, name)
        })
        .collect();

    let methods: BTreeSet<(String, String)> = db.scan("methods")
        .map(|iter| {
            iter.filter_map(|row| {
                let mname = match &row[1] { Value::String(s) => db.strings.resolve(*s).to_string(), _ => return None };
                let cid = match &row[4] { Value::Entity(e) => e.0 as i64, Value::Int(i) => *i, _ => return None };
                let cname = class_ids.get(&cid)?;
                Some((cname.clone(), mname))
            })
            .collect()
        })
        .unwrap_or_default();

    eprintln!("Methods ({}):", methods.len());
    for (c, m) in &methods {
        eprintln!("  {}.{}", c, m);
    }

    // Verify key methods exist
    assert!(methods.contains(&("Animal".into(), "getName".into())));
    assert!(methods.contains(&("Animal".into(), "getAge".into())));
    assert!(methods.contains(&("Animal".into(), "speak".into())));
    assert!(methods.contains(&("Animal".into(), "toString".into())));
    assert!(methods.contains(&("Dog".into(), "getBreed".into())));
    assert!(methods.contains(&("Dog".into(), "speak".into())));
    assert!(methods.contains(&("Dog".into(), "print".into())));
    assert!(methods.contains(&("Cat".into(), "isIndoor".into())));
    assert!(methods.contains(&("Cat".into(), "speak".into())));
    assert!(methods.contains(&("Cat".into(), "print".into())));
    assert!(methods.contains(&("Color".into(), "lower".into())));
    assert!(methods.contains(&("StringUtils".into(), "isEmpty".into())));
    assert!(methods.contains(&("StringUtils".into(), "repeat".into())));
    assert!(methods.contains(&("BasicStructure".into(), "addAnimal".into())));
    assert!(methods.contains(&("BasicStructure".into(), "getCount".into())));
    assert!(methods.contains(&("BasicStructure".into(), "findByName".into())));
    assert!(methods.contains(&("BasicStructure".into(), "main".into())));
    assert!(methods.contains(&("Printable".into(), "print".into())));
    assert!(methods.contains(&("Printable".into(), "format".into())));
}

#[test]
fn parity_basic_fields() {
    let db = extract_java_file("BasicStructure.java");

    let class_ids: std::collections::HashMap<i64, String> = db.scan("classes_or_interfaces")
        .unwrap()
        .map(|row| {
            let id = match &row[0] { Value::Entity(e) => e.0 as i64, Value::Int(i) => *i, _ => -1 };
            let name = match &row[1] { Value::String(s) => db.strings.resolve(*s).to_string(), _ => String::new() };
            (id, name)
        })
        .collect();

    let fields: BTreeSet<(String, String)> = db.scan("fields")
        .map(|iter| {
            iter.filter_map(|row| {
                let fname = match &row[1] { Value::String(s) => db.strings.resolve(*s).to_string(), _ => return None };
                let cid = match &row[3] { Value::Entity(e) => e.0 as i64, Value::Int(i) => *i, _ => return None };
                let cname = class_ids.get(&cid)?;
                Some((cname.clone(), fname))
            })
            .collect()
        })
        .unwrap_or_default();

    eprintln!("Fields: {:?}", fields);
    assert!(fields.contains(&("Animal".into(), "name".into())));
    assert!(fields.contains(&("Animal".into(), "age".into())));
    assert!(fields.contains(&("Dog".into(), "breed".into())));
    assert!(fields.contains(&("Cat".into(), "indoor".into())));
    assert!(fields.contains(&("BasicStructure".into(), "animals".into())));
}

#[test]
fn parity_basic_constructors() {
    let db = extract_java_file("BasicStructure.java");

    let class_ids: std::collections::HashMap<i64, String> = db.scan("classes_or_interfaces")
        .unwrap()
        .map(|row| {
            let id = match &row[0] { Value::Entity(e) => e.0 as i64, Value::Int(i) => *i, _ => -1 };
            let name = match &row[1] { Value::String(s) => db.strings.resolve(*s).to_string(), _ => String::new() };
            (id, name)
        })
        .collect();

    let constructors: BTreeSet<String> = db.scan("constrs")
        .map(|iter| {
            iter.filter_map(|row| {
                let cid = match &row[4] { Value::Entity(e) => e.0 as i64, Value::Int(i) => *i, _ => return None };
                class_ids.get(&cid).cloned()
            })
            .collect()
        })
        .unwrap_or_default();

    eprintln!("Classes with constructors: {:?}", constructors);
    assert!(constructors.contains("Animal"), "Animal should have a constructor");
    assert!(constructors.contains("Dog"), "Dog should have a constructor");
    assert!(constructors.contains("Cat"), "Cat should have a constructor");
    assert!(constructors.contains("StringUtils"), "StringUtils should have an implicit default constructor");
    assert!(constructors.contains("BasicStructure"), "BasicStructure should have an implicit default constructor");
}

#[test]
fn parity_basic_modifiers() {
    let mut db = extract_java_file("BasicStructure.java");

    // Use QL evaluation to test modifiers (avoids entity/int type mismatch in raw scans)
    eval_ql(r#"
        predicate abstractMethod(string methodName) {
            methods(mid, methodName, _, _, _, _) and
            hasModifier(mid, mod) and
            modifiers(mod, "abstract")
        }
        from string m
        where abstractMethod(m)
        select m
    "#, &mut db);

    let abstract_methods = collect_strings(&db, "abstractMethod", 0);
    eprintln!("Abstract methods: {:?}", abstract_methods);
    assert!(abstract_methods.contains("speak"), "speak should be abstract");
}

// =============================================================================
// ControlFlow.java tests
// =============================================================================

#[test]
fn parity_controlflow_methods() {
    let db = extract_java_file("ControlFlow.java");
    let methods: BTreeSet<String> = db.scan("methods")
        .map(|iter| {
            iter.filter_map(|row| match &row[1] {
                Value::String(s) => Some(db.strings.resolve(*s).to_string()),
                _ => None,
            })
            .collect()
        })
        .unwrap_or_default();

    eprintln!("ControlFlow methods: {:?}", methods);
    let expected = [
        "classify", "dayType", "sumFor", "sumWhile", "sumDoWhile",
        "sumEnhancedFor", "firstNegative", "findInMatrix",
        "safeDivide", "parseValue", "validate", "sqrt", "main",
    ];
    for name in expected {
        assert!(methods.contains(name), "missing method: {}", name);
    }
}

#[test]
fn parity_controlflow_statement_types() {
    let db = extract_java_file("ControlFlow.java");

    // Count statement kinds
    let mut kind_counts: std::collections::BTreeMap<i64, usize> = std::collections::BTreeMap::new();
    if let Some(iter) = db.scan("stmts") {
        for row in iter {
            if let Value::Int(kind) = &row[1] {
                *kind_counts.entry(*kind).or_insert(0) += 1;
            }
        }
    }

    eprintln!("Statement kinds: {:?}", kind_counts);

    // Should have: blocks (0), if (1), for (2), enhanced_for (3), while (4), do (5),
    // try (6), switch (7), return (9), throw (10), break (11), continue (12),
    // expr_stmt (14), labeled (15), assert (16), local_var_decl (17), catch (22)
    assert!(kind_counts.contains_key(&0), "should have block stmts");
    assert!(kind_counts.contains_key(&1), "should have if stmts");
    assert!(kind_counts.contains_key(&2), "should have for stmts");
    assert!(kind_counts.contains_key(&4), "should have while stmts");
    assert!(kind_counts.contains_key(&5), "should have do-while stmts");
    assert!(kind_counts.contains_key(&6), "should have try stmts");
    assert!(kind_counts.contains_key(&7), "should have switch stmts");
    assert!(kind_counts.contains_key(&9), "should have return stmts");
    assert!(kind_counts.contains_key(&10), "should have throw stmts");
    assert!(kind_counts.contains_key(&11), "should have break stmts");
    assert!(kind_counts.contains_key(&12), "should have continue stmts");
}

// =============================================================================
// CallGraph.java tests
// =============================================================================

#[test]
fn parity_callgraph_classes() {
    let db = extract_java_file("CallGraph.java");
    let classes = collect_strings(&db, "classes_or_interfaces", 1);
    eprintln!("CallGraph classes: {:?}", classes);

    let expected = [
        "Logger", "ConsoleLogger", "FileLogger", "Processor",
        "UpperCaseProcessor", "ReverseProcessor", "Pipeline",
        "Base", "Derived", "MathHelper", "CallGraph",
    ];
    for name in expected {
        assert!(classes.contains(name), "missing class: {}", name);
    }
    assert_eq!(classes.len(), expected.len(), "unexpected class count");
}

#[test]
fn parity_callgraph_inheritance() {
    let db = extract_java_file("CallGraph.java");

    let id_to_name: std::collections::HashMap<i64, String> = db.scan("classes_or_interfaces")
        .unwrap()
        .map(|row| {
            let id = match &row[0] { Value::Entity(e) => e.0 as i64, Value::Int(i) => *i, _ => -1 };
            let name = match &row[1] { Value::String(s) => db.strings.resolve(*s).to_string(), _ => String::new() };
            (id, name)
        })
        .collect();

    let extends: BTreeSet<(String, String)> = db.scan("extendsReftype")
        .map(|iter| {
            iter.filter_map(|row| {
                let sub = match &row[0] { Value::Entity(e) => e.0 as i64, Value::Int(i) => *i, _ => return None };
                let sup = match &row[1] { Value::Entity(e) => e.0 as i64, Value::Int(i) => *i, _ => return None };
                Some((id_to_name.get(&sub)?.clone(), id_to_name.get(&sup)?.clone()))
            })
            .collect()
        })
        .unwrap_or_default();

    eprintln!("CallGraph extends: {:?}", extends);
    assert!(extends.contains(&("UpperCaseProcessor".into(), "Processor".into())));
    assert!(extends.contains(&("ReverseProcessor".into(), "Processor".into())));
    assert!(extends.contains(&("Derived".into(), "Base".into())));

    let impls: BTreeSet<(String, String)> = db.scan("implInterface")
        .map(|iter| {
            iter.filter_map(|row| {
                let cls = match &row[0] { Value::Entity(e) => e.0 as i64, Value::Int(i) => *i, _ => return None };
                let ifc = match &row[1] { Value::Entity(e) => e.0 as i64, Value::Int(i) => *i, _ => return None };
                Some((id_to_name.get(&cls)?.clone(), id_to_name.get(&ifc)?.clone()))
            })
            .collect()
        })
        .unwrap_or_default();

    eprintln!("CallGraph implements: {:?}", impls);
    assert!(impls.contains(&("ConsoleLogger".into(), "Logger".into())));
    assert!(impls.contains(&("FileLogger".into(), "Logger".into())));
}

#[test]
fn parity_callgraph_call_bindings() {
    let db = extract_java_file("CallGraph.java");

    let binding_count = count_rows(&db, "callableBinding");
    eprintln!("callableBinding rows: {}", binding_count);

    assert!(binding_count > 0, "callableBinding should have rows after resolve_call_bindings");

    // Verify some call targets are resolved
    let method_names: std::collections::HashMap<i64, String> = db.scan("methods")
        .map(|iter| {
            iter.map(|row| {
                let id = match &row[0] { Value::Entity(e) => e.0 as i64, Value::Int(i) => *i, _ => -1 };
                let name = match &row[1] { Value::String(s) => db.strings.resolve(*s).to_string(), _ => String::new() };
                (id, name)
            })
            .collect()
        })
        .unwrap_or_default();

    let call_targets: BTreeSet<String> = db.scan("callableBinding")
        .map(|iter| {
            iter.filter_map(|row| {
                let target = match &row[1] { Value::Entity(e) => e.0 as i64, Value::Int(i) => *i, _ => return None };
                method_names.get(&target).cloned()
            })
            .collect()
        })
        .unwrap_or_default();

    eprintln!("Call targets: {:?}", call_targets);
    // Should resolve at least some method calls
    assert!(!call_targets.is_empty(), "should resolve some call targets");
}

#[test]
fn parity_callgraph_variable_bindings() {
    let db = extract_java_file("CallGraph.java");

    let binding_count = count_rows(&db, "variableBinding");
    eprintln!("variableBinding rows: {}", binding_count);

    assert!(binding_count > 0, "variableBinding should have rows after resolve_variable_bindings");
}

#[test]
fn parity_basic_default_constructors() {
    let db = extract_java_file("BasicStructure.java");

    // Check isDefConstr marks the implicit default constructors
    let def_constr_count = count_rows(&db, "isDefConstr");
    eprintln!("isDefConstr rows: {}", def_constr_count);

    // BasicStructure has no explicit constructor → gets a default
    // (StringUtils has an explicit private constructor, Printable is an interface)
    assert!(def_constr_count >= 1, "should have at least 1 default constructor (BasicStructure), got {}", def_constr_count);
}

// =============================================================================
// SecurityPatterns.java tests
// =============================================================================

#[test]
fn parity_security_classes() {
    let db = extract_java_file("SecurityPatterns.java");
    let classes = collect_strings(&db, "classes_or_interfaces", 1);
    eprintln!("SecurityPatterns classes: {:?}", classes);
    assert!(classes.contains("SecurityPatterns"));
}

#[test]
fn parity_security_methods() {
    let db = extract_java_file("SecurityPatterns.java");
    let methods: BTreeSet<String> = db.scan("methods")
        .map(|iter| {
            iter.filter_map(|row| match &row[1] {
                Value::String(s) => Some(db.strings.resolve(*s).to_string()),
                _ => None,
            })
            .collect()
        })
        .unwrap_or_default();

    eprintln!("SecurityPatterns methods: {:?}", methods);

    let expected = [
        "unsafeQuery", "safeQuery", "unsafeExec", "unsafeProcessBuilder",
        "readFile", "readPassword", "weakHash", "unsafeMultiply",
        "unsafeNullDeref", "deadCode", "riskyLoop", "swallowException",
        "leakyRead", "main",
    ];
    for name in expected {
        assert!(methods.contains(name), "missing method: {}", name);
    }
}

#[test]
fn parity_security_table_counts() {
    let db = extract_java_file("SecurityPatterns.java");

    let tables = [
        "classes_or_interfaces", "methods", "constrs", "fields",
        "stmts", "exprs", "params", "modifiers", "hasModifier",
        "callableBinding",
    ];
    for table in tables {
        let count = count_rows(&db, table);
        eprintln!("  {}: {} rows", table, count);
        // fields=0 is expected for SecurityPatterns.java (no field declarations, only local vars)
        if table == "fields" && count == 0 {
            eprintln!("    NOTE: {table}=0 (expected for this file)");
        } else {
            assert!(count > 0, "{} should have rows", table);
        }
    }
}

// =============================================================================
// Cross-file: QL query evaluation tests
// =============================================================================

#[test]
fn parity_ql_public_methods() {
    let mut db = extract_java_file("BasicStructure.java");
    eval_ql(r#"
        predicate publicMethod(string className, string methodName) {
            methods(mid, methodName, _, _, cid, _) and
            classes_or_interfaces(cid, className, _, _) and
            hasModifier(mid, mod) and
            modifiers(mod, "public")
        }
        from string c, string m
        where publicMethod(c, m)
        select c, m
    "#, &mut db);

    let results = collect_string_pairs(&db, "publicMethod", 0, 1);
    eprintln!("Public methods: {:?}", results);

    assert!(results.contains(&("Animal".into(), "getName".into())));
    assert!(results.contains(&("Animal".into(), "getAge".into())));
    assert!(results.contains(&("Dog".into(), "getBreed".into())));
    assert!(results.contains(&("Dog".into(), "speak".into())));
    assert!(results.contains(&("Dog".into(), "print".into())));
    assert!(results.contains(&("Cat".into(), "isIndoor".into())));
    assert!(results.contains(&("BasicStructure".into(), "main".into())));
    assert!(results.contains(&("StringUtils".into(), "isEmpty".into())));
}

#[test]
fn parity_ql_static_methods() {
    let mut db = extract_java_file("BasicStructure.java");
    eval_ql(r#"
        predicate staticMethod(string className, string methodName) {
            methods(mid, methodName, _, _, cid, _) and
            classes_or_interfaces(cid, className, _, _) and
            hasModifier(mid, mod) and
            modifiers(mod, "static")
        }
        from string c, string m
        where staticMethod(c, m)
        select c, m
    "#, &mut db);

    let results = collect_string_pairs(&db, "staticMethod", 0, 1);
    eprintln!("Static methods: {:?}", results);

    assert!(results.contains(&("StringUtils".into(), "isEmpty".into())));
    assert!(results.contains(&("StringUtils".into(), "repeat".into())));
    assert!(results.contains(&("BasicStructure".into(), "main".into())));
}

#[test]
fn parity_ql_abstract_methods() {
    let mut db = extract_java_file("BasicStructure.java");
    eval_ql(r#"
        predicate abstractMethod(string className, string methodName) {
            methods(mid, methodName, _, _, cid, _) and
            classes_or_interfaces(cid, className, _, _) and
            hasModifier(mid, mod) and
            modifiers(mod, "abstract")
        }
        from string c, string m
        where abstractMethod(c, m)
        select c, m
    "#, &mut db);

    let results = collect_string_pairs(&db, "abstractMethod", 0, 1);
    eprintln!("Abstract methods: {:?}", results);

    assert!(results.contains(&("Animal".into(), "speak".into())));
}

#[test]
fn parity_ql_private_fields() {
    let mut db = extract_java_file("BasicStructure.java");
    eval_ql(r#"
        predicate privateField(string className, string fieldName) {
            fields(fid, fieldName, _, cid) and
            classes_or_interfaces(cid, className, _, _) and
            hasModifier(fid, mod) and
            modifiers(mod, "private")
        }
        from string c, string f
        where privateField(c, f)
        select c, f
    "#, &mut db);

    let results = collect_string_pairs(&db, "privateField", 0, 1);
    eprintln!("Private fields: {:?}", results);

    assert!(results.contains(&("Animal".into(), "name".into())));
    assert!(results.contains(&("Dog".into(), "breed".into())));
    assert!(results.contains(&("Cat".into(), "indoor".into())));
    assert!(results.contains(&("BasicStructure".into(), "animals".into())));
}
