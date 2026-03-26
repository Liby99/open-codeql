use std::process;

mod commands;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    let result = match args[1].as_str() {
        "database" => commands::database::run(&args[2..]),
        "query" => commands::query::run(&args[2..]),
        "parse" => commands::parse::run(&args[2..]),
        "analyze" => commands::analyze::run(&args[2..]),
        "version" | "--version" | "-v" => {
            println!("ocodeql {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        other => {
            eprintln!("Unknown command: {}", other);
            eprintln!();
            print_usage();
            process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn print_usage() {
    eprintln!("ocodeql — Open CodeQL");
    eprintln!();
    eprintln!("Usage: ocodeql <command> [options]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  database create   Create a CodeQL database from source code");
    eprintln!("  database load     Load and inspect an existing database");
    eprintln!("  query run         Run a QL query against a database");
    eprintln!("  query compile     Compile a QL query to MIR (debug)");
    eprintln!("  parse             Parse a .ql/.qll file and print the AST");
    eprintln!("  analyze           Run HIR analysis on a QL project");
    eprintln!("  version           Show version information");
    eprintln!("  help              Show this help message");
}
