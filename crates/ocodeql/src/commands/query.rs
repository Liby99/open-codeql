use std::path::PathBuf;
use std::time::Instant;

use ocql_database::Database;
use ocql_engine::evaluate;
use ocql_mir::{compile_ql, compile_ql_to_engine, print_mir};

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        print_usage();
        return Err("missing subcommand".into());
    }

    match args[0].as_str() {
        "run" => run_query(&args[1..]),
        "compile" => compile(&args[1..]),
        "help" | "--help" => {
            print_usage();
            Ok(())
        }
        other => Err(format!("unknown query subcommand: {}", other).into()),
    }
}

fn print_usage() {
    eprintln!("Usage: ocodeql query <subcommand> [options]");
    eprintln!();
    eprintln!("Subcommands:");
    eprintln!("  run       Run a QL query against a database");
    eprintln!("  compile   Compile a QL query to MIR (debug output)");
    eprintln!();
    eprintln!("Run options:");
    eprintln!("  ocodeql query run --database <db-path> --query <ql-file-or-string>");
    eprintln!();
    eprintln!("  --database, -d   Path to a database file");
    eprintln!("  --query, -q      Path to a .ql file or inline QL string");
    eprintln!("  --output, -o     Output format: table (default), csv, json");
    eprintln!();
    eprintln!("Compile options:");
    eprintln!("  ocodeql query compile <ql-file-or-string>");
    eprintln!();
    eprintln!("  --mir            Show MIR S-expression (default)");
    eprintln!("  --engine         Show engine rules");
}

fn run_query(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut db_path = None;
    let mut query_input = None;
    let mut output_format = "table".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--database" | "-d" => {
                i += 1;
                db_path = Some(PathBuf::from(args.get(i).ok_or("missing value for --database")?));
            }
            "--query" | "-q" => {
                i += 1;
                query_input = Some(args.get(i).ok_or("missing value for --query")?.clone());
            }
            "--output" | "-o" => {
                i += 1;
                output_format = args.get(i).ok_or("missing value for --output")?.clone();
            }
            other => {
                // Treat positional arg as query if no --query given
                if query_input.is_none() {
                    query_input = Some(other.to_string());
                } else {
                    return Err(format!("unknown option: {}", other).into());
                }
            }
        }
        i += 1;
    }

    let db_path = db_path.ok_or("--database is required")?;
    let query_input = query_input.ok_or("--query is required")?;

    // Load database
    eprintln!("Loading database from: {}", db_path.display());
    let mut db = ocql_database::load_from_file(&db_path)?;

    // Read query source
    let ql_source = read_query_source(&query_input)?;

    eprintln!("Compiling query...");
    let start = Instant::now();
    let mut program = compile_ql_to_engine(&ql_source)
        .map_err(|e| format!("query compilation failed: {}", e))?;

    program.resolve_strings(&mut db);

    eprintln!("Evaluating...");
    evaluate(&program, &mut db)
        .map_err(|e| format!("query evaluation failed: {}", e))?;

    let elapsed = start.elapsed();
    eprintln!("Done in {:.3}s", elapsed.as_secs_f64());

    // Find select_result relation(s)
    let select_rels: Vec<String> = db.relation_names()
        .filter(|n| n.starts_with("select_result"))
        .map(|s| s.to_string())
        .collect();

    if select_rels.is_empty() {
        // Show all new predicates from the query
        let head_preds: Vec<String> = program.head_predicates().into_iter().map(|s| s.to_string()).collect();
        for pred in &head_preds {
            print_relation(&db, pred, &output_format);
        }
    } else {
        for rel in &select_rels {
            print_relation(&db, rel, &output_format);
        }
    }

    Ok(())
}

fn compile(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut show_engine = false;
    let mut query_input = None;

    for arg in args {
        match arg.as_str() {
            "--engine" => show_engine = true,
            "--mir" => show_engine = false,
            _ => {
                if query_input.is_none() {
                    query_input = Some(arg.clone());
                }
            }
        }
    }

    let query_input = query_input.ok_or("usage: ocodeql query compile <ql-file-or-string>")?;
    let ql_source = read_query_source(&query_input)?;

    if show_engine {
        let program = compile_ql_to_engine(&ql_source)
            .map_err(|e| format!("compilation failed: {}", e))?;
        println!("{:?}", program);
    } else {
        let mir = compile_ql(&ql_source)
            .map_err(|e| format!("compilation failed: {}", e))?;
        println!("{}", print_mir(&mir));
    }

    Ok(())
}

fn read_query_source(input: &str) -> Result<String, Box<dyn std::error::Error>> {
    let path = PathBuf::from(input);
    if path.exists() && (input.ends_with(".ql") || input.ends_with(".qll")) {
        Ok(std::fs::read_to_string(&path)?)
    } else {
        // Treat as inline QL
        Ok(input.to_string())
    }
}

fn print_relation(db: &Database, name: &str, format: &str) {
    let rows: Vec<ocql_database::Tuple> = match db.scan(name) {
        Some(iter) => iter.cloned().collect(),
        None => return,
    };

    if rows.is_empty() {
        return;
    }

    match format {
        "csv" => {
            for row in &rows {
                let vals: Vec<String> = row.iter().map(|v| format_value(db, v)).collect();
                println!("{}", vals.join(","));
            }
        }
        "json" => {
            println!("[");
            for (i, row) in rows.iter().enumerate() {
                let vals: Vec<String> = row.iter().map(|v| {
                    let s = format_value(db, v);
                    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
                }).collect();
                let comma = if i + 1 < rows.len() { "," } else { "" };
                println!("  [{}]{}", vals.join(", "), comma);
            }
            println!("]");
        }
        _ => {
            // table format
            println!("| {} | ({} rows)", name, rows.len());
            println!("{}", "-".repeat(60));
            for row in &rows {
                let vals: Vec<String> = row.iter().map(|v| format_value(db, v)).collect();
                println!("| {}", vals.join(" | "));
            }
            println!();
        }
    }
}

fn format_value(db: &Database, value: &ocql_database::Value) -> String {
    match value {
        ocql_database::Value::Int(n) => n.to_string(),
        ocql_database::Value::Float(f) => f.to_string(),
        ocql_database::Value::String(s) => db.strings.resolve(*s).to_string(),
        ocql_database::Value::Bool(b) => b.to_string(),
        ocql_database::Value::Entity(id) => format!("@{}", id.0),
        ocql_database::Value::Null => "null".to_string(),
    }
}
