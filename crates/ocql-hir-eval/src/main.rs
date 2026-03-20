use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: ocql-hir-eval <workspace-root> [--verbose] [--errors] [--limit N]");
        eprintln!("  Analyzes all .ql/.qll files and reports analysis rate.");
        std::process::exit(1);
    }

    let workspace_root = PathBuf::from(&args[1]);
    let verbose = args.iter().any(|a| a == "--verbose");
    let show_errors = args.iter().any(|a| a == "--errors");
    let limit: Option<usize> = args
        .windows(2)
        .find(|w| w[0] == "--limit")
        .and_then(|w| w[1].parse().ok());

    if !workspace_root.exists() {
        eprintln!("Error: path does not exist: {}", workspace_root.display());
        std::process::exit(1);
    }

    eprintln!("Analyzing project: {}", workspace_root.display());
    let start = Instant::now();

    let db = ocql_hir::analyze_project(&workspace_root);

    let elapsed = start.elapsed();
    let total_files = db.files.len();
    let clean_files = db.clean_file_count();
    let error_count = db.error_count();
    let unresolved_imports = db.module_graph.unresolved.len();

    // Collect undefined type names and variable names
    let mut undefined_type_names: HashMap<String, usize> = HashMap::new();
    let mut undefined_var_names: HashMap<String, usize> = HashMap::new();
    let mut undefined_pred_names: HashMap<String, usize> = HashMap::new();
    for (_file_id, analysis) in &db.files {
        for diag in &analysis.diagnostics {
            if diag.severity == ocql_hir::Severity::Error {
                if let Some(rest) = diag.message.strip_prefix("undefined type `") {
                    if let Some(name) = rest.strip_suffix('`') {
                        *undefined_type_names.entry(name.to_string()).or_insert(0) += 1;
                    }
                } else if let Some(rest) = diag.message.strip_prefix("undefined variable `") {
                    if let Some(name) = rest.strip_suffix('`') {
                        *undefined_var_names.entry(name.to_string()).or_insert(0) += 1;
                    }
                } else if let Some(rest) = diag.message.strip_prefix("undefined predicate `") {
                    if let Some(name) = rest.split('`').next() {
                        *undefined_pred_names.entry(name.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }
    }

    // Collect error categories
    let mut error_categories: HashMap<String, usize> = HashMap::new();
    let mut error_examples: HashMap<String, Vec<(String, String)>> = HashMap::new();

    for (file_id, analysis) in &db.files {
        for diag in &analysis.diagnostics {
            if diag.severity == ocql_hir::Severity::Error {
                let category = categorize_error(&diag.message);
                *error_categories.entry(category.clone()).or_insert(0) += 1;

                let examples = error_examples.entry(category).or_default();
                if examples.len() < 3 {
                    let path = db.sources.path(*file_id);
                    examples.push((
                        path.display().to_string(),
                        diag.message.clone(),
                    ));
                }
            }
        }
    }

    // Per-file error counts
    let mut files_by_error_count: Vec<(PathBuf, usize)> = Vec::new();
    for (&file_id, analysis) in &db.files {
        let errs = analysis
            .diagnostics
            .iter()
            .filter(|d| d.severity == ocql_hir::Severity::Error)
            .count();
        if errs > 0 {
            files_by_error_count.push((db.sources.path(file_id).to_path_buf(), errs));
        }
    }
    files_by_error_count.sort_by(|a, b| b.1.cmp(&a.1));

    // Report
    println!("═══════════════════════════════════════════════════");
    println!("  HIR Analysis Report");
    println!("═══════════════════════════════════════════════════");
    println!();
    println!(
        "  Files analyzed:    {total_files:>6}"
    );
    println!(
        "  Clean (no errors): {clean_files:>6}  ({:.1}%)",
        if total_files > 0 {
            clean_files as f64 / total_files as f64 * 100.0
        } else {
            0.0
        }
    );
    println!(
        "  With errors:       {:>6}  ({:.1}%)",
        total_files - clean_files,
        if total_files > 0 {
            (total_files - clean_files) as f64 / total_files as f64 * 100.0
        } else {
            0.0
        }
    );
    println!("  Total errors:      {error_count:>6}");
    println!("  Unresolved imports:{unresolved_imports:>6}");
    println!("  Time:              {:.2}s", elapsed.as_secs_f64());
    println!();

    // Global diagnostics
    for diag in &db.diagnostics {
        println!("  [global] {diag}");
    }

    // Error category breakdown
    if !error_categories.is_empty() {
        println!("───────────────────────────────────────────────────");
        println!("  Error Categories");
        println!("───────────────────────────────────────────────────");

        let mut sorted_cats: Vec<_> = error_categories.iter().collect();
        sorted_cats.sort_by(|a, b| b.1.cmp(a.1));

        for (cat, count) in &sorted_cats {
            let pct = **count as f64 / error_count as f64 * 100.0;
            println!("  {count:>6} ({pct:>5.1}%)  {cat}");
        }
        println!();
    }

    // Show example errors
    if show_errors && !error_examples.is_empty() {
        println!("───────────────────────────────────────────────────");
        println!("  Error Examples (up to 3 per category)");
        println!("───────────────────────────────────────────────────");

        let mut sorted_cats: Vec<_> = error_categories.iter().collect();
        sorted_cats.sort_by(|a, b| b.1.cmp(a.1));

        let display_limit = limit.unwrap_or(5);
        for (cat, _) in sorted_cats.iter().take(display_limit) {
            println!();
            println!("  [{cat}]");
            if let Some(examples) = error_examples.get(*cat) {
                for (path, msg) in examples {
                    println!("    {path}");
                    println!("      {msg}");
                }
            }
        }
        println!();
    }

    // Verbose: show worst files
    if verbose && !files_by_error_count.is_empty() {
        println!("───────────────────────────────────────────────────");
        println!("  Files with Most Errors (top 20)");
        println!("───────────────────────────────────────────────────");

        let display_limit = limit.unwrap_or(20);
        for (path, count) in files_by_error_count.iter().take(display_limit) {
            println!("  {count:>4} errors  {}", path.display());
        }
        println!();
    }

    // Show unresolved import examples
    if !db.module_graph.unresolved.is_empty() {
        println!("───────────────────────────────────────────────────");
        println!("  Unresolved Import Examples (top 10)");
        println!("───────────────────────────────────────────────────");

        let mut import_freq: HashMap<String, usize> = HashMap::new();
        for u in &db.module_graph.unresolved {
            let path_str = u.import_path.join(".");
            *import_freq.entry(path_str).or_insert(0) += 1;
        }
        let mut sorted: Vec<_> = import_freq.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));

        for (path, count) in sorted.iter().take(10) {
            println!("  {count:>4}x  import {path}");
        }
        println!();
    }

    // Show top undefined type names
    if !undefined_type_names.is_empty() {
        println!("───────────────────────────────────────────────────");
        println!("  Top Undefined Type Names");
        println!("───────────────────────────────────────────────────");

        let mut sorted: Vec<_> = undefined_type_names.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));

        for (name, count) in sorted.iter().take(30) {
            println!("  {count:>4}x  {name}");
        }
        println!();
    }

    // Show top undefined variable names
    if !undefined_var_names.is_empty() {
        println!("───────────────────────────────────────────────────");
        println!("  Top Undefined Variable Names");
        println!("───────────────────────────────────────────────────");

        let mut sorted: Vec<_> = undefined_var_names.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));

        for (name, count) in sorted.iter().take(30) {
            println!("  {count:>4}x  {name}");
        }
        println!();
    }

    // Show top undefined predicate names
    if !undefined_pred_names.is_empty() {
        println!("───────────────────────────────────────────────────");
        println!("  Top Undefined Predicate Names");
        println!("───────────────────────────────────────────────────");

        let mut sorted: Vec<_> = undefined_pred_names.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));

        for (name, count) in sorted.iter().take(30) {
            println!("  {count:>4}x  {name}");
        }
        println!();
    }

    println!("═══════════════════════════════════════════════════");
}

/// Categorize an error message into a bucket.
fn categorize_error(msg: &str) -> String {
    if msg.contains("undefined type") {
        "undefined type".to_string()
    } else if msg.contains("undefined predicate") {
        "undefined predicate".to_string()
    } else if msg.contains("undefined variable") {
        "undefined variable".to_string()
    } else if msg.contains("undefined module") {
        "undefined module".to_string()
    } else if msg.contains("`this` used outside") {
        "`this` outside class".to_string()
    } else if msg.contains("`result` used outside") {
        "`result` outside predicate".to_string()
    } else if msg.contains("cannot compare") {
        "type mismatch".to_string()
    } else if msg.contains("module-qualified") {
        "module-qualified access".to_string()
    } else if msg.contains("not defined for") {
        "operator type error".to_string()
    } else {
        "other".to_string()
    }
}
