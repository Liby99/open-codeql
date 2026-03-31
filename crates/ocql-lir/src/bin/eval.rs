//! LIR lowering evaluation tool.
//!
//! Walks a directory of .ql/.qll files, parses each one, lowers to MIR then LIR,
//! and reports success/failure statistics.
//!
//! Usage: ocql-lir-eval <directory> [--verbose] [--errors] [--limit N]

use std::path::{Path, PathBuf};
use std::time::Instant;
use std::collections::HashMap;

fn main() {
    let builder = std::thread::Builder::new().stack_size(128 * 1024 * 1024);
    let handler = builder.spawn(real_main).unwrap();
    handler.join().unwrap();
}

fn real_main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: ocql-lir-eval <directory> [--verbose] [--errors] [--limit N]");
        std::process::exit(1);
    }

    let dir = PathBuf::from(&args[1]);
    let show_errors = args.iter().any(|a| a == "--errors");
    let limit: Option<usize> = args.windows(2)
        .find(|w| w[0] == "--limit")
        .and_then(|w| w[1].parse().ok());

    let mut files = Vec::new();
    collect_ql_files(&dir, &mut files);
    files.sort();

    let total = match limit {
        Some(n) => files.len().min(n),
        None => files.len(),
    };

    eprintln!("Found {} files, processing {}", files.len(), total);
    let start = Instant::now();

    let mut parse_ok = 0usize;
    let mut parse_fail = 0usize;
    let mut mir_ok = 0usize;
    let mut mir_fail = 0usize;
    let mut lir_ok = 0usize;
    let mut lir_fail = 0usize;
    let mut lir_errors: HashMap<String, usize> = HashMap::new();
    let mut total_rules = 0usize;
    let mut total_strata = 0usize;

    for (_i, path) in files.iter().take(total).enumerate() {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let ast = match ocql_ql_parser::parse_source_file(&source) {
            Ok(ast) => { parse_ok += 1; ast }
            Err(_) => { parse_fail += 1; continue; }
        };

        let mir = match ocql_mir::lower_source_file(&ast) {
            Ok(mir) => { mir_ok += 1; mir }
            Err(_) => { mir_fail += 1; continue; }
        };

        match ocql_lir::lower_mir(&mir) {
            Ok(lir) => {
                lir_ok += 1;
                total_rules += lir.rule_count();
                total_strata += lir.strata.len();
            }
            Err(e) => {
                lir_fail += 1;
                let key = format!("{}", e);
                *lir_errors.entry(key.clone()).or_insert(0) += 1;
                if show_errors {
                    let rel_path = path.strip_prefix(&dir).unwrap_or(path);
                    eprintln!("  LIR FAIL: {} — {}", rel_path.display(), e);
                }
            }
        }
    }

    let elapsed = start.elapsed();

    eprintln!("\n=== LIR Lowering Results ===");
    eprintln!("Total files:      {}", total);
    eprintln!("Parse OK:         {} ({:.1}%)", parse_ok, parse_ok as f64 / total as f64 * 100.0);
    eprintln!("MIR OK:           {} ({:.1}%)", mir_ok, mir_ok as f64 / parse_ok.max(1) as f64 * 100.0);
    eprintln!("LIR OK:           {} ({:.1}% of MIR)", lir_ok,
        if mir_ok > 0 { lir_ok as f64 / mir_ok as f64 * 100.0 } else { 0.0 });
    eprintln!("LIR FAIL:         {}", lir_fail);
    eprintln!("Total rules:      {}", total_rules);
    eprintln!("Total strata:     {}", total_strata);
    eprintln!("Time:             {:.2}s", elapsed.as_secs_f64());

    if !lir_errors.is_empty() {
        eprintln!("\n=== LIR Error Breakdown ===");
        let mut errors: Vec<_> = lir_errors.into_iter().collect();
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
