//! Trace every pipeline stage for the negation test case.
//! Run with: cargo test -p ocql-e2e-tests --test trace_pipeline -- --nocapture

use ocql_database::{Database, Value};
use ocql_engine::evaluate;
use ocql_extractor_common::Extractor;
use ocql_extractor_cpp::{CppExtractor, cpp_schema};
use ocql_mir::{compile_ql, compile_ql_to_engine, print_mir};
use ocql_lir::{lower_mir, pretty_print};
use ocql_ql_parser::parse_source_file;

#[test]
fn trace_negation_pipeline() {
    let c_path = format!("{}/tests/fixtures/basic.c", env!("CARGO_MANIFEST_DIR"));
    let c_source = std::fs::read_to_string(&c_path).unwrap();

    let ql_source = r#"
        predicate has_param_id(int fid) {
            params(fid, _, _, _)
        }
        predicate no_params(string name) {
            functions(fid, name, _) and not has_param_id(fid)
        }
    "#;

    eprintln!("\n═══ STAGE 0: Source files ═══");
    eprintln!("\n--- basic.c ---\n{}", c_source);
    eprintln!("--- QL query ---{}\n", ql_source);

    // STAGE 1: Extract
    eprintln!("═══ STAGE 1: C Extractor → Database tables ═══\n");
    let schema = cpp_schema();
    let mut db = Database::from_schema(schema);
    let extractor = CppExtractor::c();
    let source_bytes = std::fs::read(&c_path).unwrap();
    let result = extractor.extract_source(&mut db, &c_path, &source_bytes);
    assert!(result.success);

    eprintln!("  functions:");
    if let Some(iter) = db.scan("functions") {
        for row in iter {
            let name = match &row[1] { Value::String(s) => db.strings.resolve(*s).to_string(), o => format!("{:?}", o) };
            eprintln!("    id={:?}  name={:?}  kind={:?}", row[0], name, row[2]);
        }
    }
    eprintln!("  params:");
    if let Some(iter) = db.scan("params") {
        for row in iter {
            let name = match &row[2] { Value::String(s) => db.strings.resolve(*s).to_string(), o => format!("{:?}", o) };
            let ty = match &row[3] { Value::String(s) => db.strings.resolve(*s).to_string(), o => format!("{:?}", o) };
            eprintln!("    func_id={:?}  idx={:?}  name={:?}  type={:?}", row[0], row[1], name, ty);
        }
    }

    // STAGE 2: Parse
    eprintln!("\n═══ STAGE 2: QL Parser → AST ═══\n");
    let ast = parse_source_file(ql_source).expect("parse failed");
    eprintln!("  {} top-level members", ast.members.len());
    for (i, m) in ast.members.iter().enumerate() {
        eprintln!("    [{}] {:?}", i, m);
    }

    // STAGE 3: MIR
    eprintln!("\n═══ STAGE 3: AST → MIR ═══\n");
    let mir = compile_ql(ql_source).expect("MIR failed");
    eprintln!("{}", print_mir(&mir));

    // STAGE 4: LIR
    eprintln!("═══ STAGE 4: MIR → LIR ═══\n");
    let lir = lower_mir(&mir).expect("LIR failed");
    eprintln!("{}", pretty_print(&lir));

    // STAGE 5: Engine rules
    eprintln!("═══ STAGE 5: MIR → Engine rules ═══\n");
    let mut program = compile_ql_to_engine(ql_source).expect("compile failed");
    for (i, rule) in program.rules.iter().enumerate() {
        eprintln!("  [{}] {:?}", i, rule);
    }

    // STAGE 6: Evaluate
    eprintln!("\n═══ STAGE 6: Evaluate ═══\n");
    program.resolve_strings(&mut db);
    evaluate(&program, &mut db).expect("eval failed");

    eprintln!("  has_param_id results:");
    if let Some(iter) = db.scan("has_param_id") {
        for row in iter { eprintln!("    fid={:?}", row[0]); }
    }
    eprintln!("  no_params results:");
    if let Some(iter) = db.scan("no_params") {
        for row in iter {
            let name = match &row[0] { Value::String(s) => db.strings.resolve(*s).to_string(), o => format!("{:?}", o) };
            eprintln!("    name={:?}", name);
        }
    }
    eprintln!("\n═══ DONE ═══\n");
}
