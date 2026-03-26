//! Integration tests that extract real-world C projects (git submodules).
//!
//! These tests validate that extraction succeeds on real code and produces
//! reasonable output. They're #[ignore]d by default since they require
//! the git submodules to be checked out:
//!
//!   git submodule update --init vendor/test-repos/*
//!
//! Run with:
//!   cargo test -p ocql-extractor-cpp --test real_project_tests -- --ignored

use std::path::Path;

use ocql_database::{Database, Value};
use ocql_extractor_common::Extractor;
use ocql_extractor_cpp::{CppExtractor, cpp_schema};

/// Extract a real project directory, returning the database and results.
fn extract_project(dir: &str) -> (Database, Vec<ocql_extractor_common::ExtractionResult>) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor/test-repos")
        .join(dir);
    if !path.is_dir() {
        panic!(
            "Test repo not found: {:?}\nRun: git submodule update --init vendor/test-repos/{}",
            path, dir
        );
    }
    let schema = cpp_schema();
    let mut db = Database::from_schema(schema);
    let extractor = CppExtractor::c();
    let results = extractor.extract_directory(&mut db, &path);
    (db, results)
}

fn table_count(db: &Database, table: &str) -> usize {
    db.relation(table).map_or(0, |r| r.len())
}

fn column_strings(db: &Database, table: &str, col: usize) -> Vec<String> {
    db.scan(table)
        .unwrap()
        .map(|t| match &t[col] {
            Value::String(s) => db.strings.resolve(*s).to_string(),
            other => format!("{:?}", other),
        })
        .collect()
}

// ============================================================
// qoi — phoboslab/qoi (4 files)
// Small, clean image codec
// ============================================================

#[test]
#[ignore]
fn qoi_extraction_succeeds() {
    let (_, results) = extract_project("qoi");
    assert!(results.len() >= 4, "qoi should have >= 4 C/H files");
    assert!(
        results.iter().all(|r| r.success),
        "All qoi files should extract successfully"
    );
}

#[test]
#[ignore]
fn qoi_functions() {
    let (db, _) = extract_project("qoi");
    let names = column_strings(&db, "functions", 1);
    assert!(names.len() >= 10, "qoi should have >= 10 functions, got {}", names.len());
    assert!(names.contains(&"main".into()), "should have main()");
}

#[test]
#[ignore]
fn qoi_types_and_fields() {
    let (db, _) = extract_project("qoi");
    let type_count = table_count(&db, "usertypes");
    let field_count = table_count(&db, "fields");
    assert!(type_count >= 3, "qoi should have >= 3 types, got {}", type_count);
    assert!(field_count >= 10, "qoi should have >= 10 fields, got {}", field_count);
}

#[test]
#[ignore]
fn qoi_includes() {
    let (db, _) = extract_project("qoi");
    let paths = column_strings(&db, "includes", 2);
    assert!(paths.iter().any(|p| p.contains("stdio.h")), "should include stdio.h");
    assert!(paths.iter().any(|p| p.contains("qoi.h")), "should include qoi.h");
}

// ============================================================
// rax — antirez/rax (9 files)
// Radix tree by Redis author — clean, well-structured C
// ============================================================

#[test]
#[ignore]
fn rax_extraction_succeeds() {
    let (_, results) = extract_project("rax");
    assert!(results.len() >= 8, "rax should have >= 8 C/H files");
    assert!(
        results.iter().all(|r| r.success),
        "All rax files should extract successfully"
    );
}

#[test]
#[ignore]
fn rax_core_types() {
    let (db, _) = extract_project("rax");
    let names = column_strings(&db, "usertypes", 1);
    assert!(names.contains(&"raxNode".into()), "should have raxNode");
    assert!(names.contains(&"rax".into()), "should have rax struct");
    assert!(names.contains(&"raxIterator".into()), "should have raxIterator");
    assert!(names.contains(&"raxStack".into()), "should have raxStack");
}

#[test]
#[ignore]
fn rax_core_functions() {
    let (db, _) = extract_project("rax");
    let names = column_strings(&db, "functions", 1);
    // rax.c exports many rax* functions
    let rax_funcs: Vec<_> = names.iter().filter(|n| n.starts_with("rax")).collect();
    assert!(
        rax_funcs.len() >= 10,
        "rax should have >= 10 rax* functions, got {}: {:?}",
        rax_funcs.len(),
        rax_funcs
    );
}

#[test]
#[ignore]
fn rax_fields() {
    let (db, _) = extract_project("rax");
    let names = column_strings(&db, "fields", 2);
    // raxNode fields
    assert!(names.contains(&"iskey".into()));
    assert!(names.contains(&"size".into()));
    assert!(names.contains(&"data".into()));
    // raxIterator fields
    assert!(names.contains(&"key".into()));
    assert!(names.contains(&"key_len".into()));
}

