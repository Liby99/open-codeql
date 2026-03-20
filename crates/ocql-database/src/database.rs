use std::collections::HashMap;

use ocql_schema::DbScheme;

use crate::relation::{ColumnDef, Relation, RelationSchema, Tuple};
use crate::string_interner::StringInterner;
use crate::value::{EntityId, Value};

/// A database is a collection of named relations plus metadata.
pub struct Database {
    pub schema: DbScheme,
    relations: HashMap<String, Relation>,
    pub strings: StringInterner,
    next_entity_id: u64,
}

impl Database {
    /// Create an empty database from a parsed .dbscheme.
    /// Creates one empty relation for each table in the schema.
    pub fn from_schema(schema: DbScheme) -> Self {
        let mut relations = HashMap::new();

        for table in schema.tables() {
            let rel_schema = RelationSchema {
                name: table.name.clone(),
                columns: table.columns.iter().map(|col| {
                    ColumnDef {
                        name: col.name.clone(),
                        col_type: col.col_type.clone(),
                    }
                }).collect(),
            };
            relations.insert(table.name.clone(), Relation::new(rel_schema));
        }

        Self {
            schema,
            relations,
            strings: StringInterner::new(),
            next_entity_id: 1,
        }
    }

    /// Create an empty database with no schema.
    pub fn empty() -> Self {
        Self {
            schema: DbScheme { entries: Vec::new() },
            relations: HashMap::new(),
            strings: StringInterner::new(),
            next_entity_id: 1,
        }
    }

    /// Allocate a fresh entity ID.
    pub fn alloc_entity(&mut self) -> EntityId {
        let id = EntityId(self.next_entity_id);
        self.next_entity_id += 1;
        id
    }

    /// Add a relation manually (for schema-less usage or computed relations).
    pub fn add_relation(&mut self, name: &str, schema: RelationSchema) {
        self.relations.insert(name.to_string(), Relation::new(schema));
    }

    /// Get a relation by name.
    pub fn relation(&self, name: &str) -> Option<&Relation> {
        self.relations.get(name)
    }

    /// Get a mutable relation by name.
    pub fn relation_mut(&mut self, name: &str) -> Option<&mut Relation> {
        self.relations.get_mut(name)
    }

    /// Insert a tuple into a named relation.
    pub fn insert(&mut self, table: &str, tuple: Tuple) -> Result<bool, DatabaseError> {
        match self.relations.get_mut(table) {
            Some(rel) => Ok(rel.insert(tuple)),
            None => Err(DatabaseError::UnknownTable(table.to_string())),
        }
    }

    /// Convenience: intern a string and return a Value::String.
    pub fn intern_string(&mut self, s: &str) -> Value {
        Value::String(self.strings.intern(s))
    }

    /// Iterate all tuples in a relation.
    pub fn scan(&self, table: &str) -> Option<impl Iterator<Item = &Tuple>> {
        self.relations.get(table).map(|r| r.scan())
    }

    /// List all relation names.
    pub fn relation_names(&self) -> impl Iterator<Item = &str> {
        self.relations.keys().map(|s| s.as_str())
    }

    /// Build a tuple from heterogeneous values using a helper.
    pub fn make_tuple(&mut self, values: &[TupleValue]) -> Tuple {
        values.iter().map(|v| match v {
            TupleValue::Int(i) => Value::Int(*i),
            TupleValue::Float(f) => Value::Float(ordered_float::OrderedFloat(*f)),
            TupleValue::Str(s) => {
                let interned = self.strings.intern(s);
                Value::String(interned)
            }
            TupleValue::Bool(b) => Value::Bool(*b),
            TupleValue::Entity(id) => Value::Entity(*id),
            TupleValue::Null => Value::Null,
        }).collect()
    }
}

/// Helper enum for building tuples with mixed types.
pub enum TupleValue<'a> {
    Int(i64),
    Float(f64),
    Str(&'a str),
    Bool(bool),
    Entity(EntityId),
    Null,
}

/// Database error type.
#[derive(Debug)]
pub enum DatabaseError {
    UnknownTable(String),
}

impl std::fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatabaseError::UnknownTable(name) => write!(f, "unknown table: {}", name),
        }
    }
}

impl std::error::Error for DatabaseError {}

