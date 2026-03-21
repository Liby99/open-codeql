//! Binary serialization for databases.
//!
//! Format (all integers little-endian):
//!
//! ```text
//! Header:
//!   magic:          [u8; 4]   = "OCQL"
//!   version:        u32       = 1
//!   next_entity_id: u64
//!
//! String table:
//!   num_strings:    u32
//!   for each string:
//!     len:          u32
//!     bytes:        [u8; len]
//!
//! Relations:
//!   num_relations:  u32
//!   for each relation:
//!     name_len:     u32
//!     name:         [u8; name_len]
//!     num_columns:  u32
//!     for each column:
//!       name_len:   u32
//!       name:       [u8; name_len]
//!       col_type:   u8          (0=Int, 1=Float, 2=String, 3=Boolean, 4=Date, 5=Varchar)
//!       varchar_n:  u32         (only if col_type == 5)
//!     num_tuples:   u32
//!     for each tuple:
//!       for each value:
//!         tag:      u8          (0=Int, 1=Float, 2=String, 3=Bool, 4=Entity, 5=Null)
//!         payload:  varies
//! ```

use std::io::{self, Read, Write};

use ocql_schema::DbScheme;
use smallvec::SmallVec;

use crate::database::Database;
use crate::relation::{ColumnDef, RelationSchema, Tuple};
use crate::string_interner::{InternedString, StringInterner};
use crate::value::{EntityId, Value};
use ocql_schema::ColumnType;

const MAGIC: &[u8; 4] = b"OCQL";
const VERSION: u32 = 1;

// Value tags
const TAG_INT: u8 = 0;
const TAG_FLOAT: u8 = 1;
const TAG_STRING: u8 = 2;
const TAG_BOOL: u8 = 3;
const TAG_ENTITY: u8 = 4;
const TAG_NULL: u8 = 5;

// ColumnType tags
const COL_INT: u8 = 0;
const COL_FLOAT: u8 = 1;
const COL_STRING: u8 = 2;
const COL_BOOLEAN: u8 = 3;
const COL_DATE: u8 = 4;
const COL_VARCHAR: u8 = 5;

/// Serialization error.
#[derive(Debug)]
pub enum SerializeError {
    Io(io::Error),
    InvalidMagic,
    UnsupportedVersion(u32),
    InvalidTag(u8),
    InvalidUtf8,
}

impl std::fmt::Display for SerializeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SerializeError::Io(e) => write!(f, "I/O error: {}", e),
            SerializeError::InvalidMagic => write!(f, "invalid database file (bad magic)"),
            SerializeError::UnsupportedVersion(v) => write!(f, "unsupported version: {}", v),
            SerializeError::InvalidTag(t) => write!(f, "invalid value tag: {}", t),
            SerializeError::InvalidUtf8 => write!(f, "invalid UTF-8 in database"),
        }
    }
}

impl std::error::Error for SerializeError {}

impl From<io::Error> for SerializeError {
    fn from(e: io::Error) -> Self {
        SerializeError::Io(e)
    }
}

// ============================================================
// Writing
// ============================================================

fn write_u8(w: &mut dyn Write, v: u8) -> io::Result<()> {
    w.write_all(&[v])
}

fn write_u32(w: &mut dyn Write, v: u32) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn write_u64(w: &mut dyn Write, v: u64) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn write_i64(w: &mut dyn Write, v: i64) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn write_f64(w: &mut dyn Write, v: f64) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn write_bytes(w: &mut dyn Write, b: &[u8]) -> io::Result<()> {
    write_u32(w, b.len() as u32)?;
    w.write_all(b)
}

fn write_str(w: &mut dyn Write, s: &str) -> io::Result<()> {
    write_bytes(w, s.as_bytes())
}

