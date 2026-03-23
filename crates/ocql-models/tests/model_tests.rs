//! Tests for model file parsing, access paths, and database loading.

use std::collections::HashSet;
use std::path::Path;

use ocql_database::{Database, Value};
use ocql_models::*;
use ocql_models::access_path::parse_access_path;

// ============================================================
// Parse tests: inline YAML
// ============================================================

#[test]
fn parse_source_model_yaml() {
    let yaml = r#"
extensions:
  - addsTo:
      pack: codeql/java-all
      extensible: sourceModel
    data:
      - ["java.net", "Socket", false, "getInputStream", "()", "", "ReturnValue", "remote", "manual"]
      - ["android.content", "Context", true, "getExternalFilesDir", "(String)", "", "ReturnValue", "android-external-storage-dir", "manual"]
"#;

    let store = parse_model_file(yaml).unwrap();
    assert_eq!(store.sources.len(), 2);

    let s0 = &store.sources[0];
    assert_eq!(s0.callable.package, "java.net");
    assert_eq!(s0.callable.type_name, "Socket");
    assert!(!s0.callable.subtypes);
    assert_eq!(s0.callable.name, "getInputStream");
    assert_eq!(s0.callable.signature, "()");
    assert_eq!(s0.output.root, AccessPathRoot::ReturnValue(None));
    assert_eq!(s0.kind, "remote");
    assert_eq!(s0.provenance, "manual");

    let s1 = &store.sources[1];
    assert!(s1.callable.subtypes);
    assert_eq!(s1.kind, "android-external-storage-dir");
}

#[test]
fn parse_sink_model_yaml() {
    let yaml = r#"
extensions:
  - addsTo:
      pack: codeql/java-all
      extensible: sinkModel
    data:
      - ["java.sql", "Statement", true, "execute", "(String)", "", "Argument[0]", "sql-injection", "manual"]
"#;

    let store = parse_model_file(yaml).unwrap();
    assert_eq!(store.sinks.len(), 1);

    let s = &store.sinks[0];
    assert_eq!(s.callable.package, "java.sql");
    assert_eq!(s.callable.type_name, "Statement");
    assert!(s.callable.subtypes);
    assert_eq!(s.callable.name, "execute");
    assert_eq!(s.input.root, AccessPathRoot::Argument(ArgumentSpec::Index(0)));
    assert_eq!(s.kind, "sql-injection");
}

#[test]
fn parse_summary_model_yaml() {
    let yaml = r#"
extensions:
  - addsTo:
      pack: codeql/java-all
      extensible: summaryModel
    data:
      - ["java.lang", "String", false, "concat", "(String)", "", "Argument[this]", "ReturnValue", "taint", "manual"]
      - ["java.lang", "String", false, "concat", "(String)", "", "Argument[0]", "ReturnValue", "taint", "manual"]
"#;

    let store = parse_model_file(yaml).unwrap();
    assert_eq!(store.summaries.len(), 2);

    let s0 = &store.summaries[0];
    assert_eq!(s0.input.root, AccessPathRoot::Argument(ArgumentSpec::This));
    assert_eq!(s0.output.root, AccessPathRoot::ReturnValue(None));
    assert_eq!(s0.kind, "taint");

    let s1 = &store.summaries[1];
    assert_eq!(s1.input.root, AccessPathRoot::Argument(ArgumentSpec::Index(0)));
}

#[test]
fn parse_neutral_model_yaml() {
    let yaml = r#"
extensions:
  - addsTo:
      pack: codeql/java-all
      extensible: neutralModel
    data:
      - ["java.io", "File", "delete", "()", "summary", "manual"]
      - ["java.io", "File", "exists", "()", "summary", "manual"]
"#;

    let store = parse_model_file(yaml).unwrap();
    assert_eq!(store.neutrals.len(), 2);
    assert_eq!(store.neutrals[0].name, "delete");
    assert_eq!(store.neutrals[1].name, "exists");
}

#[test]
fn parse_mixed_model_file() {
    let yaml = r#"
extensions:
  - addsTo:
      pack: codeql/java-all
      extensible: sourceModel
    data:
      - ["java.net", "Socket", false, "getInputStream", "()", "", "ReturnValue", "remote", "manual"]
  - addsTo:
      pack: codeql/java-all
      extensible: sinkModel
    data:
      - ["java.sql", "Statement", true, "execute", "(String)", "", "Argument[0]", "sql-injection", "manual"]
  - addsTo:
      pack: codeql/java-all
      extensible: summaryModel
    data:
      - ["java.lang", "String", false, "concat", "(String)", "", "Argument[0]", "ReturnValue", "taint", "manual"]
"#;

    let store = parse_model_file(yaml).unwrap();
    assert_eq!(store.sources.len(), 1);
    assert_eq!(store.sinks.len(), 1);
    assert_eq!(store.summaries.len(), 1);
    assert_eq!(store.total_models(), 3);
}

