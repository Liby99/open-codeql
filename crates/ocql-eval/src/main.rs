use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use ocql_ql_parser::{parse_source_file, ParseError};

fn collect_ql_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if !dir.is_dir() {
        return files;
    }
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_ql_files(&path));
        } else if let Some(ext) = path.extension() {
            if ext == "ql" || ext == "qll" {
                files.push(path);
            }
        }
    }
    files
}

/// Extract a short error category from a ParseError.
fn categorize_error(err: &ParseError) -> String {
    match err {
        ParseError::InvalidToken { .. } => "InvalidToken".to_string(),
        ParseError::UnrecognizedEof { expected, .. } => {
            format!("UnrecognizedEof(expected {} tokens)", expected.len())
        }
        ParseError::UnrecognizedToken { token, expected, .. } => {
            let tok_str = format!("{:?}", token.1);
            // Truncate long token debug strings
            let tok_short = if tok_str.len() > 40 {
                format!("{}...", &tok_str[..40])
            } else {
                tok_str
            };
            format!("UnrecognizedToken({}, expected {} tokens)", tok_short, expected.len())
        }
        ParseError::ExtraToken { token, .. } => {
            format!("ExtraToken({:?})", token.1)
        }
        ParseError::User { error } => {
            format!("LexError({})", error)
        }
    }
}

/// Extract the first unrecognized token kind (for aggregation).
fn error_token_kind(err: &ParseError) -> String {
    match err {
        ParseError::InvalidToken { .. } => "InvalidToken".to_string(),
        ParseError::UnrecognizedEof { .. } => "UnexpectedEOF".to_string(),
        ParseError::UnrecognizedToken { token, .. } => {
            format!("{:?}", token.1)
        }
        ParseError::ExtraToken { token, .. } => {
            format!("Extra:{:?}", token.1)
        }
        ParseError::User { .. } => "LexError".to_string(),
    }
}