#[cfg(test)]
mod tests {
    use super::*;
    use ocql_schema::parse_dbscheme;
    use smallvec::SmallVec;

    #[test]
    fn test_database_from_schema() {
        let input = r#"
            files(
                unique int id: @file,
                string name: string ref
            );

            folders(
                unique int id: @folder,
                string name: string ref
            );

            @container = @file | @folder
        "#;
        let schema = parse_dbscheme(input).unwrap();
        let db = Database::from_schema(schema);

        assert!(db.relation("files").is_some());
        assert!(db.relation("folders").is_some());
        assert!(db.relation("nonexistent").is_none());
        assert_eq!(db.relation("files").unwrap().len(), 0);
    }

    #[test]
    fn test_insert_and_scan() {
        let input = r#"
            files(
                unique int id: @file,
                string name: string ref
            );
        "#;
        let schema = parse_dbscheme(input).unwrap();
        let mut db = Database::from_schema(schema);

        let file_id = db.alloc_entity();
        let name_val = db.intern_string("main.cpp");
        let tuple: Tuple = SmallVec::from_vec(vec![Value::Entity(file_id), name_val]);
        db.insert("files", tuple).unwrap();

        assert_eq!(db.relation("files").unwrap().len(), 1);

        let tuples: Vec<_> = db.scan("files").unwrap().collect();
        assert_eq!(tuples.len(), 1);
        assert_eq!(tuples[0][0], Value::Entity(EntityId(1)));
    }

    #[test]
    fn test_make_tuple() {
        let mut db = Database::empty();
        let eid = db.alloc_entity();
        let tuple = db.make_tuple(&[
            TupleValue::Entity(eid),
            TupleValue::Str("hello"),
            TupleValue::Int(42),
        ]);
        assert_eq!(tuple.len(), 3);
        assert_eq!(tuple[0], Value::Entity(EntityId(1)));
        assert_eq!(tuple[2], Value::Int(42));
    }

    #[test]
    fn test_insert_unknown_table() {
        let mut db = Database::empty();
        let tuple: Tuple = SmallVec::from_vec(vec![Value::Int(1)]);
        let result = db.insert("nonexistent", tuple);
        assert!(result.is_err());
    }

    #[test]
    fn test_cpp_schema_database() {
        let content = std::fs::read_to_string(
            "../../vendor/codeql/cpp/ql/lib/semmlecode.cpp.dbscheme"
        );
        if let Ok(content) = content {
            let schema = parse_dbscheme(&content).unwrap();
            let mut db = Database::from_schema(schema);

            // Insert a file
            let file_id = db.alloc_entity();
            let name = db.intern_string("example.cpp");
            let tuple: Tuple = SmallVec::from_vec(vec![Value::Entity(file_id), name]);
            db.insert("files", tuple).unwrap();
            assert_eq!(db.relation("files").unwrap().len(), 1);

            // Insert a location
            let loc_id = db.alloc_entity();
            let loc_tuple: Tuple = SmallVec::from_vec(vec![
                Value::Entity(loc_id),
                Value::Entity(file_id),
                Value::Int(1),   // beginLine
                Value::Int(1),   // beginColumn
                Value::Int(10),  // endLine
                Value::Int(1),   // endColumn
            ]);
            db.insert("locations_default", loc_tuple).unwrap();
            assert_eq!(db.relation("locations_default").unwrap().len(), 1);

            // Verify we can scan
            let files: Vec<_> = db.scan("files").unwrap().collect();
            assert_eq!(files.len(), 1);
            let locs: Vec<_> = db.scan("locations_default").unwrap().collect();
            assert_eq!(locs.len(), 1);
        }
    }

    #[test]
    fn test_entity_id_allocation() {
        let mut db = Database::empty();
        let a = db.alloc_entity();
        let b = db.alloc_entity();
        let c = db.alloc_entity();
        assert_eq!(a, EntityId(1));
        assert_eq!(b, EntityId(2));
        assert_eq!(c, EntityId(3));
    }

    #[test]
    fn test_string_interning_through_db() {
        let mut db = Database::empty();
        let v1 = db.intern_string("hello");
        let v2 = db.intern_string("hello");
        let v3 = db.intern_string("world");
        assert_eq!(v1, v2); // same string → same value
        assert_ne!(v1, v3);
        assert_eq!(db.strings.len(), 2);
    }
}
