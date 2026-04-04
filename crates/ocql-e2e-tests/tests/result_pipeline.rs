//! End-to-end test for the query result pipeline:
//!   extract C → evaluate QL → extract results → write .obqrs → read .obqrs → decode CSV/JSON/SARIF
//!
//! Run with: cargo test -p ocql-e2e-tests --test result_pipeline -- --nocapture

use ocql_database::Database;
use ocql_engine::evaluate;
use ocql_extractor_common::Extractor;
use ocql_extractor_cpp::{CppExtractor, cpp_schema};
use ocql_mir::compile_ql_to_engine;
use ocql_result::{
    parse_query_metadata, extract_query_result, extract_from_relation,
    write_obqrs, read_obqrs, to_csv, to_json, to_sarif,
    write_obqrs_file, read_obqrs_file,
    QueryKind,
};

fn fixture_path(filename: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), filename)
}

fn extract_c(filename: &str) -> Database {
    let path = fixture_path(filename);
    let source = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path, e));
    let schema = cpp_schema();
    let mut db = Database::from_schema(schema);
    let extractor = CppExtractor::c();
    let result = extractor.extract_source(&mut db, &path, &source);
    assert!(result.success, "C extraction failed: {:?}", result.error);
    db
}

fn eval_ql(ql: &str, db: &mut Database) {
    let mut program = compile_ql_to_engine(ql).expect("compile_ql_to_engine failed");
    program.resolve_strings(db);
    evaluate(&program, db).expect("evaluate failed");
}

// ============================================================
// Test: Full pipeline with dangerous_functions.ql
// ============================================================

#[test]
fn result_pipeline_dangerous_functions() {
    // Read the query source and parse metadata
    let query_path = fixture_path("dangerous_functions.ql");
    let query_source = std::fs::read_to_string(&query_path).unwrap();
    let metadata = parse_query_metadata(&query_source);

    eprintln!("=== Query metadata ===");
    eprintln!("  name: {:?}", metadata.name);
    eprintln!("  kind: {:?}", metadata.kind);
    eprintln!("  id:   {:?}", metadata.id);
    eprintln!("  tags: {:?}", metadata.tags);
    assert_eq!(metadata.kind, Some(QueryKind::Problem));
    assert_eq!(metadata.id.as_deref(), Some("ocql/dangerous-function-call"));

    // Extract and evaluate
    let mut db = extract_c("security.c");
    eval_ql(r#"
        predicate isDangerousName(string name) {
            name = "gets" or name = "strcpy" or name = "sprintf" or name = "strcat"
        }
        predicate dangerousCall(int call_id, string callee_name) {
            exprs(call_id, 74, _) and
            exprparents(callee_id, 0, call_id) and
            valuetext(callee_id, callee_name) and
            isDangerousName(callee_name)
        }
        predicate dangerousFinding(string callee_name, string in_function) {
            dangerousCall(call_id, callee_name) and
            enclosingfunction(call_id, func_id) and
            functions(func_id, in_function, _)
        }
        from string callee, string caller
        where dangerousFinding(callee, caller)
        select callee, caller
    "#, &mut db);

    // Extract results from named predicate (dangerousFinding)
    let result = extract_from_relation(&db, "dangerousFinding", metadata);
    eprintln!("\n=== Query result ===");
    eprintln!("  columns: {}", result.num_columns());
    eprintln!("  rows:    {}", result.num_rows());
    for row in &result.rows {
        eprintln!("  {:?}", row);
    }
    assert!(result.num_rows() >= 2, "should find at least 2 dangerous calls");

    // === CSV ===
    let csv = to_csv(&result);
    eprintln!("\n=== CSV output ===");
    eprintln!("{}", csv);
    assert!(csv.contains("gets"));
    assert!(csv.contains("strcpy"));

    // === JSON ===
    let json = to_json(&result);
    eprintln!("=== JSON output ===");
    eprintln!("{}", json);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["metadata"]["kind"], "problem");
    assert!(parsed["rows"].as_array().unwrap().len() >= 2);

    // === SARIF ===
    let sarif = to_sarif(&result);
    eprintln!("=== SARIF output ===");
    eprintln!("{}", sarif);
    let sarif_parsed: serde_json::Value = serde_json::from_str(&sarif).unwrap();
    assert_eq!(sarif_parsed["version"], "2.1.0");
    let sarif_results = sarif_parsed["runs"][0]["results"].as_array().unwrap();
    assert!(sarif_results.len() >= 2);
    assert_eq!(sarif_results[0]["ruleId"], "ocql/dangerous-function-call");
    assert_eq!(sarif_results[0]["level"], "error");

    // === Binary .obqrs round-trip ===
    let mut buf = Vec::new();
    write_obqrs(&mut buf, &result).unwrap();
    eprintln!("\n=== Binary .obqrs ===");
    eprintln!("  Size: {} bytes", buf.len());

    let mut cursor = std::io::Cursor::new(&buf);
    let decoded = read_obqrs(&mut cursor).unwrap();
    assert_eq!(decoded.num_rows(), result.num_rows());
    assert_eq!(decoded.metadata.id, result.metadata.id);
    assert_eq!(decoded.rows, result.rows);

    // === File round-trip ===
    let tmp_path = "/tmp/ocql_test_result.obqrs";
    write_obqrs_file(tmp_path, &result).unwrap();
    let from_file = read_obqrs_file(tmp_path).unwrap();
    assert_eq!(from_file.rows, result.rows);
    assert_eq!(from_file.metadata.name, result.metadata.name);
    std::fs::remove_file(tmp_path).ok();

    eprintln!("\n=== All result pipeline checks passed ===");
}

// ============================================================
// Test: select_result extraction
// ============================================================

#[test]
fn result_pipeline_select_result_extraction() {
    let mut db = extract_c("security.c");
    eval_ql(r#"
        predicate isDangerousName(string name) {
            name = "gets" or name = "strcpy" or name = "sprintf" or name = "strcat"
        }
        predicate dangerousCall(int call_id, string callee_name) {
            exprs(call_id, 74, _) and
            exprparents(callee_id, 0, call_id) and
            valuetext(callee_id, callee_name) and
            isDangerousName(callee_name)
        }
        predicate dangerousFinding(string callee_name, string in_function) {
            dangerousCall(call_id, callee_name) and
            enclosingfunction(call_id, func_id) and
            functions(func_id, in_function, _)
        }
        from string callee, string caller
        where dangerousFinding(callee, caller)
        select callee, caller
    "#, &mut db);

    // Test extract_query_result which finds select_result_* automatically
    let metadata = ocql_result::QueryMetadata {
        name: Some("Test".to_string()),
        kind: Some(QueryKind::Problem),
        ..Default::default()
    };
    let result = extract_query_result(&db, metadata);
    eprintln!("select_result extraction: {} rows", result.num_rows());

    // The select_result should contain from-clause vars + select expression values
    // For `from string callee, string caller ... select callee, caller`:
    //   params are: callee, caller, _sel0, _sel1
    // But the output should contain rows with those values
    assert!(result.num_rows() >= 2, "should have at least 2 rows from select_result");

    // Verify we can find dangerous function names in the output
    let mut found_gets = false;
    let mut found_strcpy = false;
    for row in &result.rows {
        for val in row {
            match val {
                ocql_result::ResultValue::String(s) if s == "gets" => found_gets = true,
                ocql_result::ResultValue::String(s) if s == "strcpy" => found_strcpy = true,
                _ => {}
            }
        }
    }
    assert!(found_gets, "should find 'gets' in select results");
    assert!(found_strcpy, "should find 'strcpy' in select results");
}
