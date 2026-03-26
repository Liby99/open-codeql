use std::path::PathBuf;

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        eprintln!("Usage: ocodeql parse <file.ql|file.qll> [--ast]");
        return Err("missing file path".into());
    }

    let mut show_ast = false;
    let mut file_path = None;

    for arg in args {
        match arg.as_str() {
            "--ast" => show_ast = true,
            "--help" | "-h" => {
                eprintln!("Usage: ocodeql parse <file.ql|file.qll> [--ast]");
                eprintln!();
                eprintln!("Options:");
                eprintln!("  --ast    Show full AST debug output (default: summary)");
                return Ok(());
            }
            _ => {
                if file_path.is_none() {
                    file_path = Some(PathBuf::from(arg));
                }
            }
        }
    }

    let file_path = file_path.ok_or("missing file path")?;
    if !file_path.exists() {
        return Err(format!("file not found: {}", file_path.display()).into());
    }

    let source = std::fs::read_to_string(&file_path)?;

    let ast = ocql_ql_parser::parse_source_file(&source)
        .map_err(|e| format!("parse error: {:?}", e))?;

    if show_ast {
        println!("{:#?}", ast);
    } else {
        print_summary(&ast);
    }

    Ok(())
}

fn print_summary(file: &ocql_ql_ast::module::SourceFile) {
    use ocql_ql_ast::module::ModuleMember;

    println!("Parse OK");
    println!();

    let mut imports = 0;
    let mut predicates = 0;
    let mut classes = 0;
    let mut modules = 0;
    let mut selects = 0;

    for member in &file.members {
        match member {
            ModuleMember::Import(_) => imports += 1,
            ModuleMember::Predicate(_) => predicates += 1,
            ModuleMember::Class(_) => classes += 1,
            ModuleMember::Module(_) => modules += 1,
            ModuleMember::Select(_) => selects += 1,
            _ => {}
        }
    }

    println!("  Imports:    {}", imports);
    println!("  Predicates: {}", predicates);
    println!("  Classes:    {}", classes);
    println!("  Modules:    {}", modules);
    if selects > 0 {
        println!("  Selects:    {}", selects);
    }
}
