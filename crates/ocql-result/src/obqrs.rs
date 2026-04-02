//! Binary `.obqrs` (Open Binary Query Result Sets) format.
//!
//! Format layout:
//! ```text
//! [magic: 5 bytes "OBQRS"]
//! [version: u16]
//! [metadata section]
//! [column defs section]
//! [row data section]
//! ```
//!
//! All multi-byte integers are little-endian.
//! Strings are length-prefixed: [len: u32][utf8 bytes].

use std::io::{self, Read, Write};

use crate::metadata::{QueryKind, QueryMetadata};
use crate::result::{ColumnDef, ColumnType, QueryResult, ResultValue};

const MAGIC: &[u8; 5] = b"OBQRS";
const VERSION: u16 = 1;

// ============================================================
// Write
// ============================================================

/// Write a QueryResult to the `.obqrs` binary format.
pub fn write_obqrs<W: Write>(w: &mut W, result: &QueryResult) -> io::Result<()> {
    // Header
    w.write_all(MAGIC)?;
    w.write_all(&VERSION.to_le_bytes())?;

    // Metadata
    write_metadata(w, &result.metadata)?;

    // Column defs
    w.write_all(&(result.columns.len() as u32).to_le_bytes())?;
    for col in &result.columns {
        write_string(w, &col.name)?;
        let type_tag: u8 = match col.col_type {
            ColumnType::String => 0,
            ColumnType::Int => 1,
            ColumnType::Float => 2,
            ColumnType::Entity => 3,
        };
        w.write_all(&[type_tag])?;
    }

    // Rows
    w.write_all(&(result.rows.len() as u64).to_le_bytes())?;
    for row in &result.rows {
        for val in row {
            write_value(w, val)?;
        }
    }

    Ok(())
}

/// Write a QueryResult to a file path.
pub fn write_obqrs_file(path: &str, result: &QueryResult) -> io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    write_obqrs(&mut f, result)
}

fn write_metadata<W: Write>(w: &mut W, meta: &QueryMetadata) -> io::Result<()> {
    write_opt_string(w, &meta.name)?;
    write_opt_string(w, &meta.description)?;
    // kind
    let kind_str = match &meta.kind {
        Some(QueryKind::Problem) => "problem",
        Some(QueryKind::PathProblem) => "path-problem",
        Some(QueryKind::Table) => "table",
        Some(QueryKind::Metric) => "metric",
        Some(QueryKind::Diagnostic) => "diagnostic",
        Some(QueryKind::Other(s)) => s.as_str(),
        None => "",
    };
    write_string(w, kind_str)?;
    write_opt_string(w, &meta.id)?;
    write_opt_string(w, &meta.problem_severity)?;
    write_opt_string(w, &meta.security_severity)?;
    // tags
    w.write_all(&(meta.tags.len() as u32).to_le_bytes())?;
    for tag in &meta.tags {
        write_string(w, tag)?;
    }
    Ok(())
}

fn write_string<W: Write>(w: &mut W, s: &str) -> io::Result<()> {
    let bytes = s.as_bytes();
    w.write_all(&(bytes.len() as u32).to_le_bytes())?;
    w.write_all(bytes)
}

fn write_opt_string<W: Write>(w: &mut W, s: &Option<String>) -> io::Result<()> {
    match s {
        Some(s) => {
            w.write_all(&[1])?;
            write_string(w, s)
        }
        None => w.write_all(&[0]),
    }
}

fn write_value<W: Write>(w: &mut W, v: &ResultValue) -> io::Result<()> {
    match v {
        ResultValue::String(s) => {
            w.write_all(&[0])?;
            write_string(w, s)
        }
        ResultValue::Int(i) => {
            w.write_all(&[1])?;
            w.write_all(&i.to_le_bytes())
        }
        ResultValue::Float(f) => {
            w.write_all(&[2])?;
            w.write_all(&f.to_le_bytes())
        }
        ResultValue::Entity(id) => {
            w.write_all(&[3])?;
            w.write_all(&id.to_le_bytes())
        }
    }
}

// ============================================================
// Read
// ============================================================

/// Read a QueryResult from the `.obqrs` binary format.
pub fn read_obqrs<R: Read>(r: &mut R) -> io::Result<QueryResult> {
    // Header
    let mut magic = [0u8; 5];
    r.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid OBQRS magic"));
    }
    let mut ver = [0u8; 2];
    r.read_exact(&mut ver)?;
    let version = u16::from_le_bytes(ver);
    if version != VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported OBQRS version {}", version),
        ));
    }

    // Metadata
    let metadata = read_metadata(r)?;

    // Columns
    let ncols = read_u32(r)? as usize;
    let mut columns = Vec::with_capacity(ncols);
    for _ in 0..ncols {
        let name = read_string(r)?;
        let mut tag = [0u8; 1];
        r.read_exact(&mut tag)?;
        let col_type = match tag[0] {
            0 => ColumnType::String,
            1 => ColumnType::Int,
            2 => ColumnType::Float,
            3 => ColumnType::Entity,
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("unknown column type tag {}", other),
                ))
            }
        };
        columns.push(ColumnDef { name, col_type });
    }

    // Rows
    let nrows = read_u64(r)? as usize;
    let mut rows = Vec::with_capacity(nrows);
    for _ in 0..nrows {
        let mut row = Vec::with_capacity(ncols);
        for _ in 0..ncols {
            row.push(read_value(r)?);
        }
        rows.push(row);
    }

    Ok(QueryResult::new(metadata, columns, rows))
}