#[test]
fn parse_complex_access_paths_in_summary() {
    let yaml = r#"
extensions:
  - addsTo:
      pack: codeql/java-all
      extensible: summaryModel
    data:
      - ["android.content", "Intent", true, "putExtra", "", "", "Argument[1]", "Argument[this].SyntheticField[android.content.Intent.extras].MapValue", "value", "manual"]
"#;

    let store = parse_model_file(yaml).unwrap();
    assert_eq!(store.summaries.len(), 1);

    let s = &store.summaries[0];
    assert_eq!(s.input.root, AccessPathRoot::Argument(ArgumentSpec::Index(1)));
    assert_eq!(s.output.root, AccessPathRoot::Argument(ArgumentSpec::This));
    assert_eq!(s.output.components.len(), 2);
    assert_eq!(
        s.output.components[0],
        AccessPathComponent::SyntheticField("android.content.Intent.extras".into())
    );
    assert_eq!(s.output.components[1], AccessPathComponent::MapValue);
}

// ============================================================
// Tests: ModelStore queries
// ============================================================

#[test]
fn model_store_queries() {
    let yaml = r#"
extensions:
  - addsTo:
      pack: codeql/java-all
      extensible: sourceModel
    data:
      - ["java.net", "Socket", false, "getInputStream", "()", "", "ReturnValue", "remote", "manual"]
      - ["java.net", "ServerSocket", false, "accept", "()", "", "ReturnValue", "remote", "manual"]
      - ["android.content", "Context", true, "getExternalFilesDir", "(String)", "", "ReturnValue", "file", "manual"]
  - addsTo:
      pack: codeql/java-all
      extensible: sinkModel
    data:
      - ["java.sql", "Statement", true, "execute", "(String)", "", "Argument[0]", "sql-injection", "manual"]
      - ["java.sql", "Statement", true, "executeQuery", "(String)", "", "Argument[0]", "sql-injection", "manual"]
"#;

    let store = parse_model_file(yaml).unwrap();

    let remote = store.sources_by_kind("remote");
    assert_eq!(remote.len(), 2);

    let file = store.sources_by_kind("file");
    assert_eq!(file.len(), 1);

    let sqli = store.sinks_by_kind("sql-injection");
    assert_eq!(sqli.len(), 2);
}

#[test]
fn model_store_merge() {
    let yaml1 = r#"
extensions:
  - addsTo:
      pack: codeql/java-all
      extensible: sourceModel
    data:
      - ["java.net", "Socket", false, "getInputStream", "()", "", "ReturnValue", "remote", "manual"]
"#;
    let yaml2 = r#"
extensions:
  - addsTo:
      pack: codeql/java-all
      extensible: sinkModel
    data:
      - ["java.sql", "Statement", true, "execute", "(String)", "", "Argument[0]", "sql-injection", "manual"]
"#;

    let mut store = parse_model_file(yaml1).unwrap();
    let store2 = parse_model_file(yaml2).unwrap();
    store.merge(store2);

    assert_eq!(store.sources.len(), 1);
    assert_eq!(store.sinks.len(), 1);
    assert_eq!(store.total_models(), 2);
}

#[test]
fn callable_spec_qualified_name() {
    let c = CallableSpec {
        package: "java.sql".into(),
        type_name: "Statement".into(),
        subtypes: true,
        name: "execute".into(),
        signature: "(String)".into(),
    };
    assert_eq!(c.qualified_name(), "java.sql.Statement.execute");

    let c2 = CallableSpec {
        package: "os/exec".into(),
        type_name: String::new(),
        subtypes: false,
        name: "Command".into(),
        signature: String::new(),
    };
    assert_eq!(c2.qualified_name(), "os/exec.Command");
}

// ============================================================
// Tests: Database loading
// ============================================================

#[test]
fn load_models_into_database() {
    let yaml = r#"
extensions:
  - addsTo:
      pack: codeql/java-all
      extensible: sourceModel
    data:
      - ["java.net", "Socket", false, "getInputStream", "()", "", "ReturnValue", "remote", "manual"]
  - addsTo:
      pack: codeql/java-all
      extensible: sinkModel
    data:
      - ["java.sql", "Statement", true, "execute", "(String)", "", "Argument[0]", "sql-injection", "manual"]
  - addsTo:
      pack: codeql/java-all
      extensible: summaryModel
    data:
      - ["java.lang", "String", false, "concat", "(String)", "", "Argument[0]", "ReturnValue", "taint", "manual"]
"#;

    let store = parse_model_file(yaml).unwrap();
    let mut db = Database::empty();
    load_models_into_db(&store, &mut db);

    // Check sourceModel relation
    let sources: Vec<_> = db.scan("sourceModel").unwrap().collect();
    assert_eq!(sources.len(), 1);

    // Check sinkModel relation
    let sinks: Vec<_> = db.scan("sinkModel").unwrap().collect();
    assert_eq!(sinks.len(), 1);

    // Check summaryModel relation
    let summaries: Vec<_> = db.scan("summaryModel").unwrap().collect();
    assert_eq!(summaries.len(), 1);
}

