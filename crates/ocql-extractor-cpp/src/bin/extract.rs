//! Extract C/C++ source files into a database and dump as CSV.
//!
//! Usage:
//!   extract-cpp <source-dir> <output-dir> [--cpp]
//!
//! Extracts all .c/.h files (or .cpp/.hpp with --cpp) from <source-dir>,
//! populates a database, and writes one CSV file per table into <output-dir>.

use std::path::Path;

use ocql_database::{Database, Value};
use ocql_extractor_common::Extractor;
use ocql_extractor_cpp::{CppExtractor, cpp_schema};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: extract-cpp <source-dir> <output-dir> [--cpp]");
        std::process::exit(1);
    }

    let source_dir = Path::new(&args[1]);
    let output_dir = Path::new(&args[2]);
    let use_cpp = args.iter().any(|a| a == "--cpp");

    if !source_dir.is_dir() {
        eprintln!("Error: {:?} is not a directory", source_dir);
        std::process::exit(1);
    }

    std::fs::create_dir_all(output_dir).expect("failed to create output directory");

    let schema = cpp_schema();
    let mut db = Database::from_schema(schema);

    let extractor = if use_cpp {
        CppExtractor::cpp()
    } else {
        CppExtractor::c()
    };

    eprintln!("Extracting from {:?} ...", source_dir);
    let results = extractor.extract_directory(&mut db, source_dir);

    let mut success = 0;
    let mut failed = 0;
    for r in &results {
        if r.success {
            success += 1;
        } else {
            failed += 1;
            eprintln!("  FAIL: {} — {:?}", r.file_path, r.error);
        }
    }
    eprintln!("Extracted {} files ({} success, {} failed)", results.len(), success, failed);

    // Dump each table as CSV
    let table_names: Vec<String> = db.relation_names().map(|s| s.to_string()).collect();
    let mut table_names_sorted = table_names;
    table_names_sorted.sort();

    eprintln!("\nTable summary:");
    for name in &table_names_sorted {
        let count = db.relation(name).map_or(0, |r| r.len());
        eprintln!("  {:30} {:>6} rows", name, count);
    }

    for name in &table_names_sorted {
        let rel = match db.relation(name) {
            Some(r) => r,
            None => continue,
        };
        if rel.len() == 0 {
            continue;
        }

        let csv_path = output_dir.join(format!("{}.csv", name));
        let mut out = String::new();

        // Header row from schema
        let col_names: Vec<&str> = rel.schema.columns.iter()
            .map(|c| c.name.as_str())
            .collect();
        out.push_str(&col_names.join(","));
        out.push('\n');

        // Data rows
        for tuple in rel.scan() {
            let fields: Vec<String> = tuple.iter().map(|v| {
                format_value(v, &db)
            }).collect();
            out.push_str(&fields.join(","));
            out.push('\n');
        }

        std::fs::write(&csv_path, &out).expect("failed to write CSV");
    }

    eprintln!("\nCSV files written to {:?}", output_dir);
}

fn format_value(v: &Value, db: &Database) -> String {
    match v {
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::String(s) => {
            let resolved = db.strings.resolve(*s);
            // CSV-escape: wrap in quotes if contains comma, quote, or newline
            if resolved.contains(',') || resolved.contains('"') || resolved.contains('\n') {
                format!("\"{}\"", resolved.replace('"', "\"\""))
            } else {
                resolved.to_string()
            }
        }
        Value::Bool(b) => b.to_string(),
        Value::Entity(e) => format!("#{}", e.0),
        Value::Null => String::new(),
    }
}
