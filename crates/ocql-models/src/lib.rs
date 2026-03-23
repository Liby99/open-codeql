//! CodeQL model file support — parse `.model.yml` files for data-flow analysis.
//!
//! These files define sources, sinks, and summaries that describe how tainted
//! data enters, exits, and flows through library code. This crate can:
//!
//! 1. Parse `.model.yml` files from CodeQL distributions
//! 2. Store parsed models in a queryable `ModelStore`
//! 3. Load models into the database as relations for Datalog querying

pub mod access_path;
pub mod database;
pub mod parse;
pub mod types;

pub use access_path::{AccessPath, AccessPathComponent, AccessPathRoot, ArgumentSpec};
pub use database::load_models_into_db;
pub use parse::{parse_model_file, ModelParseError};
pub use types::*;

use std::path::Path;

/// Load all `.model.yml` files from a directory into a ModelStore.
pub fn load_models_from_dir(dir: &Path) -> Result<ModelStore, ModelParseError> {
    let mut store = ModelStore::new();

    if !dir.exists() {
        return Ok(store);
    }

    let entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(entries) => entries.filter_map(|e| e.ok()).collect(),
        Err(_) => return Ok(store),
    };

    for entry in entries {
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "yml" || e == "yaml") {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".model.yml") || name.ends_with(".model.yaml") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        match parse_model_file(&content) {
                            Ok(file_store) => store.merge(file_store),
                            Err(e) => {
                                eprintln!("warning: failed to parse {}: {}", path.display(), e);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(store)
}

/// Load all `.model.yml` files from a directory and insert them into the database.
pub fn load_models_into_database(dir: &Path, db: &mut ocql_database::Database) -> Result<ModelStore, ModelParseError> {
    let store = load_models_from_dir(dir)?;
    load_models_into_db(&store, db);
    Ok(store)
}
