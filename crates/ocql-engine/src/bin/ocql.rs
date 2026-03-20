//! ocql — Extract C/C++/Java source files and run Datalog queries.
//!
//! Usage:
//!   ocql <source-files...> --query <query.dl>
//!   ocql <source-files...> --query <query.dl> --lang c|cpp|java
//!   ocql <source-files...> --query <query.dl> --show <table1,table2,...>
//!   ocql <source-files...> --query <query.dl> --show-all
//!   ocql <source-files...> --dump                         # dump all EDB tables
//!
//! Examples:
//!   ocql examples/callgraph.c --query examples/callgraph.dl
//!   ocql examples/callgraph.c --query examples/callgraph.dl --show direct_call,transitive_call
//!   ocql src/*.c --query my_analysis.dl --show-all
//!   ocql Main.java --query find_bugs.dl --lang java

use std::path::Path;

use ocql_database::{Database, Value};
use ocql_extractor_common::Extractor;
use ocql_extractor_cpp::{CppExtractor, cpp_schema};
use ocql_extractor_java::{JavaExtractor, java_schema};
use ocql_engine::{evaluate, parse_program};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        print_usage();
        std::process::exit(if args.is_empty() { 1 } else { 0 });
    }

    // Parse arguments
    let mut source_files: Vec<String> = Vec::new();
    let mut query_file: Option<String> = None;
    let mut lang: Option<String> = None;
    let mut show_tables: Vec<String> = Vec::new();
    let mut show_all = false;
    let mut dump_edb = false;
    let mut verbose = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--query" | "-q" => {
                i += 1;
                query_file = Some(args.get(i).expect("--query requires a file path").clone());
            }
            "--lang" | "-l" => {
                i += 1;
                lang = Some(args.get(i).expect("--lang requires a value").clone());
            }
            "--show" | "-s" => {
                i += 1;
                let tables = args.get(i).expect("--show requires table names");
                show_tables.extend(tables.split(',').map(|s| s.trim().to_string()));
            }
            "--show-all" => {
                show_all = true;
            }
            "--dump" => {
                dump_edb = true;
            }
            "--verbose" | "-v" => {
                verbose = true;
            }
            other => {
                source_files.push(other.to_string());
            }
        }
        i += 1;
    }

    if source_files.is_empty() {
        eprintln!("Error: no source files specified");
        std::process::exit(1);
    }

    if query_file.is_none() && !dump_edb {
        eprintln!("Error: no --query file or --dump specified");
        std::process::exit(1);
    }

    // Detect language from file extensions if not specified
    let lang = lang.unwrap_or_else(|| detect_language(&source_files));

    // Create database and extractor
    let (mut db, extractor): (Database, Box<dyn Extractor>) = match lang.as_str() {
        "c" => (Database::from_schema(cpp_schema()), Box::new(CppExtractor::c())),
        "cpp" | "c++" | "cxx" => (Database::from_schema(cpp_schema()), Box::new(CppExtractor::cpp())),
        "java" => (Database::from_schema(java_schema()), Box::new(JavaExtractor::new())),
        other => {
            eprintln!("Error: unsupported language '{}' (use c, cpp, or java)", other);
            std::process::exit(1);
        }
    };

    // Extract source files
    let mut total_files = 0;
    let mut success_files = 0;
    for path_str in &source_files {
        let path = Path::new(path_str);
        if path.is_dir() {
            let results = extractor.extract_directory(&mut db, path);
            for r in &results {
                total_files += 1;
                if r.success {
                    success_files += 1;
                } else {
                    eprintln!("  FAIL: {} — {:?}", r.file_path, r.error);
                }
            }
        } else {
            let source = std::fs::read(path)
                .unwrap_or_else(|e| { eprintln!("Error reading {}: {}", path_str, e); std::process::exit(1); });
            let result = extractor.extract_source(&mut db, path_str, &source);
            total_files += 1;
            if result.success {
                success_files += 1;
            } else {
                eprintln!("  FAIL: {} — {:?}", result.file_path, result.error);
            }
        }
    }
    eprintln!("Extracted {}/{} files ({})", success_files, total_files, lang);

    // Dump EDB tables if requested
    if dump_edb {
        dump_database(&db, verbose);
        return;
    }

    // Read and parse the query
    let query_path = query_file.unwrap();
    let query_text = std::fs::read_to_string(&query_path)
        .unwrap_or_else(|e| { eprintln!("Error reading query {}: {}", query_path, e); std::process::exit(1); });

    let mut program = parse_program(&query_text)
        .unwrap_or_else(|e| { eprintln!("Parse error in {}: {}", query_path, e); std::process::exit(1); });

    eprintln!("Parsed {} rules from {}", program.rules.len(), query_path);

    // Resolve string literals
    program.resolve_strings(&mut db);

    // Record which tables exist before evaluation (EDB)
    let edb_tables: std::collections::HashSet<String> = db.relation_names()
        .map(|s| s.to_string())
        .collect();

    // Evaluate
    evaluate(&program, &mut db).unwrap_or_else(|e| {
        eprintln!("Evaluation error: {}", e);
        std::process::exit(1);
    });

    // Determine which tables to display
    let idb_tables: Vec<String> = db.relation_names()
        .filter(|n| !edb_tables.contains(*n))
        .map(|s| s.to_string())
        .collect();

    let tables_to_show = if !show_tables.is_empty() {
        show_tables
    } else if show_all {
        let mut all: Vec<String> = idb_tables.clone();
        all.sort();
        all
    } else {
        // Default: show all IDB (derived) tables
        let mut sorted = idb_tables.clone();
        sorted.sort();
        sorted
    };

    // Print results
    for table in &tables_to_show {
        print_table(&db, table);
    }

    // Summary
    if verbose || tables_to_show.is_empty() {
        eprintln!("\nDerived tables:");
        let mut sorted = idb_tables;
        sorted.sort();
        for name in &sorted {
            let count = db.relation(name).map_or(0, |r| r.len());
            eprintln!("  {:30} {:>6} rows", name, count);
        }
    }
}

