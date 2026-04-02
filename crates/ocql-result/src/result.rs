//! Core query result types.
//!
//! A `QueryResult` holds the evaluated output of a CodeQL-style query:
//! column definitions + rows of resolved values.

use crate::metadata::QueryMetadata;

/// The type of a column in the result set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnType {
    String,
    Int,
    Float,
    Entity,
}

/// A column definition in the result set.
#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub col_type: ColumnType,
}

/// A single cell value in the result table, fully resolved (no interned handles).
#[derive(Debug, Clone, PartialEq)]
pub enum ResultValue {
    String(String),
    Int(i64),
    Float(f64),
    Entity(u64),
}

impl std::fmt::Display for ResultValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResultValue::String(s) => write!(f, "{}", s),
            ResultValue::Int(i) => write!(f, "{}", i),
            ResultValue::Float(v) => write!(f, "{}", v),
            ResultValue::Entity(id) => write!(f, "#{}", id),
        }
    }
}

/// A complete query result: metadata + column defs + rows.
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub metadata: QueryMetadata,
    pub columns: Vec<ColumnDef>,
    pub rows: Vec<Vec<ResultValue>>,
}

impl QueryResult {
    pub fn new(metadata: QueryMetadata, columns: Vec<ColumnDef>, rows: Vec<Vec<ResultValue>>) -> Self {
        Self { metadata, columns, rows }
    }

    pub fn num_columns(&self) -> usize {
        self.columns.len()
    }

    pub fn num_rows(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}