/// Extract the first few expected tokens for reporting.
fn expected_tokens(err: &ParseError) -> Vec<String> {
    match err {
        ParseError::UnrecognizedToken { expected, .. }
        | ParseError::UnrecognizedEof { expected, .. } => {
            expected.iter().take(10).cloned().collect()
        }
        _ => vec![],
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: ocql-eval <directory> [--verbose] [--limit N] [--lang LANG]");
        eprintln!("  Parses all .ql/.qll files in <directory> and reports results.");
        eprintln!("  --verbose    Show each file result");
        eprintln!("  --limit N    Only process first N files");
        eprintln!("  --lang LANG  Filter to files under a language dir (cpp, java, etc.)");
        eprintln!("  --errors     Only show errors (with --verbose)");
        std::process::exit(1);
    }

    let dir = PathBuf::from(&args[1]);
    let verbose = args.contains(&"--verbose".to_string());
    let errors_only = args.contains(&"--errors".to_string());
    let limit: Option<usize> = args
        .iter()
        .position(|a| a == "--limit")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok());
    let lang_filter: Option<&str> = args
        .iter()
        .position(|a| a == "--lang")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str());

    eprintln!("Collecting .ql/.qll files from {:?}...", dir);
    let mut files = collect_ql_files(&dir);
    files.sort();

    // Apply language filter
    if let Some(lang) = lang_filter {
        let lang_component = format!("/{}/", lang);
        files.retain(|f| f.to_string_lossy().contains(&lang_component));
    }

    // Apply limit
    if let Some(n) = limit {
        files.truncate(n);
    }

    let total = files.len();
    eprintln!("Found {} files to parse.", total);

    let mut success = 0usize;
    let mut fail = 0usize;
    let mut lex_errors = 0usize;
    let mut error_token_counts: HashMap<String, usize> = HashMap::new();
    let mut error_category_counts: HashMap<String, usize> = HashMap::new();
    let mut first_error_examples: HashMap<String, (String, String)> = HashMap::new(); // token_kind -> (file, detail)
    let mut fail_by_ext: HashMap<String, usize> = HashMap::new();
    let mut total_by_ext: HashMap<String, usize> = HashMap::new();
    let mut fail_by_lang: HashMap<String, usize> = HashMap::new();
    let mut total_by_lang: HashMap<String, usize> = HashMap::new();

    let base = dir.to_string_lossy().to_string();

    for (i, file) in files.iter().enumerate() {
        let rel = file
            .to_string_lossy()
            .strip_prefix(&base)
            .unwrap_or(&file.to_string_lossy())
            .trim_start_matches('/')
            .to_string();

        let ext = file
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Extract language from path (e.g., "cpp", "java")
        let lang = rel
            .split('/')
            .next()
            .unwrap_or("unknown")
            .to_string();

        *total_by_ext.entry(ext.clone()).or_default() += 1;
        *total_by_lang.entry(lang.clone()).or_default() += 1;

        let source = match fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                if verbose {
                    println!("SKIP  {} (read error: {})", rel, e);
                }
                continue;
            }
        };

        match parse_source_file(&source) {
            Ok(_) => {
                success += 1;
                if verbose && !errors_only {
                    println!("OK    {}", rel);
                }
            }
            Err(err) => {
                fail += 1;
                *fail_by_ext.entry(ext).or_default() += 1;
                *fail_by_lang.entry(lang).or_default() += 1;

                let tok_kind = error_token_kind(&err);
                let category = categorize_error(&err);

                *error_token_counts.entry(tok_kind.clone()).or_default() += 1;
                *error_category_counts.entry(category.clone()).or_default() += 1;

                if matches!(err, ParseError::User { .. }) {
                    lex_errors += 1;
                }

                if !first_error_examples.contains_key(&tok_kind) {
                    let detail = format!("{:?}", err);
                    first_error_examples.insert(tok_kind.clone(), (rel.clone(), detail));
                }

                if verbose {
                    let expected = expected_tokens(&err);
                    let exp_str = if expected.is_empty() {
                        String::new()
                    } else {
                        format!(" (expected: {})", expected.join(", "))
                    };
                    println!("FAIL  {} :: {}{}", rel, tok_kind, exp_str);
                }
            }
        }

        // Progress indicator
        if (i + 1) % 1000 == 0 {
            eprintln!("  ... processed {}/{}", i + 1, total);
        }
    }

    // ── Summary ──
    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  PARSE EVALUATION REPORT");
    println!("═══════════════════════════════════════════════════════════");
    println!();
    println!("Total files:    {}", total);
    println!("Parse success:  {} ({:.1}%)", success, 100.0 * success as f64 / total as f64);
    println!("Parse failure:  {} ({:.1}%)", fail, 100.0 * fail as f64 / total as f64);
    println!("  Lex errors:   {}", lex_errors);
    println!("  Parse errors: {}", fail - lex_errors);

    // By extension
    println!();
    println!("── By file extension ──");
    for ext in ["ql", "qll"] {
        let t = total_by_ext.get(ext).copied().unwrap_or(0);
        let f = fail_by_ext.get(ext).copied().unwrap_or(0);
        let s = t - f;
        if t > 0 {
            println!("  .{}: {}/{} pass ({:.1}%)", ext, s, t, 100.0 * s as f64 / t as f64);
        }
    }

    // By language
    println!();
    println!("── By language directory ──");
    let mut langs: Vec<_> = total_by_lang.iter().collect();
    langs.sort_by_key(|&(_, v)| std::cmp::Reverse(*v));
    for (lang, t) in &langs {
        let f = fail_by_lang.get(*lang).copied().unwrap_or(0);
        let s = *t - f;
        println!("  {:12} {}/{} pass ({:.1}%)", lang, s, t, 100.0 * s as f64 / **t as f64);
    }

    // Error token breakdown
    println!();
    println!("── Top error tokens (what token caused failure) ──");
    let mut tok_sorted: Vec<_> = error_token_counts.iter().collect();
    tok_sorted.sort_by_key(|&(_, v)| std::cmp::Reverse(*v));
    for (tok, count) in tok_sorted.iter().take(25) {
        let pct = 100.0 * **count as f64 / fail as f64;
        println!("  {:6} ({:5.1}%)  {}", count, pct, tok);

        // Show example
        if let Some((file, _detail)) = first_error_examples.get(*tok) {
            println!("                   e.g. {}", file);
        }
    }

    println!();
    println!("═══════════════════════════════════════════════════════════");
}