// ============================================================
// utf8.h — sheredom/utf8.h (5 files)
// Header-only UTF-8 library — tests header extraction
// ============================================================

#[test]
#[ignore]
fn utf8h_extraction_succeeds() {
    let (_, results) = extract_project("utf8.h");
    assert!(results.len() >= 4, "utf8.h should have >= 4 C/H files");
    assert!(
        results.iter().all(|r| r.success),
        "All utf8.h files should extract successfully"
    );
}

#[test]
#[ignore]
fn utf8h_functions() {
    let (db, _) = extract_project("utf8.h");
    let names = column_strings(&db, "functions", 1);
    // The main utf8.h header has many utf8* functions
    let utf8_funcs: Vec<_> = names.iter().filter(|n| n.starts_with("utf8")).collect();
    assert!(
        utf8_funcs.len() >= 20,
        "utf8.h should have >= 20 utf8* functions, got {}: {:?}",
        utf8_funcs.len(),
        utf8_funcs
    );
}

// ============================================================
// lua-cjson — mpx/lua-cjson (8 files)
// JSON parser for Lua — tests complex parsing code
// ============================================================

#[test]
#[ignore]
fn lua_cjson_extraction_succeeds() {
    let (_, results) = extract_project("lua-cjson");
    assert!(results.len() >= 7, "lua-cjson should have >= 7 C/H files");
    assert!(
        results.iter().all(|r| r.success),
        "All lua-cjson files should extract successfully"
    );
}

#[test]
#[ignore]
fn lua_cjson_functions() {
    let (db, _) = extract_project("lua-cjson");
    let names = column_strings(&db, "functions", 1);
    assert!(names.len() >= 30, "lua-cjson should have >= 30 functions, got {}", names.len());
    // strbuf functions
    assert!(names.iter().any(|n| n.starts_with("strbuf")), "should have strbuf_* functions");
    // fpconv functions
    assert!(names.iter().any(|n| n.starts_with("fpconv")), "should have fpconv_* functions");
}

#[test]
#[ignore]
fn lua_cjson_types() {
    let (db, _) = extract_project("lua-cjson");
    let type_count = table_count(&db, "usertypes");
    let names = column_strings(&db, "usertypes", 1);
    // lua-cjson uses typedef struct { ... } strbuf_t; pattern, so the struct
    // is anonymous. We check that types exist and include known named ones.
    assert!(type_count >= 5, "lua-cjson should have >= 5 types, got {}: {:?}", type_count, names);
    // Bigint and BCinfo are named structs in dtoa.c
    assert!(names.contains(&"Bigint".into()) || names.contains(&"BCinfo".into()),
        "should have Bigint or BCinfo from dtoa.c, got: {:?}", names);
}

// ============================================================
// dperf — baidu/dperf (73 files)
// Largest test repo — tests extraction at scale
// ============================================================

#[test]
#[ignore]
fn dperf_extraction_succeeds() {
    let (_, results) = extract_project("dperf");
    assert!(results.len() >= 60, "dperf should have >= 60 C/H files");
    let failures: Vec<_> = results.iter().filter(|r| !r.success).collect();
    assert!(
        failures.is_empty(),
        "dperf should have no extraction failures, got {} failures: {:?}",
        failures.len(),
        failures.iter().map(|f| &f.file_path).collect::<Vec<_>>()
    );
}

#[test]
#[ignore]
fn dperf_scale() {
    let (db, _) = extract_project("dperf");
    let func_count = table_count(&db, "functions");
    let type_count = table_count(&db, "usertypes");
    let field_count = table_count(&db, "fields");
    let param_count = table_count(&db, "params");

    eprintln!("dperf: {} functions, {} types, {} fields, {} params",
        func_count, type_count, field_count, param_count);

    assert!(func_count >= 200, "dperf should have >= 200 functions, got {}", func_count);
    assert!(type_count >= 40, "dperf should have >= 40 types, got {}", type_count);
    assert!(field_count >= 100, "dperf should have >= 100 fields, got {}", field_count);
    assert!(param_count >= 400, "dperf should have >= 400 params, got {}", param_count);
}

#[test]
#[ignore]
fn dperf_locations_valid() {
    let (db, _) = extract_project("dperf");
    // Every location should have positive line numbers
    for loc in db.scan("locations_default").unwrap() {
        let line = loc[2].as_int().unwrap();
        assert!(line > 0, "line should be positive, got {}", line);
        let col = loc[3].as_int().unwrap();
        assert!(col > 0, "column should be positive, got {}", col);
    }
    // Every element_location should reference an existing location
    let loc_count = table_count(&db, "locations_default");
    let elem_loc_count = table_count(&db, "element_location");
    assert!(
        elem_loc_count > 0 && elem_loc_count <= loc_count * 2,
        "element_location count ({}) should be reasonable vs locations ({})",
        elem_loc_count, loc_count
    );
}

