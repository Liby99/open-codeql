//! Extract query results from an evaluated database.
//!
//! After evaluation, `select_result_*` relations contain the query output.
//! This module reads those relations and produces a `QueryResult`.

use ocql_database::{Database, Value};

use crate::metadata::QueryMetadata;
use crate::result::{ColumnDef, ColumnType, QueryResult, ResultValue};

/// Extract query results from the database after evaluation.
///
/// Finds all `select_result_*` relations, reads their tuples, resolves
/// interned strings, and returns a `QueryResult`.
pub fn extract_query_result(db: &Database, metadata: QueryMetadata) -> QueryResult {
    // Find select_result relations (there should be exactly one for a simple query)
    let mut select_names: Vec<String> = db
        .relation_names()
        .filter(|n| n.starts_with("select_result"))
        .map(|n| n.to_string())
        .collect();
    select_names.sort();

    if select_names.is_empty() {
        return QueryResult::new(metadata, vec![], vec![]);
    }

    // Use the first (or only) select_result relation
    let select_name = &select_names[0];
    let Some(iter) = db.scan(select_name) else {
        return QueryResult::new(metadata, vec![], vec![]);
    };

    let tuples: Vec<_> = iter.collect();
    if tuples.is_empty() {
        return QueryResult::new(metadata, vec![], vec![]);
    }

    // Infer column count from first tuple
    let ncols = tuples[0].len();

    // Infer column types from first tuple
    let columns: Vec<ColumnDef> = (0..ncols)
        .map(|i| {
            let col_type = match &tuples[0][i] {
                Value::String(_) => ColumnType::String,
                Value::Int(_) => ColumnType::Int,
                Value::Float(_) => ColumnType::Float,
                Value::Entity(_) => ColumnType::Entity,
                _ => ColumnType::String,
            };
            ColumnDef {
                name: format!("col{}", i),
                col_type,
            }
        })
        .collect();

    // Resolve all values
    let rows: Vec<Vec<ResultValue>> = tuples
        .iter()
        .map(|tuple| {
            tuple
                .iter()
                .map(|v| resolve_value(db, v))
                .collect()
        })
        .collect();

    QueryResult::new(metadata, columns, rows)
}

/// Extract results from a specific named relation (not just select_result).
pub fn extract_from_relation(
    db: &Database,
    relation_name: &str,
    metadata: QueryMetadata,
) -> QueryResult {
    let Some(iter) = db.scan(relation_name) else {
        return QueryResult::new(metadata, vec![], vec![]);
    };

    let tuples: Vec<_> = iter.collect();
    if tuples.is_empty() {
        return QueryResult::new(metadata, vec![], vec![]);
    }

    let ncols = tuples[0].len();
    let columns: Vec<ColumnDef> = (0..ncols)
        .map(|i| {
            let col_type = match &tuples[0][i] {
                Value::String(_) => ColumnType::String,
                Value::Int(_) => ColumnType::Int,
                Value::Float(_) => ColumnType::Float,
                Value::Entity(_) => ColumnType::Entity,
                _ => ColumnType::String,
            };
            ColumnDef {
                name: format!("col{}", i),
                col_type,
            }
        })
        .collect();

    let rows: Vec<Vec<ResultValue>> = tuples
        .iter()
        .map(|tuple| tuple.iter().map(|v| resolve_value(db, v)).collect())
        .collect();

    QueryResult::new(metadata, columns, rows)
}

fn resolve_value(db: &Database, v: &Value) -> ResultValue {
    match v {
        Value::String(s) => ResultValue::String(db.strings.resolve(*s).to_string()),
        Value::Int(i) => ResultValue::Int(*i),
        Value::Float(f) => ResultValue::Float(f.0),
        Value::Entity(e) => ResultValue::Entity(e.0),
        Value::Bool(b) => ResultValue::String(b.to_string()),
        Value::Null => ResultValue::String("null".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocql_database::Database;

    #[test]
    fn extract_empty_result() {
        let db = Database::empty();
        let meta = QueryMetadata::default();
        let result = extract_query_result(&db, meta);
        assert!(result.is_empty());
    }
}
