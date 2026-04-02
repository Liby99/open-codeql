//! End-to-end test: compile our own Java oqlpack, extract Java code,
//! and evaluate queries using our translated QL library.
//!
//! This validates that the oqlpack/java/ library we translated from
//! vendor/codeql/java/ actually works with our pipeline.
//!
//! Run with: cargo test -p ocql-e2e-tests --test oqlpack_java -- --nocapture

use std::path::Path;

use ocql_database::{Database, Value};
use ocql_extractor_common::Extractor;
use ocql_extractor_java::{JavaExtractor, java_schema};

/// Test: compile oqlpack java library + simple query, extract Simple.java, evaluate.
#[test]
fn oqlpack_java_basic_query() {
    let builder = std::thread::Builder::new().stack_size(64 * 1024 * 1024);
    let handle = builder.spawn(oqlpack_java_basic_query_inner).unwrap();
    handle.join().unwrap();
}

fn oqlpack_java_basic_query_inner() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()  // crates/
        .parent().unwrap(); // workspace root
    let oqlpack_lib = workspace.join("oqlpack/java/lib");

    if !oqlpack_lib.exists() {
        eprintln!("SKIP: oqlpack/java/lib not found");
        return;
    }

    let t_total = std::time::Instant::now();

    // ================================================================
    // Phase 1: HIR analysis of our oqlpack
    // ================================================================
    eprintln!("\n=== Phase 1: HIR analysis of oqlpack ===");
    let t0 = std::time::Instant::now();
    let hir = ocql_hir::analyze_project(&oqlpack_lib);
    eprintln!("  Library files: {}", hir.files.len());
    eprintln!("  Clean: {}", hir.clean_file_count());
    eprintln!("  Time: {:.2}s", t0.elapsed().as_secs_f64());

    // Show any errors
    let errors: Vec<_> = hir.files.values()
        .flat_map(|f| f.diagnostics.iter())
        .collect();
    if !errors.is_empty() {
        eprintln!("  HIR errors ({}):", errors.len());
        for (i, e) in errors.iter().enumerate().take(20) {
            eprintln!("    [{}] {:?}", i, e);
        }
    }

    // ================================================================
    // Phase 2: Parse a query that uses `import java`
    // ================================================================
    eprintln!("\n=== Phase 2: Parse query ===");
    let query_source = r#"
import java

