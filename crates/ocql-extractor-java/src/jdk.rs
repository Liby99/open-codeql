//! JDK bytecode extraction — discovers and extracts .class files from the JDK.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use ocql_database::{Database, EntityId};
use ocql_extractor_common::FactEmitter;

use crate::bytecode;
use crate::bytecode_extract;

/// Discover the JDK home directory.
/// Tries these in order:
/// 1. JAVA_HOME environment variable
/// 2. /usr/libexec/java_home (macOS)
/// 3. Common paths
pub fn find_java_home() -> Option<PathBuf> {
    // 1. JAVA_HOME env var
    if let Ok(home) = std::env::var("JAVA_HOME") {
        let path = PathBuf::from(&home);
        if path.join("jmods").exists() || path.join("jre/lib/rt.jar").exists() {
            return Some(path);
        }
    }

    // 2. /usr/libexec/java_home (macOS)
    if let Ok(output) = std::process::Command::new("/usr/libexec/java_home")
        .output()
    {
        if output.status.success() {
            let home = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let path = PathBuf::from(&home);
            if path.exists() {
                return Some(path);
            }
        }
    }

    // 3. Common paths
    for candidate in &[
        "/usr/lib/jvm/default-java",
        "/usr/lib/jvm/java-21-openjdk-amd64",
        "/usr/lib/jvm/java-17-openjdk-amd64",
        "/usr/lib/jvm/java-11-openjdk-amd64",
    ] {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

/// Core JDK modules to extract by default.
/// These cover the essential Java SE APIs (java.lang, java.util, java.io, etc.).
/// Extracting only these keeps memory usage reasonable (~7.5K classes) instead of
/// extracting all 69 modules (~28K classes, which can use several GB of RAM).
const DEFAULT_JDK_MODULES: &[&str] = &[
    "java.base.jmod",
    "java.logging.jmod",
    "java.sql.jmod",
    "java.xml.jmod",
];

/// Extract JDK bytecode from the given JAVA_HOME into the database.
/// Returns the number of .class files successfully extracted.
///
/// By default, only extracts core modules (java.base) to keep memory bounded.
/// Use `extract_jdk_modules` for fine-grained control.
pub fn extract_jdk(db: &mut Database, java_home: &Path) -> Result<usize, String> {
    extract_jdk_modules(db, java_home, DEFAULT_JDK_MODULES)
}

/// Extract JDK bytecode from specific modules.
/// Pass `&["all"]` to extract everything, or specific module names like
/// `&["java.base.jmod", "java.sql.jmod"]`.
pub fn extract_jdk_modules(
    db: &mut Database,
    java_home: &Path,
    modules: &[&str],
) -> Result<usize, String> {
    let jmods_dir = java_home.join("jmods");

    if jmods_dir.exists() {
        // Java 9+ modules
        extract_from_jmods(db, &jmods_dir, modules)
    } else {
        let rt_jar = java_home.join("jre/lib/rt.jar");
        if rt_jar.exists() {
            // Java 8 rt.jar — no module filtering possible
            extract_from_jar(db, &rt_jar)
        } else {
            Err(format!(
                "No JDK class files found at {} (looked for jmods/ and jre/lib/rt.jar)",
                java_home.display()
            ))
        }
    }
}

/// Extract .class files from .jmod archives in the jmods directory.
/// If `modules` contains `"all"`, extracts from every .jmod file.
/// Otherwise, only extracts from the listed module files.
fn extract_from_jmods(
    db: &mut Database,
    jmods_dir: &Path,
    modules: &[&str],
) -> Result<usize, String> {
    let mut total = 0usize;
    let mut type_cache: HashMap<String, EntityId> = HashMap::new();
    let mut modifier_cache: HashMap<String, EntityId> = HashMap::new();
    let mut package_cache: HashMap<String, EntityId> = HashMap::new();

    // Create a synthetic file entity for bytecode-extracted classes
    let file_id;
    {
        let mut emitter = FactEmitter::new(db);
        file_id = emitter.alloc();
        emitter.emit_file(file_id, "<jdk-bytecode>");
    }

    let extract_all = modules.iter().any(|m| *m == "all");

    let mut entries: Vec<_> = std::fs::read_dir(jmods_dir)
        .map_err(|e| format!("Failed to read jmods dir: {}", e))?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            path.extension().map_or(false, |ext| ext == "jmod")
                && (extract_all
                    || modules.iter().any(|m| {
                        path.file_name()
                            .map_or(false, |f| f.to_string_lossy() == *m)
                    }))
        })
        .collect();
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let path = entry.path();
        match extract_from_archive(db, &path, "classes/", file_id, &mut type_cache, &mut modifier_cache, &mut package_cache) {
            Ok(count) => {
                eprintln!("  Extracted {} classes from {}", count, path.file_name().unwrap_or_default().to_string_lossy());
                total += count;
            }
            Err(e) => eprintln!("  Warning: failed to extract {}: {}", path.display(), e),
        }
    }

    Ok(total)
}