fn print_usage() {
    eprintln!("ocql — Extract source files and run Datalog queries");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  ocql <source-files...> --query <query.dl>");
    eprintln!("  ocql <source-files...> --query <query.dl> --show table1,table2");
    eprintln!("  ocql <source-files...> --query <query.dl> --show-all");
    eprintln!("  ocql <source-files...> --dump");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -q, --query <file>    Datalog query file (.dl)");
    eprintln!("  -l, --lang <lang>     Language: c, cpp, java (auto-detected if omitted)");
    eprintln!("  -s, --show <tables>   Comma-separated list of tables to display");
    eprintln!("      --show-all        Show all derived tables");
    eprintln!("      --dump            Dump all extracted (EDB) tables");
    eprintln!("  -v, --verbose         Show table summary");
    eprintln!("  -h, --help            Show this help");
}

fn detect_language(files: &[String]) -> String {
    for f in files {
        let path = Path::new(f);
        if path.is_dir() {
            // Look at first file in directory
            continue;
        }
        match path.extension().and_then(|e| e.to_str()) {
            Some("java") => return "java".to_string(),
            Some("cpp" | "cxx" | "cc" | "hpp" | "hxx") => return "cpp".to_string(),
            Some("c" | "h") => return "c".to_string(),
            _ => {}
        }
    }
    "c".to_string() // default
}

fn dump_database(db: &Database, verbose: bool) {
    let mut names: Vec<String> = db.relation_names().map(|s| s.to_string()).collect();
    names.sort();

    eprintln!("\nDatabase tables:");
    for name in &names {
        let count = db.relation(name).map_or(0, |r| r.len());
        if count > 0 || verbose {
            eprintln!("  {:30} {:>6} rows", name, count);
        }
    }

    for name in &names {
        let rel = match db.relation(name) {
            Some(r) if r.len() > 0 => r,
            _ => continue,
        };
        println!("\n=== {} ({} rows) ===", name, rel.len());
        // Print column headers
        let headers: Vec<&str> = rel.schema.columns.iter().map(|c| c.name.as_str()).collect();
        println!("{}", headers.join("\t"));
        // Print rows
        for tuple in rel.scan() {
            let fields: Vec<String> = tuple.iter().map(|v| format_value(v, db)).collect();
            println!("{}", fields.join("\t"));
        }
    }
}

fn print_table(db: &Database, table: &str) {
    let rel = match db.relation(table) {
        Some(r) => r,
        None => {
            eprintln!("Warning: table '{}' not found", table);
            return;
        }
    };

    if rel.is_empty() {
        println!("\n=== {} (empty) ===", table);
        return;
    }

    println!("\n=== {} ({} rows) ===", table, rel.len());
    for tuple in rel.scan() {
        let fields: Vec<String> = tuple.iter().map(|v| format_value(v, db)).collect();
        println!("{}", fields.join("\t"));
    }
}

fn format_value(v: &Value, db: &Database) -> String {
    match v {
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::String(s) => db.strings.resolve(*s).to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Entity(e) => format!("#{}", e.0),
        Value::Null => "NULL".to_string(),
    }
}
