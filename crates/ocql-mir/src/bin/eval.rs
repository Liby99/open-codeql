//! MIR lowering evaluation tool.
//!
//! Walks a directory of .ql/.qll files, parses each one, attempts MIR lowering,
//! and reports success/failure statistics.
//!
//! Usage: ocql-mir-eval <directory> [--verbose] [--errors] [--limit N]

use std::path::{Path, PathBuf};
use std::time::Instant;
use std::collections::HashMap;

fn main() {
    // Larger stack for deeply nested QL ASTs
    let builder = std::thread::Builder::new().stack_size(128 * 1024 * 1024);
    let handler = builder.spawn(real_main).unwrap();
    handler.join().unwrap();
}

fn real_main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: ocql-mir-eval <directory> [--verbose] [--errors] [--limit N]");
        eprintln!("  Parses .ql/.qll files and attempts MIR lowering, reporting statistics.");
        std::process::exit(1);
    }

    let dir = PathBuf::from(&args[1]);
    let verbose = args.iter().any(|a| a == "--verbose");
    let show_errors = args.iter().any(|a| a == "--errors");
    let limit: Option<usize> = args.windows(2)
        .find(|w| w[0] == "--limit")
        .and_then(|w| w[1].parse().ok());

    if !dir.exists() {
        eprintln!("Error: directory does not exist: {}", dir.display());
        std::process::exit(1);
    }

    eprintln!("Scanning for .ql/.qll files in: {}", dir.display());
    let start = Instant::now();

    let mut files = Vec::new();
    collect_ql_files(&dir, &mut files);
    files.sort();

    let total = match limit {
        Some(n) => files.len().min(n),
        None => files.len(),
    };

    eprintln!("Found {} files, processing {}", files.len(), total);

    let mut parse_ok = 0usize;
    let mut parse_fail = 0usize;
    let mut lower_ok = 0usize;
    let mut lower_fail = 0usize;
    let mut lower_errors: HashMap<String, usize> = HashMap::new();
    let mut total_predicates = 0usize;

    for (i, path) in files.iter().take(total).enumerate() {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Parse (preprocessing is internal to the parser)
        let ast = match ocql_ql_parser::parse_source_file(&source) {
            Ok(ast) => {
                parse_ok += 1;
                ast
            }
            Err(_) => {
                parse_fail += 1;
                if verbose {
                    eprintln!("  PARSE FAIL: {}", path.display());
                }
                continue;
            }
        };

        // MIR lowering
        match ocql_mir::lower_source_file(&ast) {
            Ok(mir) => {
                lower_ok += 1;
                total_predicates += mir.predicates.len();
                if verbose && (i + 1) % 500 == 0 {
                    eprintln!("  [{}/{}] {} — {} predicates",
                        i + 1, total, path.display(), mir.predicates.len());
                }
            }
            Err(e) => {
                lower_fail += 1;
                let key = format!("{}", e);
                *lower_errors.entry(key.clone()).or_insert(0) += 1;
                if show_errors {
                    let rel_path = path.strip_prefix(&dir).unwrap_or(path);
                    eprintln!("  MIR FAIL: {} — {}", rel_path.display(), e);
                }
            }
        }
    }

    let elapsed = start.elapsed();

    eprintln!("\n=== MIR Lowering Results ===");
    eprintln!("Total files:      {}", total);
    eprintln!("Parse OK:         {} ({:.1}%)", parse_ok, parse_ok as f64 / total as f64 * 100.0);
    eprintln!("Parse FAIL:       {}", parse_fail);
    eprintln!("MIR Lower OK:     {} ({:.1}% of parsed)", lower_ok,
        if parse_ok > 0 { lower_ok as f64 / parse_ok as f64 * 100.0 } else { 0.0 });
    eprintln!("MIR Lower FAIL:   {}", lower_fail);
    eprintln!("Total predicates: {}", total_predicates);
    eprintln!("Time:             {:.2}s", elapsed.as_secs_f64());

    if !lower_errors.is_empty() {
        eprintln!("\n=== MIR Lowering Error Breakdown ===");
        let mut errors: Vec<_> = lower_errors.into_iter().collect();
        errors.sort_by(|a, b| b.1.cmp(&a.1));
        for (err, count) in &errors {
            eprintln!("  {:>5}x  {}", count, err);
        }
    }
}

fn collect_ql_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip upgrade directories
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name == "upgrade" || name == ".git" || name == "node_modules" {
                    continue;
                }
            }
            collect_ql_files(&path, files);
        } else if let Some(ext) = path.extension() {
            if ext == "ql" || ext == "qll" {
                files.push(path);
            }
        }
    }
}