/// Extract .class files from a .jar file.
fn extract_from_jar(db: &mut Database, jar_path: &Path) -> Result<usize, String> {
    let mut type_cache: HashMap<String, EntityId> = HashMap::new();
    let mut modifier_cache: HashMap<String, EntityId> = HashMap::new();
    let mut package_cache: HashMap<String, EntityId> = HashMap::new();

    let file_id;
    {
        let mut emitter = FactEmitter::new(db);
        file_id = emitter.alloc();
        emitter.emit_file(file_id, "<jdk-bytecode>");
    }

    extract_from_archive(db, jar_path, "", file_id, &mut type_cache, &mut modifier_cache, &mut package_cache)
}

/// Extract .class files from a ZIP archive (jmod or jar).
/// `prefix` is stripped from entry names (e.g., "classes/" for jmod files).
fn extract_from_archive(
    db: &mut Database,
    archive_path: &Path,
    prefix: &str,
    file_id: EntityId,
    type_cache: &mut HashMap<String, EntityId>,
    modifier_cache: &mut HashMap<String, EntityId>,
    package_cache: &mut HashMap<String, EntityId>,
) -> Result<usize, String> {
    let file = std::fs::File::open(archive_path)
        .map_err(|e| format!("Failed to open {}: {}", archive_path.display(), e))?;
    let reader = std::io::BufReader::new(file);

    // jmod files have a 4-byte header "JM\x01\x00" before the ZIP data.
    // We need to detect this and skip it.
    let mut raw = Vec::new();
    {
        let mut unbuf = reader.into_inner();
        unbuf.read_to_end(&mut raw)
            .map_err(|e| format!("Failed to read {}: {}", archive_path.display(), e))?;
    }

    // Check for jmod header: "JM\x01\x00"
    let zip_data = if raw.len() >= 4 && &raw[..2] == b"JM" {
        &raw[4..] // Skip 4-byte jmod header
    } else {
        &raw[..]
    };

    let cursor = std::io::Cursor::new(zip_data);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| format!("Failed to read ZIP {}: {}", archive_path.display(), e))?;

    let mut count = 0usize;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)
            .map_err(|e| format!("Failed to read entry {}: {}", i, e))?;

        let name = entry.name().to_string();

        // Only process .class files under the prefix
        if !name.ends_with(".class") {
            continue;
        }
        let class_path = if !prefix.is_empty() {
            if let Some(stripped) = name.strip_prefix(prefix) {
                stripped
            } else {
                continue; // Not under the prefix
            }
        } else {
            &name
        };

        // Skip module-info.class and package-info.class
        let filename = class_path.rsplit('/').next().unwrap_or(class_path);
        if filename == "module-info.class" || filename == "package-info.class" {
            continue;
        }

        // Read the .class data
        let mut data = Vec::new();
        if entry.read_to_end(&mut data).is_err() {
            continue;
        }

        // Parse the class file
        let cf = match bytecode::parse_class(&data) {
            Ok(cf) => cf,
            Err(_) => continue, // Skip unparseable classes
        };

        // Extract facts into database
        let mut emitter = FactEmitter::new(db);
        bytecode_extract::extract_classfile(
            &mut emitter,
            &cf,
            file_id,
            type_cache,
            modifier_cache,
            package_cache,
        );

        count += 1;
    }

    Ok(count)
}