fn write_col_type(w: &mut dyn Write, ct: &ColumnType) -> io::Result<()> {
    match ct {
        ColumnType::Int => write_u8(w, COL_INT),
        ColumnType::Float => write_u8(w, COL_FLOAT),
        ColumnType::String => write_u8(w, COL_STRING),
        ColumnType::Boolean => write_u8(w, COL_BOOLEAN),
        ColumnType::Date => write_u8(w, COL_DATE),
        ColumnType::Varchar(n) => {
            write_u8(w, COL_VARCHAR)?;
            write_u32(w, *n)
        }
    }
}

fn write_value(w: &mut dyn Write, v: &Value) -> io::Result<()> {
    match v {
        Value::Int(i) => { write_u8(w, TAG_INT)?; write_i64(w, *i) }
        Value::Float(f) => { write_u8(w, TAG_FLOAT)?; write_f64(w, f.into_inner()) }
        Value::String(s) => { write_u8(w, TAG_STRING)?; write_u32(w, s.0) }
        Value::Bool(b) => { write_u8(w, TAG_BOOL)?; write_u8(w, *b as u8) }
        Value::Entity(e) => { write_u8(w, TAG_ENTITY)?; write_u64(w, e.0) }
        Value::Null => write_u8(w, TAG_NULL),
    }
}

/// Serialize a database to a writer.
pub fn save_database(db: &Database, w: &mut dyn Write) -> Result<(), SerializeError> {
    // Header
    w.write_all(MAGIC)?;
    write_u32(w, VERSION)?;
    write_u64(w, db.next_entity_id())?;

    // String table
    let num_strings = db.strings.len() as u32;
    write_u32(w, num_strings)?;
    for i in 0..num_strings {
        let s = db.strings.resolve(InternedString(i));
        write_str(w, s)?;
    }

    // Relations — only non-empty ones
    let names: Vec<&str> = db.relation_names()
        .filter(|n| db.relation(n).is_some_and(|r| !r.is_empty()))
        .collect();
    write_u32(w, names.len() as u32)?;

    for name in &names {
        let rel = db.relation(name).unwrap();
        write_str(w, name)?;

        // Schema
        write_u32(w, rel.schema.columns.len() as u32)?;
        for col in &rel.schema.columns {
            write_str(w, &col.name)?;
            write_col_type(w, &col.col_type)?;
        }

        // Tuples
        write_u32(w, rel.len() as u32)?;
        for tuple in rel.scan() {
            for value in tuple.iter() {
                write_value(w, value)?;
            }
        }
    }

    Ok(())
}

// ============================================================
// Reading
// ============================================================

fn read_u8(r: &mut dyn Read) -> io::Result<u8> {
    let mut buf = [0u8; 1];
    r.read_exact(&mut buf)?;
    Ok(buf[0])
}

fn read_u32(r: &mut dyn Read) -> io::Result<u32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u64(r: &mut dyn Read) -> io::Result<u64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

fn read_i64(r: &mut dyn Read) -> io::Result<i64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(i64::from_le_bytes(buf))
}

fn read_f64(r: &mut dyn Read) -> io::Result<f64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(f64::from_le_bytes(buf))
}

fn read_string(r: &mut dyn Read) -> Result<String, SerializeError> {
    let len = read_u32(r)? as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    String::from_utf8(buf).map_err(|_| SerializeError::InvalidUtf8)
}

fn read_col_type(r: &mut dyn Read) -> Result<ColumnType, SerializeError> {
    let tag = read_u8(r)?;
    match tag {
        COL_INT => Ok(ColumnType::Int),
        COL_FLOAT => Ok(ColumnType::Float),
        COL_STRING => Ok(ColumnType::String),
        COL_BOOLEAN => Ok(ColumnType::Boolean),
        COL_DATE => Ok(ColumnType::Date),
        COL_VARCHAR => {
            let n = read_u32(r)?;
            Ok(ColumnType::Varchar(n))
        }
        _ => Err(SerializeError::InvalidTag(tag)),
    }
}

