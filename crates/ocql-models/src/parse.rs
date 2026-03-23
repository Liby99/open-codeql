//! Parse CodeQL `.model.yml` files.
//!
//! These YAML files define source/sink/summary models for data-flow analysis.
//! Each file contains extensions that add tuples to extensible predicates.

use serde::Deserialize;

use crate::access_path::parse_access_path;
use crate::types::*;

/// Raw YAML structure of a model file.
#[derive(Debug, Deserialize)]
pub struct ModelFile {
    pub extensions: Vec<Extension>,
}

#[derive(Debug, Deserialize)]
pub struct Extension {
    #[serde(rename = "addsTo")]
    pub adds_to: AddsTo,
    pub data: Vec<Vec<serde_yaml::Value>>,
}

#[derive(Debug, Deserialize)]
pub struct AddsTo {
    pub pack: String,
    pub extensible: String,
}

/// Errors from parsing model files.
#[derive(Debug)]
pub enum ModelParseError {
    Yaml(serde_yaml::Error),
    InvalidTuple { extensible: String, message: String },
}

impl std::fmt::Display for ModelParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelParseError::Yaml(e) => write!(f, "YAML parse error: {}", e),
            ModelParseError::InvalidTuple { extensible, message } => {
                write!(f, "invalid {} tuple: {}", extensible, message)
            }
        }
    }
}

impl std::error::Error for ModelParseError {}

/// Parse a `.model.yml` file from a YAML string.
pub fn parse_model_file(yaml: &str) -> Result<ModelStore, ModelParseError> {
    let file: ModelFile = serde_yaml::from_str(yaml).map_err(ModelParseError::Yaml)?;
    let mut store = ModelStore::new();

    for ext in &file.extensions {
        let extensible = &ext.adds_to.extensible;
        let pack = &ext.adds_to.pack;

        for row in &ext.data {
            match extensible.as_str() {
                "sourceModel" => {
                    let model = parse_source_model(row, pack, extensible)?;
                    store.sources.push(model);
                }
                "sinkModel" => {
                    let model = parse_sink_model(row, pack, extensible)?;
                    store.sinks.push(model);
                }
                "summaryModel" => {
                    let model = parse_summary_model(row, pack, extensible)?;
                    store.summaries.push(model);
                }
                "neutralModel" => {
                    let model = parse_neutral_model(row, extensible)?;
                    store.neutrals.push(model);
                }
                _ => {
                    // Unknown extensible — store as raw extension
                    store.raw_extensions.push(RawExtension {
                        pack: pack.clone(),
                        extensible: extensible.clone(),
                        data: row.iter().map(yaml_value_to_string).collect(),
                    });
                }
            }
        }
    }

    Ok(store)
}

fn yaml_value_to_string(v: &serde_yaml::Value) -> String {
    match v {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Null => String::new(),
        _ => format!("{:?}", v),
    }
}

fn get_str(row: &[serde_yaml::Value], idx: usize, extensible: &str) -> Result<String, ModelParseError> {
    row.get(idx)
        .map(yaml_value_to_string)
        .ok_or_else(|| ModelParseError::InvalidTuple {
            extensible: extensible.to_string(),
            message: format!("missing column {}", idx),
        })
}

fn get_bool(row: &[serde_yaml::Value], idx: usize, extensible: &str) -> Result<bool, ModelParseError> {
    match row.get(idx) {
        Some(serde_yaml::Value::Bool(b)) => Ok(*b),
        Some(serde_yaml::Value::String(s)) => match s.to_lowercase().as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(ModelParseError::InvalidTuple {
                extensible: extensible.to_string(),
                message: format!("column {} not boolean: {}", idx, s),
            }),
        },
        _ => Ok(false),
    }
}