#[test]
fn query_models_with_datalog() {
    let yaml = r#"
extensions:
  - addsTo:
      pack: codeql/java-all
      extensible: sinkModel
    data:
      - ["java.sql", "Statement", true, "execute", "(String)", "", "Argument[0]", "sql-injection", "manual"]
      - ["java.sql", "Statement", true, "executeQuery", "(String)", "", "Argument[0]", "sql-injection", "manual"]
      - ["java.io", "FileOutputStream", true, "write", "(byte[])", "", "Argument[0]", "path-injection", "manual"]
"#;

    let store = parse_model_file(yaml).unwrap();
    let mut db = Database::empty();
    load_models_into_db(&store, &mut db);

    // Query: find all SQL injection sinks
    let mut program = ocql_engine::parse_program(r#"
        sqli_sink(name) :-
            sinkModel(_pkg, _type, _sub, name, _sig, _input, "sql-injection", _prov).
    "#).unwrap();
    program.resolve_strings(&mut db);
    ocql_engine::evaluate(&program, &mut db).unwrap();

    let names: HashSet<String> = db.scan("sqli_sink").unwrap()
        .map(|t| match &t[0] {
            Value::String(s) => db.strings.resolve(*s).to_string(),
            _ => panic!(),
        })
        .collect();

    assert!(names.contains("execute"));
    assert!(names.contains("executeQuery"));
    assert!(!names.contains("write")); // path-injection, not sql-injection
}

// ============================================================
// Tests: Real vendor model files
// ============================================================

#[test]
fn parse_android_app_model() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vendor/codeql/java/ql/lib/ext/android.app.model.yml"
    );
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return, // skip if vendor not checked out
    };

    let store = parse_model_file(&content).unwrap();
    assert!(store.total_models() > 0, "should parse some models");
    // Verify we got at least some sources and summaries
    assert!(!store.sources.is_empty() || !store.sinks.is_empty() || !store.summaries.is_empty(),
        "android.app.model.yml should have sources, sinks, or summaries");
}

#[test]
fn parse_android_content_model() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vendor/codeql/java/ql/lib/ext/android.content.model.yml"
    );
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let store = parse_model_file(&content).unwrap();
    assert!(store.total_models() > 0);

    // This file has Intent.putExtra summaries with complex access paths
    let intent_summaries: Vec<_> = store.summaries.iter()
        .filter(|s| s.callable.type_name == "Intent")
        .collect();
    assert!(!intent_summaries.is_empty(), "should have Intent summaries");
}

#[test]
fn parse_java_io_model() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vendor/codeql/java/ql/lib/ext/java.io.model.yml"
    );
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let store = parse_model_file(&content).unwrap();
    assert!(store.total_models() > 0);
    // java.io has lots of neutrals
    assert!(!store.neutrals.is_empty(), "java.io should have neutral models");
}

#[test]
fn parse_cpp_model() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vendor/codeql/cpp/ql/lib/ext/bsl.vector.model.yml"
    );
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let store = parse_model_file(&content).unwrap();
    assert!(store.total_models() > 0);
}

#[test]
fn load_all_java_models() {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vendor/codeql/java/ql/lib/ext"
    );
    let dir_path = Path::new(dir);
    if !dir_path.exists() {
        return;
    }

    let store = load_models_from_dir(dir_path).unwrap();
    // Java has hundreds of model files
    eprintln!(
        "Java models: {} sources, {} sinks, {} summaries, {} neutrals ({} total)",
        store.sources.len(),
        store.sinks.len(),
        store.summaries.len(),
        store.neutrals.len(),
        store.total_models()
    );
    assert!(store.total_models() > 100, "should load many Java models");
}

#[test]
fn load_all_cpp_models() {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vendor/codeql/cpp/ql/lib/ext"
    );
    let dir_path = Path::new(dir);
    if !dir_path.exists() {
        return;
    }

    let store = load_models_from_dir(dir_path).unwrap();
    eprintln!(
        "C++ models: {} sources, {} sinks, {} summaries, {} neutrals ({} total)",
        store.sources.len(),
        store.sinks.len(),
        store.summaries.len(),
        store.neutrals.len(),
        store.total_models()
    );
    assert!(store.total_models() > 0, "should load some C++ models");
}