fn read_value(r: &mut dyn Read) -> Result<Value, SerializeError> {
    let tag = read_u8(r)?;
    match tag {
        TAG_INT => Ok(Value::Int(read_i64(r)?)),
        TAG_FLOAT => Ok(Value::Float(ordered_float::OrderedFloat(read_f64(r)?))),
        TAG_STRING => Ok(Value::String(InternedString(read_u32(r)?))),
        TAG_BOOL => Ok(Value::Bool(read_u8(r)? != 0)),
        TAG_ENTITY => Ok(Value::Entity(EntityId(read_u64(r)?))),
        TAG_NULL => Ok(Value::Null),
        _ => Err(SerializeError::InvalidTag(tag)),
    }
}

/// Deserialize a database from a reader.
pub fn load_database(r: &mut dyn Read) -> Result<Database, SerializeError> {
    // Header
    let mut magic = [0u8; 4];
    r.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(SerializeError::InvalidMagic);
    }
    let version = read_u32(r)?;
    if version != VERSION {
        return Err(SerializeError::UnsupportedVersion(version));
    }
    let next_entity_id = read_u64(r)?;

    // String table
    let num_strings = read_u32(r)?;
    let mut strings = StringInterner::new();
    for _ in 0..num_strings {
        let s = read_string(r)?;
        strings.intern(&s);
    }

    // Relations
    let num_relations = read_u32(r)?;
    let mut db = Database::from_parts(
        DbScheme { entries: Vec::new() },
        strings,
        next_entity_id,
    );

    for _ in 0..num_relations {
        let name = read_string(r)?;

        let num_cols = read_u32(r)?;
        let mut columns = Vec::with_capacity(num_cols as usize);
        for _ in 0..num_cols {
            let col_name = read_string(r)?;
            let col_type = read_col_type(r)?;
            columns.push(ColumnDef { name: col_name, col_type });
        }

        let schema = RelationSchema { name: name.clone(), columns };
        db.add_relation(&name, schema);

        let num_tuples = read_u32(r)?;
        let num_cols = db.relation(&name).unwrap().schema.columns.len();
        for _ in 0..num_tuples {
            let mut tuple: Tuple = SmallVec::with_capacity(num_cols);
            for _ in 0..num_cols {
                tuple.push(read_value(r)?);
            }
            db.insert(&name, tuple).unwrap();
        }
    }

    Ok(db)
}

/// Save database to a file path.
pub fn save_to_file(db: &Database, path: &std::path::Path) -> Result<(), SerializeError> {
    let mut file = io::BufWriter::new(std::fs::File::create(path)?);
    save_database(db, &mut file)
}

