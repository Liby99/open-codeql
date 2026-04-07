use std::path::{Path, PathBuf};
use std::time::Instant;

use ocql_database::Database;
use ocql_extractor_common::Extractor;
use ocql_extractor_cpp::{CppExtractor, cpp_schema};
use ocql_extractor_java::{JavaExtractor, java_schema};

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        print_usage();
        return Err("missing subcommand".into());
    }

    match args[0].as_str() {
        "create" => create(&args[1..]),
        "load" => load(&args[1..]),
        "help" | "--help" => {
            print_usage();
            Ok(())
        }
        other => Err(format!("unknown database subcommand: {}", other).into()),
    }
}

fn print_usage() {
    eprintln!("Usage: ocodeql database <subcommand> [options]");
    eprintln!();
    eprintln!("Subcommands:");
    eprintln!("  create   Create a database from source code");
    eprintln!("  load     Load and inspect an existing database");
    eprintln!();
    eprintln!("Create options:");
    eprintln!("  ocodeql database create --language <lang> --source <dir> --output <db-path>");
    eprintln!();
    eprintln!("  --language, -l   Language: cpp, c, java");
    eprintln!("  --source, -s     Source directory to extract");
    eprintln!("  --output, -o     Output database file path");
    eprintln!();
    eprintln!("Load options:");
    eprintln!("  ocodeql database load <db-path>");
}

fn create(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut language = None;
    let mut source_dir = None;
    let mut output_path = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--language" | "-l" => {
                i += 1;
                language = Some(args.get(i).ok_or("missing value for --language")?.clone());
            }
            "--source" | "-s" => {
                i += 1;
                source_dir = Some(PathBuf::from(args.get(i).ok_or("missing value for --source")?));
            }
            "--output" | "-o" => {
                i += 1;
                output_path = Some(PathBuf::from(args.get(i).ok_or("missing value for --output")?));
            }
            other => return Err(format!("unknown option: {}", other).into()),
        }
        i += 1;
    }

    let language = language.ok_or("--language is required")?;
    let source_dir = source_dir.ok_or("--source is required")?;
    let output_path = output_path.ok_or("--output is required")?;

    if !source_dir.exists() {
        return Err(format!("source directory does not exist: {}", source_dir.display()).into());
    }

    eprintln!("Creating database for language '{}' from: {}", language, source_dir.display());
    let start = Instant::now();

    let mut db = create_db_for_language(&language)?;
    let results = extract_for_language(&language, &mut db, &source_dir)?;

    let success_count = results.iter().filter(|r| r.success).count();
    let fail_count = results.iter().filter(|r| !r.success).count();

    eprintln!("Extracted {} files ({} ok, {} failed) in {:.2}s",
        results.len(), success_count, fail_count, start.elapsed().as_secs_f64());

    // For Java, also extract JDK bytecode
    if language == "java" {
        if let Some(java_home) = ocql_extractor_java::jdk::find_java_home() {
            eprintln!("Extracting JDK bytecode from: {}", java_home.display());
            let jdk_start = Instant::now();
            match ocql_extractor_java::jdk::extract_jdk(&mut db, &java_home) {
                Ok(count) => {
                    eprintln!("Extracted {} JDK classes in {:.2}s",
                        count, jdk_start.elapsed().as_secs_f64());
                }
                Err(e) => {
                    eprintln!("Warning: JDK extraction failed: {}", e);
                }
            }
        } else {
            eprintln!("Warning: JAVA_HOME not found, skipping JDK bytecode extraction");
        }

        // Re-resolve call/variable bindings now that JDK methods are available
        eprintln!("Resolving call bindings (with JDK)...");
        ocql_extractor_java::resolve_bindings(&mut db);
    }

    for r in &results {
        if !r.success {
            if let Some(ref err) = r.error {
                eprintln!("  FAIL: {} — {}", r.file_path, err);
            }
        }
    }

    // Save database
    eprintln!("Saving database to: {}", output_path.display());
    ocql_database::save_to_file(&db, &output_path)?;

    let metadata = std::fs::metadata(&output_path)?;
    eprintln!("Database saved ({:.1} KB)", metadata.len() as f64 / 1024.0);

    Ok(())
}

fn create_db_for_language(language: &str) -> Result<Database, Box<dyn std::error::Error>> {
    match language {
        "cpp" | "c" | "c++" => Ok(Database::from_schema(cpp_schema())),
        "java" => Ok(Database::from_schema(java_schema())),
        _ => Err(format!("unsupported language: {} (supported: cpp, c, java)", language).into()),
    }
}

fn extract_for_language(
    language: &str,
    db: &mut Database,
    source_dir: &Path,
) -> Result<Vec<ocql_extractor_common::ExtractionResult>, Box<dyn std::error::Error>> {
    match language {
        "cpp" | "c++" => {
            let extractor = CppExtractor::cpp();
            Ok(extractor.extract_directory(db, source_dir))
        }
        "c" => {
            let extractor = CppExtractor::c();
            Ok(extractor.extract_directory(db, source_dir))
        }
        "java" => {
            let extractor = JavaExtractor::new();
            Ok(extractor.extract_directory(db, source_dir))
        }
        _ => Err(format!("unsupported language: {}", language).into()),
    }
}

fn load(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = args.first().ok_or("usage: ocodeql database load <db-path>")?;
    let path = PathBuf::from(db_path);

    if !path.exists() {
        return Err(format!("database file does not exist: {}", path.display()).into());
    }

    eprintln!("Loading database from: {}", path.display());
    let db = ocql_database::load_from_file(&path)?;

    println!("Database loaded successfully.");
    println!();
    println!("Relations:");

    let mut names: Vec<String> = db.relation_names().map(|s| s.to_string()).collect();
    names.sort();

    for name in &names {
        if let Some(iter) = db.scan(name) {
            let count = iter.count();
            if count > 0 {
                println!("  {:40} {:>8} tuples", name, count);
            }
        }
    }

    let total: usize = names.iter()
        .filter_map(|n| db.scan(n).map(|i| i.count()))
        .sum();
    println!();
    println!("Total: {} relations, {} tuples", names.len(), total);

    Ok(())
}