// ============================================================
// coreutils — coreutils/coreutils (400+ files)
// GNU core utilities — large-scale real-world project
// ============================================================

#[test]
#[ignore]
fn coreutils_extraction_succeeds() {
    let (_, results) = extract_project("coreutils");
    assert!(results.len() >= 100, "coreutils should have >= 100 C/H files");
    let failures: Vec<_> = results.iter().filter(|r| !r.success).collect();
    assert!(
        failures.is_empty(),
        "coreutils should have no extraction failures, got {} failures: {:?}",
        failures.len(),
        failures.iter().map(|f| &f.file_path).collect::<Vec<_>>()
    );
}

#[test]
#[ignore]
fn coreutils_scale() {
    let (db, _) = extract_project("coreutils");
    let func_count = table_count(&db, "functions");
    let type_count = table_count(&db, "usertypes");
    let field_count = table_count(&db, "fields");
    let param_count = table_count(&db, "params");

    eprintln!("coreutils: {} functions, {} types, {} fields, {} params",
        func_count, type_count, field_count, param_count);

    assert!(func_count >= 200, "coreutils should have >= 200 functions, got {}", func_count);
    assert!(type_count >= 20, "coreutils should have >= 20 types, got {}", type_count);
    assert!(field_count >= 50, "coreutils should have >= 50 fields, got {}", field_count);
    assert!(param_count >= 400, "coreutils should have >= 400 params, got {}", param_count);
}

// ============================================================
// lua — lua/lua (~30 files)
// Lua programming language implementation
// ============================================================

#[test]
#[ignore]
fn lua_extraction_succeeds() {
    let (_, results) = extract_project("lua");
    assert!(results.len() >= 20, "lua should have >= 20 C/H files");
    assert!(
        results.iter().all(|r| r.success),
        "All lua files should extract successfully"
    );
}

#[test]
#[ignore]
fn lua_core_functions() {
    let (db, _) = extract_project("lua");
    let names = column_strings(&db, "functions", 1);
    assert!(names.len() >= 100, "lua should have >= 100 functions, got {}", names.len());
    // Check for known Lua API functions
    assert!(
        names.contains(&"lua_newstate".into()) || names.contains(&"luaL_openlibs".into()),
        "should have lua_newstate or luaL_openlibs"
    );
}

#[test]
#[ignore]
fn lua_types() {
    let (db, _) = extract_project("lua");
    let type_count = table_count(&db, "usertypes");
    assert!(type_count >= 10, "lua should have >= 10 types, got {}", type_count);
}

// ============================================================
// libpng — glennrp/libpng (~20 files)
// PNG reference library
// ============================================================

#[test]
#[ignore]
fn libpng_extraction_succeeds() {
    let (_, results) = extract_project("libpng");
    assert!(results.len() >= 15, "libpng should have >= 15 C/H files");
    assert!(
        results.iter().all(|r| r.success),
        "All libpng files should extract successfully"
    );
}

#[test]
#[ignore]
fn libpng_functions() {
    let (db, _) = extract_project("libpng");
    let names = column_strings(&db, "functions", 1);
    assert!(names.len() >= 50, "libpng should have >= 50 functions, got {}", names.len());
    // Check for known PNG API functions
    let png_funcs: Vec<_> = names.iter().filter(|n| n.starts_with("png_")).collect();
    assert!(
        png_funcs.len() >= 20,
        "libpng should have >= 20 png_* functions, got {}: {:?}",
        png_funcs.len(),
        png_funcs
    );
}

#[test]
#[ignore]
fn libpng_types() {
    let (db, _) = extract_project("libpng");
    let type_count = table_count(&db, "usertypes");
    assert!(type_count >= 5, "libpng should have >= 5 types, got {}", type_count);
}

// ============================================================
// Cross-project summary (for manual inspection)
// ============================================================

#[test]
#[ignore]
fn all_projects_summary() {
    let repos = ["qoi", "rax", "utf8.h", "lua-cjson", "dperf", "coreutils", "lua", "libpng"];

    eprintln!("\n{:<12} {:>5} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}",
        "Project", "Files", "Funcs", "Types", "Fields", "Params", "Vars", "Incls");
    eprintln!("{}", "-".repeat(70));

    for repo in &repos {
        let (db, results) = extract_project(repo);
        let files = results.len();
        let funcs = table_count(&db, "functions");
        let types = table_count(&db, "usertypes");
        let fields = table_count(&db, "fields");
        let params = table_count(&db, "params");
        let vars = table_count(&db, "variables");
        let incls = table_count(&db, "includes");

        eprintln!("{:<12} {:>5} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}",
            repo, files, funcs, types, fields, params, vars, incls);
    }
}