/// Load database from a file path.
pub fn load_from_file(path: &std::path::Path) -> Result<Database, SerializeError> {
    let mut file = io::BufReader::new(std::fs::File::open(path)?);
    load_database(&mut file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use smallvec::smallvec;

    fn roundtrip(db: &Database) -> Database {
        let mut buf = Vec::new();
        save_database(db, &mut buf).unwrap();
        let mut cursor = io::Cursor::new(buf);
        load_database(&mut cursor).unwrap()
    }

    #[test]
    fn test_roundtrip_empty() {
        let db = Database::empty();
        let db2 = roundtrip(&db);
        assert_eq!(db2.strings.len(), 0);
        assert_eq!(db2.relation_names().count(), 0);
    }

    #[test]
    fn test_roundtrip_strings() {
        let mut db = Database::empty();
        db.strings.intern("hello");
        db.strings.intern("world");
        db.strings.intern("hello"); // duplicate

        let db2 = roundtrip(&db);
        assert_eq!(db2.strings.len(), 2);
        assert_eq!(db2.strings.resolve(InternedString(0)), "hello");
        assert_eq!(db2.strings.resolve(InternedString(1)), "world");
    }

    #[test]
    fn test_roundtrip_entity_id() {
        let mut db = Database::empty();
        db.alloc_entity();
        db.alloc_entity();
        db.alloc_entity();

        let db2 = roundtrip(&db);
        // Next entity should continue from where we left off
        assert_eq!(db2.next_entity_id(), 4);
    }

    #[test]
    fn test_roundtrip_relation_with_data() {
        let mut db = Database::empty();
        let name = db.intern_string("test_file.c");
        let schema = RelationSchema {
            name: "files".to_string(),
            columns: vec![
                ColumnDef { name: "id".to_string(), col_type: ColumnType::Int },
                ColumnDef { name: "name".to_string(), col_type: ColumnType::String },
            ],
        };
        db.add_relation("files", schema);
        db.insert("files", smallvec![Value::Entity(EntityId(1)), name]).unwrap();
        db.insert("files", smallvec![Value::Entity(EntityId(2)), Value::String(InternedString(0))]).unwrap();

        let db2 = roundtrip(&db);
        let rel = db2.relation("files").unwrap();
        assert_eq!(rel.len(), 2);

        let tuples: Vec<_> = rel.scan().collect();
        assert_eq!(tuples[0][0], Value::Entity(EntityId(1)));
        assert_eq!(tuples[1][0], Value::Entity(EntityId(2)));
        // String should resolve to same value
        assert_eq!(
            db2.strings.resolve(tuples[0][1].as_string().unwrap()),
            "test_file.c"
        );
    }

    #[test]
    fn test_roundtrip_all_value_types() {
        let mut db = Database::empty();
        db.strings.intern("test");
        let schema = RelationSchema {
            name: "values".to_string(),
            columns: vec![
                ColumnDef { name: "a".to_string(), col_type: ColumnType::Int },
                ColumnDef { name: "b".to_string(), col_type: ColumnType::Float },
                ColumnDef { name: "c".to_string(), col_type: ColumnType::String },
                ColumnDef { name: "d".to_string(), col_type: ColumnType::Boolean },
                ColumnDef { name: "e".to_string(), col_type: ColumnType::Int },
                ColumnDef { name: "f".to_string(), col_type: ColumnType::Int },
            ],
        };
        db.add_relation("values", schema);
        db.insert("values", smallvec![
            Value::Int(42),
            Value::Float(ordered_float::OrderedFloat(3.14)),
            Value::String(InternedString(0)),
            Value::Bool(true),
            Value::Entity(EntityId(99)),
            Value::Null,
        ]).unwrap();

        let db2 = roundtrip(&db);
        let tuples: Vec<_> = db2.scan("values").unwrap().collect();
        assert_eq!(tuples.len(), 1);
        assert_eq!(tuples[0][0], Value::Int(42));
        assert_eq!(tuples[0][1], Value::Float(ordered_float::OrderedFloat(3.14)));
        assert_eq!(tuples[0][2], Value::String(InternedString(0)));
        assert_eq!(tuples[0][3], Value::Bool(true));
        assert_eq!(tuples[0][4], Value::Entity(EntityId(99)));
        assert_eq!(tuples[0][5], Value::Null);
    }

    #[test]
    fn test_roundtrip_multiple_relations() {
        let mut db = Database::empty();
        for (name, cols) in [
            ("r1", vec!["a", "b"]),
            ("r2", vec!["x"]),
            ("r3", vec!["p", "q", "r"]),
        ] {
            let schema = RelationSchema {
                name: name.to_string(),
                columns: cols.iter().map(|c| ColumnDef {
                    name: c.to_string(),
                    col_type: ColumnType::Int,
                }).collect(),
            };
            db.add_relation(name, schema);
        }

        db.insert("r1", smallvec![Value::Int(1), Value::Int(2)]).unwrap();
        db.insert("r1", smallvec![Value::Int(3), Value::Int(4)]).unwrap();
        db.insert("r2", smallvec![Value::Int(10)]).unwrap();
        db.insert("r3", smallvec![Value::Int(100), Value::Int(200), Value::Int(300)]).unwrap();

        let db2 = roundtrip(&db);
        assert_eq!(db2.relation("r1").unwrap().len(), 2);
        assert_eq!(db2.relation("r2").unwrap().len(), 1);
        assert_eq!(db2.relation("r3").unwrap().len(), 1);
    }

    #[test]
    fn test_roundtrip_empty_relations_skipped() {
        let mut db = Database::empty();
        let schema = RelationSchema {
            name: "empty".to_string(),
            columns: vec![ColumnDef { name: "x".to_string(), col_type: ColumnType::Int }],
        };
        db.add_relation("empty", schema);

        let schema2 = RelationSchema {
            name: "notempty".to_string(),
            columns: vec![ColumnDef { name: "x".to_string(), col_type: ColumnType::Int }],
        };
        db.add_relation("notempty", schema2);
        db.insert("notempty", smallvec![Value::Int(1)]).unwrap();

        let db2 = roundtrip(&db);
        // Empty relation should not be serialized
        assert!(db2.relation("empty").is_none());
        assert_eq!(db2.relation("notempty").unwrap().len(), 1);
    }

    #[test]
    fn test_invalid_magic() {
        let buf = b"BADM\x01\x00\x00\x00";
        let mut cursor = io::Cursor::new(buf.as_ref());
        let result = load_database(&mut cursor);
        assert!(matches!(result, Err(SerializeError::InvalidMagic)));
    }

    #[test]
    fn test_unsupported_version() {
        let mut buf = Vec::new();
        buf.extend_from_slice(MAGIC);
        buf.extend_from_slice(&99u32.to_le_bytes()); // bad version
        let mut cursor = io::Cursor::new(buf);
        let result = load_database(&mut cursor);
        assert!(matches!(result, Err(SerializeError::UnsupportedVersion(99))));
    }

    #[test]
    fn test_file_roundtrip() {
        let mut db = Database::empty();
        db.strings.intern("hello");
        let schema = RelationSchema {
            name: "test".to_string(),
            columns: vec![
                ColumnDef { name: "val".to_string(), col_type: ColumnType::Int },
            ],
        };
        db.add_relation("test", schema);
        db.insert("test", smallvec![Value::Int(42)]).unwrap();

        let tmp = std::env::temp_dir().join("ocql_test_roundtrip.ocqldb");
        save_to_file(&db, &tmp).unwrap();
        let db2 = load_from_file(&tmp).unwrap();
        std::fs::remove_file(&tmp).ok();

        assert_eq!(db2.strings.len(), 1);
        assert_eq!(db2.relation("test").unwrap().len(), 1);
    }

    #[test]
    fn test_roundtrip_varchar_column_type() {
        let mut db = Database::empty();
        let schema = RelationSchema {
            name: "t".to_string(),
            columns: vec![
                ColumnDef { name: "s".to_string(), col_type: ColumnType::Varchar(255) },
            ],
        };
        db.add_relation("t", schema);
        db.insert("t", smallvec![Value::Int(1)]).unwrap();

        let db2 = roundtrip(&db);
        let rel = db2.relation("t").unwrap();
        assert_eq!(rel.schema.columns[0].col_type, ColumnType::Varchar(255));
    }

    #[test]
    fn test_large_database_roundtrip() {
        let mut db = Database::empty();
        // Intern 1000 strings
        for i in 0..1000 {
            db.strings.intern(&format!("string_{}", i));
        }
        let schema = RelationSchema {
            name: "big".to_string(),
            columns: vec![
                ColumnDef { name: "id".to_string(), col_type: ColumnType::Int },
                ColumnDef { name: "name".to_string(), col_type: ColumnType::String },
            ],
        };
        db.add_relation("big", schema);
        for i in 0..1000 {
            db.insert("big", smallvec![
                Value::Int(i),
                Value::String(InternedString(i as u32)),
            ]).unwrap();
        }

        let mut buf = Vec::new();
        save_database(&db, &mut buf).unwrap();

        let mut cursor = io::Cursor::new(&buf);
        let db2 = load_database(&mut cursor).unwrap();

        assert_eq!(db2.strings.len(), 1000);
        assert_eq!(db2.relation("big").unwrap().len(), 1000);

        // Spot check
        let tuples: Vec<_> = db2.scan("big").unwrap().collect();
        assert_eq!(
            db2.strings.resolve(tuples[42][1].as_string().unwrap()),
            "string_42"
        );
    }
}