/// Parse a sourceModel tuple:
/// (package, type, subtypes, name, signature, ext, output, kind, provenance)
fn parse_source_model(
    row: &[serde_yaml::Value],
    _pack: &str,
    extensible: &str,
) -> Result<SourceModel, ModelParseError> {
    if row.len() < 9 {
        return Err(ModelParseError::InvalidTuple {
            extensible: extensible.to_string(),
            message: format!("expected 9 columns, got {}", row.len()),
        });
    }

    let output_str = get_str(row, 6, extensible)?;
    let output = parse_access_path(&output_str).map_err(|e| ModelParseError::InvalidTuple {
        extensible: extensible.to_string(),
        message: format!("invalid output access path '{}': {}", output_str, e),
    })?;

    Ok(SourceModel {
        callable: CallableSpec {
            package: get_str(row, 0, extensible)?,
            type_name: get_str(row, 1, extensible)?,
            subtypes: get_bool(row, 2, extensible)?,
            name: get_str(row, 3, extensible)?,
            signature: get_str(row, 4, extensible)?,
        },
        output,
        kind: get_str(row, 7, extensible)?,
        provenance: get_str(row, 8, extensible)?,
    })
}

/// Parse a sinkModel tuple:
/// (package, type, subtypes, name, signature, ext, input, kind, provenance)
fn parse_sink_model(
    row: &[serde_yaml::Value],
    _pack: &str,
    extensible: &str,
) -> Result<SinkModel, ModelParseError> {
    if row.len() < 9 {
        return Err(ModelParseError::InvalidTuple {
            extensible: extensible.to_string(),
            message: format!("expected 9 columns, got {}", row.len()),
        });
    }

    let input_str = get_str(row, 6, extensible)?;
    let input = parse_access_path(&input_str).map_err(|e| ModelParseError::InvalidTuple {
        extensible: extensible.to_string(),
        message: format!("invalid input access path '{}': {}", input_str, e),
    })?;

    Ok(SinkModel {
        callable: CallableSpec {
            package: get_str(row, 0, extensible)?,
            type_name: get_str(row, 1, extensible)?,
            subtypes: get_bool(row, 2, extensible)?,
            name: get_str(row, 3, extensible)?,
            signature: get_str(row, 4, extensible)?,
        },
        input,
        kind: get_str(row, 7, extensible)?,
        provenance: get_str(row, 8, extensible)?,
    })
}

/// Parse a summaryModel tuple:
/// (package, type, subtypes, name, signature, ext, input, output, kind, provenance)
fn parse_summary_model(
    row: &[serde_yaml::Value],
    _pack: &str,
    extensible: &str,
) -> Result<SummaryModel, ModelParseError> {
    if row.len() < 10 {
        return Err(ModelParseError::InvalidTuple {
            extensible: extensible.to_string(),
            message: format!("expected 10 columns, got {}", row.len()),
        });
    }

    let input_str = get_str(row, 6, extensible)?;
    let input = parse_access_path(&input_str).map_err(|e| ModelParseError::InvalidTuple {
        extensible: extensible.to_string(),
        message: format!("invalid input access path '{}': {}", input_str, e),
    })?;

    let output_str = get_str(row, 7, extensible)?;
    let output = parse_access_path(&output_str).map_err(|e| ModelParseError::InvalidTuple {
        extensible: extensible.to_string(),
        message: format!("invalid output access path '{}': {}", output_str, e),
    })?;

    Ok(SummaryModel {
        callable: CallableSpec {
            package: get_str(row, 0, extensible)?,
            type_name: get_str(row, 1, extensible)?,
            subtypes: get_bool(row, 2, extensible)?,
            name: get_str(row, 3, extensible)?,
            signature: get_str(row, 4, extensible)?,
        },
        input,
        output,
        kind: get_str(row, 8, extensible)?,
        provenance: get_str(row, 9, extensible)?,
    })
}

/// Parse a neutralModel tuple:
/// (package, type, name, signature, kind, provenance)
fn parse_neutral_model(
    row: &[serde_yaml::Value],
    extensible: &str,
) -> Result<NeutralModel, ModelParseError> {
    if row.len() < 6 {
        return Err(ModelParseError::InvalidTuple {
            extensible: extensible.to_string(),
            message: format!("expected 6 columns, got {}", row.len()),
        });
    }

    Ok(NeutralModel {
        package: get_str(row, 0, extensible)?,
        type_name: get_str(row, 1, extensible)?,
        name: get_str(row, 2, extensible)?,
        signature: get_str(row, 3, extensible)?,
        kind: get_str(row, 4, extensible)?,
        provenance: get_str(row, 5, extensible)?,
    })
}