/// Read a QueryResult from a file path.
pub fn read_obqrs_file(path: &str) -> io::Result<QueryResult> {
    let mut f = std::fs::File::open(path)?;
    read_obqrs(&mut f)
}

fn read_metadata<R: Read>(r: &mut R) -> io::Result<QueryMetadata> {
    let name = read_opt_string(r)?;
    let description = read_opt_string(r)?;
    let kind_str = read_string(r)?;
    let kind = if kind_str.is_empty() {
        None
    } else {
        Some(QueryKind::parse_str(&kind_str))
    };
    let id = read_opt_string(r)?;
    let problem_severity = read_opt_string(r)?;
    let security_severity = read_opt_string(r)?;
    let ntags = read_u32(r)? as usize;
    let mut tags = Vec::with_capacity(ntags);
    for _ in 0..ntags {
        tags.push(read_string(r)?);
    }
    Ok(QueryMetadata {
        name,
        description,
        kind,
        id,
        problem_severity,
        security_severity,
        tags,
    })
}

fn read_string<R: Read>(r: &mut R) -> io::Result<String> {
    let len = read_u32(r)? as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    String::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn read_opt_string<R: Read>(r: &mut R) -> io::Result<Option<String>> {
    let mut tag = [0u8; 1];
    r.read_exact(&mut tag)?;
    if tag[0] == 0 {
        Ok(None)
    } else {
        Ok(Some(read_string(r)?))
    }
}

fn read_value<R: Read>(r: &mut R) -> io::Result<ResultValue> {
    let mut tag = [0u8; 1];
    r.read_exact(&mut tag)?;
    match tag[0] {
        0 => Ok(ResultValue::String(read_string(r)?)),
        1 => Ok(ResultValue::Int(read_i64(r)?)),
        2 => Ok(ResultValue::Float(read_f64(r)?)),
        3 => Ok(ResultValue::Entity(read_u64(r)?)),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown value tag {}", other),
        )),
    }
}

fn read_u32<R: Read>(r: &mut R) -> io::Result<u32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u64<R: Read>(r: &mut R) -> io::Result<u64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

fn read_i64<R: Read>(r: &mut R) -> io::Result<i64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(i64::from_le_bytes(buf))
}

fn read_f64<R: Read>(r: &mut R) -> io::Result<f64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(f64::from_le_bytes(buf))
}

// Helper for QueryKind — needed by the reader
impl QueryKind {
    pub(crate) fn parse_str(s: &str) -> Self {
        match s {
            "problem" => Self::Problem,
            "path-problem" => Self::PathProblem,
            "table" => Self::Table,
            "metric" => Self::Metric,
            "diagnostic" => Self::Diagnostic,
            other => Self::Other(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::QueryKind;

    fn sample_result() -> QueryResult {
        QueryResult::new(
            QueryMetadata {
                name: Some("Test query".to_string()),
                description: Some("A test".to_string()),
                kind: Some(QueryKind::Problem),
                id: Some("test/query".to_string()),
                problem_severity: Some("error".to_string()),
                security_severity: Some("9.0".to_string()),
                tags: vec!["security".to_string(), "cwe-120".to_string()],
            },
            vec![
                ColumnDef { name: "callee".to_string(), col_type: ColumnType::String },
                ColumnDef { name: "caller".to_string(), col_type: ColumnType::String },
            ],
            vec![
                vec![ResultValue::String("gets".to_string()), ResultValue::String("main".to_string())],
                vec![ResultValue::String("strcpy".to_string()), ResultValue::String("copy_buf".to_string())],
            ],
        )
    }

    #[test]
    fn round_trip_obqrs() {
        let result = sample_result();
        let mut buf = Vec::new();
        write_obqrs(&mut buf, &result).unwrap();

        let mut cursor = std::io::Cursor::new(buf);
        let decoded = read_obqrs(&mut cursor).unwrap();

        assert_eq!(decoded.metadata.name, result.metadata.name);
        assert_eq!(decoded.metadata.kind, result.metadata.kind);
        assert_eq!(decoded.metadata.id, result.metadata.id);
        assert_eq!(decoded.metadata.tags, result.metadata.tags);
        assert_eq!(decoded.columns.len(), 2);
        assert_eq!(decoded.rows.len(), 2);
        assert_eq!(decoded.rows[0][0], ResultValue::String("gets".to_string()));
        assert_eq!(decoded.rows[1][1], ResultValue::String("copy_buf".to_string()));
    }

    #[test]
    fn round_trip_with_int_and_float() {
        let result = QueryResult::new(
            QueryMetadata::default(),
            vec![
                ColumnDef { name: "id".to_string(), col_type: ColumnType::Int },
                ColumnDef { name: "score".to_string(), col_type: ColumnType::Float },
            ],
            vec![
                vec![ResultValue::Int(42), ResultValue::Float(3.14)],
            ],
        );
        let mut buf = Vec::new();
        write_obqrs(&mut buf, &result).unwrap();

        let mut cursor = std::io::Cursor::new(buf);
        let decoded = read_obqrs(&mut cursor).unwrap();
        assert_eq!(decoded.rows[0][0], ResultValue::Int(42));
        assert_eq!(decoded.rows[0][1], ResultValue::Float(3.14));
    }

    #[test]
    fn invalid_magic_rejected() {
        let buf = b"XXXX\x01\x00";
        let mut cursor = std::io::Cursor::new(buf.as_slice());
        let err = read_obqrs(&mut cursor).unwrap_err();
        assert!(err.to_string().contains("magic"));
    }
}
