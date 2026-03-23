//! Load models into the database as queryable relations.
//!
//! Creates relations `sourceModel`, `sinkModel`, `summaryModel`, `neutralModel`
//! that can be queried by the Datalog engine.

use ocql_database::{Database, Value, RelationSchema, ColumnDef};
use ocql_schema::ColumnType;

use crate::access_path::AccessPath;
use crate::types::ModelStore;

fn str_col(name: &str) -> ColumnDef {
    ColumnDef { name: name.to_string(), col_type: ColumnType::String }
}

fn bool_col(name: &str) -> ColumnDef {
    ColumnDef { name: name.to_string(), col_type: ColumnType::Boolean }
}

fn ensure_model_relations(db: &mut Database) {
    if db.scan("sourceModel").is_none() {
        db.add_relation("sourceModel", RelationSchema {
            name: "sourceModel".to_string(),
            columns: vec![
                str_col("package"), str_col("type"), bool_col("subtypes"),
                str_col("name"), str_col("signature"), str_col("output"),
                str_col("kind"), str_col("provenance"),
            ],
        });
    }
    if db.scan("sinkModel").is_none() {
        db.add_relation("sinkModel", RelationSchema {
            name: "sinkModel".to_string(),
            columns: vec![
                str_col("package"), str_col("type"), bool_col("subtypes"),
                str_col("name"), str_col("signature"), str_col("input"),
                str_col("kind"), str_col("provenance"),
            ],
        });
    }
    if db.scan("summaryModel").is_none() {
        db.add_relation("summaryModel", RelationSchema {
            name: "summaryModel".to_string(),
            columns: vec![
                str_col("package"), str_col("type"), bool_col("subtypes"),
                str_col("name"), str_col("signature"), str_col("input"),
                str_col("output"), str_col("kind"), str_col("provenance"),
            ],
        });
    }
    if db.scan("neutralModel").is_none() {
        db.add_relation("neutralModel", RelationSchema {
            name: "neutralModel".to_string(),
            columns: vec![
                str_col("package"), str_col("type"), str_col("name"),
                str_col("signature"), str_col("kind"), str_col("provenance"),
            ],
        });
    }
}

/// Load all models from a ModelStore into the database as relations.
///
/// Creates the following relations:
/// - `sourceModel(package, type, subtypes, name, signature, output, kind, provenance)`
/// - `sinkModel(package, type, subtypes, name, signature, input, kind, provenance)`
/// - `summaryModel(package, type, subtypes, name, signature, input, output, kind, provenance)`
/// - `neutralModel(package, type, name, signature, kind, provenance)`
pub fn load_models_into_db(store: &ModelStore, db: &mut Database) {
    ensure_model_relations(db);
    // sourceModel
    for source in &store.sources {
        let tuple = vec![
            db.intern_string(&source.callable.package),
            db.intern_string(&source.callable.type_name),
            Value::Bool(source.callable.subtypes),
            db.intern_string(&source.callable.name),
            db.intern_string(&source.callable.signature),
            db.intern_string(&format_access_path(&source.output)),
            db.intern_string(&source.kind),
            db.intern_string(&source.provenance),
        ];
        let _ = db.insert("sourceModel", tuple.into());
    }

    // sinkModel
    for sink in &store.sinks {
        let tuple = vec![
            db.intern_string(&sink.callable.package),
            db.intern_string(&sink.callable.type_name),
            Value::Bool(sink.callable.subtypes),
            db.intern_string(&sink.callable.name),
            db.intern_string(&sink.callable.signature),
            db.intern_string(&format_access_path(&sink.input)),
            db.intern_string(&sink.kind),
            db.intern_string(&sink.provenance),
        ];
        let _ = db.insert("sinkModel", tuple.into());
    }

    // summaryModel
    for summary in &store.summaries {
        let tuple = vec![
            db.intern_string(&summary.callable.package),
            db.intern_string(&summary.callable.type_name),
            Value::Bool(summary.callable.subtypes),
            db.intern_string(&summary.callable.name),
            db.intern_string(&summary.callable.signature),
            db.intern_string(&format_access_path(&summary.input)),
            db.intern_string(&format_access_path(&summary.output)),
            db.intern_string(&summary.kind),
            db.intern_string(&summary.provenance),
        ];
        let _ = db.insert("summaryModel", tuple.into());
    }

    // neutralModel
    for neutral in &store.neutrals {
        let tuple = vec![
            db.intern_string(&neutral.package),
            db.intern_string(&neutral.type_name),
            db.intern_string(&neutral.name),
            db.intern_string(&neutral.signature),
            db.intern_string(&neutral.kind),
            db.intern_string(&neutral.provenance),
        ];
        let _ = db.insert("neutralModel", tuple.into());
    }
}

/// Format an access path back to string representation for storage.
fn format_access_path(ap: &AccessPath) -> String {
    use crate::access_path::*;

    let mut s = match &ap.root {
        AccessPathRoot::Argument(spec) => {
            let spec_str = match spec {
                ArgumentSpec::Index(n) => n.to_string(),
                ArgumentSpec::This => "this".to_string(),
                ArgumentSpec::Range(lo, hi) => format!("{}..{}", lo, hi),
                ArgumentSpec::Deref(n) => format!("*{}", n),
                ArgumentSpec::DerefDeref(n) => format!("**{}", n),
                ArgumentSpec::DerefElement(n) => format!("*@{}", n),
                ArgumentSpec::All => "*".to_string(),
            };
            format!("Argument[{}]", spec_str)
        }
        AccessPathRoot::ReturnValue(None) => "ReturnValue".to_string(),
        AccessPathRoot::ReturnValue(Some(q)) => format!("ReturnValue[{}]", q),
        AccessPathRoot::Empty => String::new(),
    };

    for comp in &ap.components {
        match comp {
            AccessPathComponent::Field(name) => s.push_str(&format!(".Field[{}]", name)),
            AccessPathComponent::DerefField(name) => s.push_str(&format!(".Field[*{}]", name)),
            AccessPathComponent::SyntheticField(name) => {
                s.push_str(&format!(".SyntheticField[{}]", name));
            }
            AccessPathComponent::MapKey => s.push_str(".MapKey"),
            AccessPathComponent::MapValue => s.push_str(".MapValue"),
            AccessPathComponent::ArrayElement => s.push_str(".ArrayElement"),
            AccessPathComponent::Element => s.push_str(".Element[@]"),
        }
    }

    s
}
