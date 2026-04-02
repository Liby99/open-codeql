//! Query result storage and decoding for open-cql.
//!
//! This crate handles the output side of the query pipeline:
//!   evaluate → extract results → serialize to `.obqrs` → decode to CSV/JSON/SARIF
//!
//! # Pipeline
//!
//! ```text
//! Database (post-eval) → extract_query_result() → QueryResult
//!                                                     ↓
//!                                          write_obqrs() → .obqrs file
//!                                                     ↓
//!                                          read_obqrs()  → QueryResult
//!                                                     ↓
//!                                         to_csv() / to_json() / to_sarif()
//! ```
//!
//! # Query kinds
//!
//! The `@kind` metadata in the QLDoc comment determines how results are interpreted:
//! - `problem`: alert-style — each row is (element, message)
//! - `path-problem`: data-flow — each row is (element, source, sink, message)
//! - `table`: raw tabular output
//! - `metric`: numeric metrics
//! - `diagnostic`: diagnostic messages

pub mod metadata;
pub mod result;
pub mod extract;
pub mod obqrs;
pub mod decode;

pub use metadata::{QueryKind, QueryMetadata, parse_query_metadata};
pub use result::{QueryResult, ResultValue, ColumnDef, ColumnType};
pub use extract::{extract_query_result, extract_from_relation};
pub use obqrs::{write_obqrs, read_obqrs, write_obqrs_file, read_obqrs_file};
pub use decode::{to_csv, to_json, to_sarif};
