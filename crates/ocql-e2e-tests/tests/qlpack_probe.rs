//! Probe: attempt to compile the full vendor/codeql C++ library through the pipeline.
//! Run with: cargo test -p ocql-e2e-tests --test qlpack_probe -- --nocapture

use std::collections::HashMap;
use std::path::Path;

use ocql_database::Database;
use ocql_extractor_common::Extractor;
use ocql_extractor_cpp::{CppExtractor, cpp_schema};

#[test]
fn probe_vendor_cpp_qlpack() {
    // Need large stack for deep ASTs in vendor files
    let builder = std::thread::Builder::new().stack_size(128 * 1024 * 1024);
    let handle = builder.spawn(probe_inner).unwrap();
    handle.join().unwrap();
}

fn probe_inner() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()  // crates/
        .parent().unwrap(); // workspace root
    let cpp_lib = workspace.join("vendor/codeql/cpp/ql/lib");

    if !cpp_lib.exists() {
        eprintln!("SKIP: vendor/codeql/cpp/ql/lib not found");
        return;
    }

    // Phase 1: HIR
    eprintln!("\n=== Phase 1: HIR analysis ===");
    let t0 = std::time::Instant::now();
    let hir = ocql_hir::analyze_project(&cpp_lib);
    eprintln!("  Files: {}", hir.files.len());
    eprintln!("  Clean: {}", hir.clean_file_count());
    eprintln!("  Errors: {}", hir.error_count());
    eprintln!("  Time: {:.2}s", t0.elapsed().as_secs_f64());

    // Phase 2: Per-file MIR
    eprintln!("\n=== Phase 2: Per-file MIR lowering ===");
    let mut mir_ok = 0u32;
    let mut mir_fail = 0u32;
    let mut error_kinds: HashMap<String, u32> = HashMap::new();
    let mut total_preds = 0usize;

    for (_fid, analysis) in &hir.files {
        match ocql_mir::lower_source_file(&analysis.ast) {
            Ok(mir) => {
                mir_ok += 1;
                total_preds += mir.predicates.len();
            }
            Err(e) => {
                mir_fail += 1;
                *error_kinds.entry(format!("{}", e)).or_default() += 1;
            }
        }
    }
    eprintln!("  OK: {}/{} ({:.1}%)", mir_ok, mir_ok + mir_fail,
        100.0 * mir_ok as f64 / (mir_ok + mir_fail) as f64);
    eprintln!("  Predicates: {}", total_preds);
    for (err, count) in &error_kinds {
        eprintln!("    [{}] {}", count, err);
    }

    // Phase 3: Merged MIR
    eprintln!("\n=== Phase 3: Merged MIR lowering ===");
    let asts: Vec<&ocql_ql_ast::module::SourceFile> = hir.files.values()
        .map(|f| &f.ast)
        .collect();
    let mir = ocql_mir::lower_source_files(&asts).expect("merged MIR failed");
    eprintln!("  Merged predicates: {}", mir.predicates.len());

    // Phase 4: Engine emission
    eprintln!("\n=== Phase 4: Engine emission ===");
    let program = ocql_mir::emit_program_with_strings(&mir);
    eprintln!("  Engine rules: {}", program.rules.len());
    eprintln!("  Head predicates: {}", program.head_predicates().len());

    // Phase 5: Evaluate against basic.c
    eprintln!("\n=== Phase 5: Evaluate against basic.c ===");
    let c_path = workspace.join("crates/ocql-e2e-tests/tests/fixtures/basic.c");
    let source = std::fs::read(&c_path).unwrap();
    let schema = cpp_schema();
    let mut db = Database::from_schema(schema);
    let extractor = CppExtractor::c();
    let result = extractor.extract_source(&mut db, c_path.to_str().unwrap(), &source);
    assert!(result.success);

    let mut program2 = program;
    program2.resolve_strings(&mut db);

    match ocql_engine::evaluate(&program2, &mut db) {
        Ok(_) => {
            eprintln!("  Evaluation: OK");
            // Check class characteristic predicates
            for name in ["Function#char", "Element#char", "Declaration#char"] {
                let count = db.scan(name).map(|i| i.count()).unwrap_or(0);
                eprintln!("  {}: {} rows", name, count);
            }
            // Count non-empty #char relations
            let nonempty_count = db.relation_names()
                .filter(|n| n.contains("#char"))
                .filter(|n| db.scan(n).map(|i| i.count()).unwrap_or(0) > 0)
                .count();
            let total_char = db.relation_names()
                .filter(|n| n.contains("#char"))
                .count();
            eprintln!("  #char relations: {} total, {} non-empty", total_char, nonempty_count);

            // Check key predicates
            for name in ["File#char", "Folder#char", "Container#char",
                         "Location#char", "Function#char", "Declaration#char",
                         "Element#char", "Locatable#char",
                         "ControlFlowNode#char", "AccessHolder#char",
                         "Variable#char", "Stmt#char", "Expr#char",
                         "Namespace#char", "Comment#char"] {
                let count = db.scan(name).map(|i| i.count()).unwrap_or(0);
                eprintln!("  {}: {} rows", name, count);
            }
        }
        Err(e) => {
            eprintln!("  Evaluation FAILED: {}", e);
            // This is expected — let's see what error we get
        }
    }
}
