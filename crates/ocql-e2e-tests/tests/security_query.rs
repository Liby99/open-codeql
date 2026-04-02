//! End-to-end test: compile the FULL vendor C++ qlpack library alongside a
//! security query, extract a vulnerable C file, and evaluate the query.
//!
//! This exercises the complete pipeline:
//!   vendor qlpack (594 files) + security query → HIR → MIR → engine rules
//!   security.c → C extractor → Database
//!   evaluate(rules, database) → security findings
//!
//! Run with: cargo test -p ocql-e2e-tests --test security_query -- --nocapture

use std::collections::HashSet;
use std::path::Path;

use ocql_database::{Database, Value};
use ocql_extractor_common::Extractor;
use ocql_extractor_cpp::{CppExtractor, cpp_schema};

#[test]
fn vendor_security_query_dangerous_functions() {
    // Need large stack for deep ASTs in vendor files
    let builder = std::thread::Builder::new().stack_size(128 * 1024 * 1024);
    let handle = builder.spawn(security_query_inner).unwrap();
    handle.join().unwrap();
}

fn security_query_inner() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()  // crates/
        .parent().unwrap(); // workspace root
    let cpp_lib = workspace.join("vendor/codeql/cpp/ql/lib");

    if !cpp_lib.exists() {
        eprintln!("SKIP: vendor/codeql/cpp/ql/lib not found");
        return;
    }

    let t_total = std::time::Instant::now();

    // ================================================================
    // Phase 1: Compile vendor C++ qlpack library (594 files)
    // ================================================================
    eprintln!("\n=== Phase 1: HIR analysis of vendor qlpack ===");
    let t0 = std::time::Instant::now();
    let hir = ocql_hir::analyze_project(&cpp_lib);
    eprintln!("  Library files: {}", hir.files.len());
    eprintln!("  Clean: {}", hir.clean_file_count());
    eprintln!("  Time: {:.2}s", t0.elapsed().as_secs_f64());

    // ================================================================
    // Phase 2: Parse the security query
    // ================================================================
    eprintln!("\n=== Phase 2: Parse security query ===");
    let query_path = workspace.join("crates/ocql-e2e-tests/tests/fixtures/dangerous_functions.ql");
    let query_source = std::fs::read_to_string(&query_path).unwrap();
    let query_ast = ocql_ql_parser::parse_source_file(&query_source)
        .expect("security query should parse");
    eprintln!("  Query parsed: {} members", query_ast.members.len());

    // ================================================================
    // Phase 3: Merged MIR lowering (vendor library + query)
    // ================================================================
    eprintln!("\n=== Phase 3: Merged MIR lowering ===");
    let t0 = std::time::Instant::now();
    let mut all_asts: Vec<&ocql_ql_ast::module::SourceFile> = hir.files.values()
        .map(|f| &f.ast)
        .collect();
    all_asts.push(&query_ast);
    let mir = ocql_mir::lower_source_files(&all_asts).expect("merged MIR failed");
    eprintln!("  Merged predicates: {}", mir.predicates.len());
    eprintln!("  Time: {:.2}s", t0.elapsed().as_secs_f64());

    // Check our query predicates are in the MIR
    let has_is_dangerous = mir.predicates.iter().any(|p| p.name == "isDangerousName");
    let select_count = mir.predicates.iter().filter(|p| p.name.starts_with("select_result")).count();
    eprintln!("  isDangerousName predicate: {}", has_is_dangerous);
    eprintln!("  select_result predicates: {}", select_count);
    assert!(has_is_dangerous, "query predicate isDangerousName should be in MIR");
    assert!(select_count > 0, "query should produce at least one select_result predicate");

    // ================================================================
    // Phase 4: Engine emission
    // ================================================================
    eprintln!("\n=== Phase 4: Engine emission ===");
    let program = ocql_mir::emit_program_with_strings(&mir);
    eprintln!("  Engine rules: {}", program.rules.len());

    // ================================================================
    // Phase 5: Extract security.c
    // ================================================================
    eprintln!("\n=== Phase 5: Extract security.c ===");
    let c_path = workspace.join("crates/ocql-e2e-tests/tests/fixtures/security.c");
    let source = std::fs::read(&c_path).unwrap();
    let schema = cpp_schema();
    let mut db = Database::from_schema(schema);
    let extractor = CppExtractor::c();
    let result = extractor.extract_source(&mut db, c_path.to_str().unwrap(), &source);
    assert!(result.success, "extraction failed");

    // Show what we extracted
    let func_count = db.scan("functions").map(|i| i.count()).unwrap_or(0);
    let expr_count = db.scan("exprs").map(|i| i.count()).unwrap_or(0);
    let stmt_count = db.scan("stmts").map(|i| i.count()).unwrap_or(0);
    eprintln!("  Functions: {}", func_count);
    eprintln!("  Expressions: {}", expr_count);
    eprintln!("  Statements: {}", stmt_count);

    // Show extracted function names
    let func_names: Vec<String> = db.scan("functions").unwrap()
        .map(|t| match &t[1] {
            Value::String(s) => db.strings.resolve(*s).to_string(),
            _ => "?".to_string(),
        })
        .collect();
    eprintln!("  Function names: {:?}", func_names);

    // ================================================================
    // Phase 6: Evaluate — vendor qlpack + security query against security.c
    // ================================================================
    eprintln!("\n=== Phase 6: Evaluate ===");
    let t0 = std::time::Instant::now();
    let mut program2 = program;
    program2.resolve_strings(&mut db);

    match ocql_engine::evaluate(&program2, &mut db) {
        Ok(_) => {
            let eval_time = t0.elapsed().as_secs_f64();
            eprintln!("  Evaluation: OK ({:.2}s)", eval_time);

            // ================================================================
            // Phase 7: Check security query results
            // ================================================================
            eprintln!("\n=== Phase 7: Security query results ===");

            // Check intermediate predicates
            let dangerous_name_count = db.scan("isDangerousName").map(|i| i.count()).unwrap_or(0);
            let dangerous_call_count = db.scan("dangerousCall").map(|i| i.count()).unwrap_or(0);
            let finding_count = db.scan("dangerousFinding").map(|i| i.count()).unwrap_or(0);
            eprintln!("  isDangerousName rows: {}", dangerous_name_count);
            eprintln!("  dangerousCall rows: {}", dangerous_call_count);
            eprintln!("  dangerousFinding rows: {}", finding_count);

            // Show dangerousFinding details
            if let Some(iter) = db.scan("dangerousFinding") {
                for row in iter {
                    let callee = match &row[0] {
                        Value::String(s) => db.strings.resolve(*s).to_string(),
                        _ => "?".to_string(),
                    };
                    let caller = match &row[1] {
                        Value::String(s) => db.strings.resolve(*s).to_string(),
                        _ => "?".to_string(),
                    };
                    eprintln!("    FINDING: {}() called in {}()", callee, caller);
                }
            }

            // Find all select result relations
            let select_names: Vec<String> = db.relation_names()
                .filter(|n| n.starts_with("select_result"))
                .map(|n| n.to_string())
                .collect();

            // Collect findings from both direct predicates and select results
            let mut found_dangerous: HashSet<String> = HashSet::new();

            // From dangerousFinding predicate
            if let Some(iter) = db.scan("dangerousFinding") {
                for row in iter {
                    if let Value::String(s) = &row[0] {
                        found_dangerous.insert(db.strings.resolve(*s).to_string());
                    }
                }
            }

            // Also check select results
            let mut select_with_results = 0;
            for select_name in &select_names {
                let rows: Vec<_> = db.scan(select_name).unwrap().collect();
                if !rows.is_empty() {
                    select_with_results += 1;
                    eprintln!("  {} has {} rows", select_name, rows.len());
                    for row in &rows {
                        for v in row.iter() {
                            if let Value::String(s) = v {
                                let name = db.strings.resolve(*s).to_string();
                                if ["gets", "strcpy", "sprintf", "strcat"].contains(&name.as_str()) {
                                    found_dangerous.insert(name);
                                }
                            }
                        }
                    }
                }
            }
            eprintln!("  Select results with data: {}/{}", select_with_results, select_names.len());

            eprintln!("\n  === SECURITY FINDINGS ===");
            for name in &found_dangerous {
                let cwe = match name.as_str() {
                    "gets" => "CWE-120 (buffer overflow via gets)",
                    "strcpy" => "CWE-120 (buffer overflow via strcpy)",
                    "sprintf" => "CWE-120 (buffer overflow via sprintf)",
                    "strcat" => "CWE-120 (buffer overflow via strcat)",
                    _ => "unknown",
                };
                eprintln!("  [VULN] Call to {}() — {}", name, cwe);
            }

            // Verify results
            assert!(!found_dangerous.is_empty(),
                "Should find at least one dangerous function call. Found: {:?}", found_dangerous);
            assert!(found_dangerous.contains("gets"),
                "Should find gets() call — CWE-120 buffer overflow. Found: {:?}", found_dangerous);
            assert!(found_dangerous.contains("strcpy"),
                "Should find strcpy() call — CWE-120 no bounds check. Found: {:?}", found_dangerous);

            // Show vendor class char stats
            eprintln!("\n  === Vendor class statistics ===");
            for name in ["Function#char", "Element#char", "Stmt#char", "Expr#char",
                         "Declaration#char", "Comment#char", "ControlFlowNode#char"] {
                let count = db.scan(name).map(|i| i.count()).unwrap_or(0);
                eprintln!("  {}: {} rows", name, count);
            }

            let nonempty_char = db.relation_names()
                .filter(|n| n.contains("#char"))
                .filter(|n| db.scan(n).map(|i| i.count()).unwrap_or(0) > 0)
                .count();
            eprintln!("  Non-empty #char relations: {}", nonempty_char);

            eprintln!("\n  Total time: {:.2}s", t_total.elapsed().as_secs_f64());
        }
        Err(e) => {
            panic!("Evaluation FAILED: {}", e);
        }
    }
}
