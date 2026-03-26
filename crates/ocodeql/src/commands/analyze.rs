use std::path::PathBuf;

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        print_usage();
        return Err("missing path".into());
    }

    let mut project_path = None;
    let mut show_errors = false;
    let mut verbose = false;

    for arg in args {
        match arg.as_str() {
            "--errors" => show_errors = true,
            "--verbose" | "-v" => verbose = true,
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            _ => {
                if project_path.is_none() {
                    project_path = Some(PathBuf::from(arg));
                }
            }
        }
    }

    let project_path = project_path.ok_or("missing project path")?;
    if !project_path.exists() {
        return Err(format!("path not found: {}", project_path.display()).into());
    }

    // Larger stack for deeply nested QL ASTs
    let builder = std::thread::Builder::new().stack_size(64 * 1024 * 1024);
    let handler = builder.spawn(move || -> Result<(), String> {
        run_analysis(&project_path, show_errors, verbose)
            .map_err(|e| e.to_string())
    })?;

    handler.join()
        .map_err(|_| -> Box<dyn std::error::Error> { "analysis thread panicked".into() })?
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })
}

fn run_analysis(
    project_path: &PathBuf,
    show_errors: bool,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Analyzing QL project: {}", project_path.display());

    let start = std::time::Instant::now();

    let hir_db = if project_path.is_file() {
        let source = std::fs::read_to_string(project_path)?;
        let path_str = project_path.to_string_lossy();
        ocql_hir::analyze_single_file(&source, &path_str)
    } else {
        ocql_hir::analyze_project(project_path)
    };

    let elapsed = start.elapsed();

    let total_files = hir_db.files.len();
    let clean_files = hir_db.clean_file_count();
    let error_count = hir_db.error_count();

    println!("=== HIR Analysis Results ===");
    println!("Total files:  {}", total_files);
    println!("Clean files:  {} ({:.1}%)",
        clean_files,
        if total_files > 0 { clean_files as f64 / total_files as f64 * 100.0 } else { 0.0 });
    println!("Errors:       {}", error_count);
    println!("Time:         {:.2}s", elapsed.as_secs_f64());

    if show_errors || verbose {
        let errors: Vec<_> = hir_db.all_errors().collect();
        if !errors.is_empty() {
            println!();
            println!("=== Errors ===");
            for diag in &errors {
                println!("  {}", hir_db.format_diagnostic(diag));
            }
        }
    }

    Ok(())
}

fn print_usage() {
    eprintln!("Usage: ocodeql analyze <project-dir|file.ql> [options]");
    eprintln!();
    eprintln!("Run HIR analysis (name resolution + type checking) on a QL project.");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --errors     Show all errors");
    eprintln!("  --verbose    Verbose output");
}