from Method m
where m.isPublic()
select m, m.getName(), m.getDeclaringType().getName()
"#;
    let query_ast = ocql_ql_parser::parse_source_file(query_source)
        .expect("query should parse");
    eprintln!("  Query parsed: {} members", query_ast.members.len());

    // ================================================================
    // Phase 3: MIR lowering
    // ================================================================
    eprintln!("\n=== Phase 3: MIR lowering ===");
    let t0 = std::time::Instant::now();
    let mut all_asts: Vec<&ocql_ql_ast::module::SourceFile> = hir.files.values()
        .map(|f| &f.ast)
        .collect();
    all_asts.push(&query_ast);
    let mir = ocql_mir::lower_source_files(&all_asts).expect("MIR lowering failed");
    eprintln!("  Predicates: {}", mir.predicates.len());
    eprintln!("  Time: {:.2}s", t0.elapsed().as_secs_f64());

    let select_count = mir.predicates.iter()
        .filter(|p| p.name.starts_with("select_result"))
        .count();
    eprintln!("  select_result predicates: {}", select_count);
    assert!(select_count > 0, "query should produce select_result predicates");

    // ================================================================
    // Phase 4: Engine emission
    // ================================================================
    eprintln!("\n=== Phase 4: Engine emission ===");
    let program = ocql_mir::emit_program_with_strings(&mir);
    eprintln!("  Engine rules: {}", program.rules.len());

    // ================================================================
    // Phase 5: Extract Simple.java
    // ================================================================
    eprintln!("\n=== Phase 5: Extract Simple.java ===");
    let java_path = workspace.join("crates/ocql-e2e-tests/tests/fixtures/Simple.java");
    let source = std::fs::read(&java_path).unwrap();
    let schema = java_schema();
    let mut db = Database::from_schema(schema);
    let extractor = JavaExtractor::new();
    let result = extractor.extract_source(&mut db, java_path.to_str().unwrap(), &source);
    assert!(result.success, "Java extraction failed: {:?}", result.error);

    let method_count = db.scan("methods").map(|i| i.count()).unwrap_or(0);
    let class_count = db.scan("classes_or_interfaces").map(|i| i.count()).unwrap_or(0);
    eprintln!("  Classes: {}", class_count);
    eprintln!("  Methods: {}", method_count);

    // ================================================================
    // Phase 6: Evaluate
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
            // Phase 7: Check results
            // ================================================================
            eprintln!("\n=== Phase 7: Query results ===");

            // List select_result relations
            let select_names: Vec<String> = db.relation_names()
                .filter(|n| n.starts_with("select_result"))
                .map(|n| n.to_string())
                .collect();
            eprintln!("  select_result relations: {:?}", select_names);

            for select_name in &select_names {
                if let Some(iter) = db.scan(select_name) {
                    let rows: Vec<_> = iter.collect();
                    if !rows.is_empty() {
                        eprintln!("  {} has {} rows:", select_name, rows.len());
                        for row in &rows {
                            let vals: Vec<String> = row.iter().map(|v| match v {
                                Value::String(s) => db.strings.resolve(*s).to_string(),
                                Value::Int(i) => i.to_string(),
                                Value::Entity(e) => format!("#{}", e.0),
                                _ => "?".to_string(),
                            }).collect();
                            eprintln!("    {:?}", vals);
                        }
                    }
                }
            }

            // Check #char relations
            for name in ["Method#char", "Class#char", "Element#char", "Type#char",
                         "RefType#char", "Callable#char", "Member#char",
                         "Modifiable#char", "Modifier#char", "ClassOrInterface#char",
                         "SrcRefType#char", "TopLevelType#char",
                         "@method#char", "@constructor#char", "@classorinterface#char",
                         "@modifier#char", "@field#char", "@param#char"] {
                let count = db.scan(name).map(|i| i.count()).unwrap_or(0);
                if count > 0 {
                    eprintln!("  {}: {} rows", name, count);
                }
            }

            // Debug: dump all non-empty relations
            eprintln!("\n  All non-empty relations:");
            let mut all_names: Vec<String> = db.relation_names().map(|s| s.to_string()).collect();
            all_names.sort();
            for name in &all_names {
                let count = db.scan(name).map(|i| i.count()).unwrap_or(0);
                if count > 0 {
                    eprintln!("    {}: {} rows", name, count);
                }
            }

            // Debug: dump methods and hasModifier tables
            eprintln!("\n  methods table:");
            if let Some(iter) = db.scan("methods") {
                for row in iter {
                    let vals: Vec<String> = row.iter().map(|v| match v {
                        Value::String(s) => format!("\"{}\"", db.strings.resolve(*s)),
                        Value::Int(i) => format!("{}", i),
                        Value::Entity(e) => format!("#{}", e.0),
                        _ => "?".to_string(),
                    }).collect();
                    eprintln!("    {:?}", vals);
                }
            }

            eprintln!("\n  modifiers table:");
            if let Some(iter) = db.scan("modifiers") {
                for row in iter {
                    let vals: Vec<String> = row.iter().map(|v| match v {
                        Value::String(s) => format!("\"{}\"", db.strings.resolve(*s)),
                        Value::Int(i) => format!("{}", i),
                        Value::Entity(e) => format!("#{}", e.0),
                        _ => "?".to_string(),
                    }).collect();
                    eprintln!("    {:?}", vals);
                }
            }

            eprintln!("\n  hasModifier table:");
            if let Some(iter) = db.scan("hasModifier") {
                for row in iter {
                    let vals: Vec<String> = row.iter().map(|v| match v {
                        Value::String(s) => format!("\"{}\"", db.strings.resolve(*s)),
                        Value::Int(i) => format!("{}", i),
                        Value::Entity(e) => format!("#{}", e.0),
                        _ => "?".to_string(),
                    }).collect();
                    eprintln!("    {:?}", vals);
                }
            }

            eprintln!("\n  Total time: {:.2}s", t_total.elapsed().as_secs_f64());
        }
        Err(e) => {
            panic!("Evaluation FAILED: {}", e);
        }
    }
}
