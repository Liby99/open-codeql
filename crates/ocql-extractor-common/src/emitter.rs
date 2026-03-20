use ocql_database::{Database, EntityId, Value, Tuple};
use ocql_database::smallvec::SmallVec;

/// Emits facts (tuples) into a database.
///
/// Wraps a `Database` and provides convenient methods for inserting
/// typed tuples into named relations.
pub struct FactEmitter<'a> {
    pub db: &'a mut Database,
}

impl<'a> FactEmitter<'a> {
    pub fn new(db: &'a mut Database) -> Self {
        Self { db }
    }

    /// Allocate a fresh entity ID.
    pub fn alloc(&mut self) -> EntityId {
        self.db.alloc_entity()
    }

    /// Intern a string and return it as a Value.
    pub fn string(&mut self, s: &str) -> Value {
        self.db.intern_string(s)
    }

    /// Insert a tuple of values into a named table.
    /// Panics if the table doesn't exist (schema error in extractor).
    pub fn emit(&mut self, table: &str, values: Vec<Value>) {
        let tuple: Tuple = SmallVec::from_vec(values);
        self.db.insert(table, tuple)
            .unwrap_or_else(|e| panic!("Failed to insert into '{}': {}", table, e));
    }

    /// Emit a file registration: files(id, name).
    pub fn emit_file(&mut self, file_id: EntityId, path: &str) {
        let name = self.string(path);
        self.emit("files", vec![Value::Entity(file_id), name]);
    }

    /// Emit a folder registration: folders(id, name).
    pub fn emit_folder(&mut self, folder_id: EntityId, path: &str) {
        let name = self.string(path);
        self.emit("folders", vec![Value::Entity(folder_id), name]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocql_schema::parse_dbscheme;

    fn make_db() -> Database {
        let schema = parse_dbscheme(r#"
            files(unique int id: @file, string name: string ref);
            folders(unique int id: @folder, string name: string ref);
        "#).unwrap();
        Database::from_schema(schema)
    }

    #[test]
    fn test_emit_file() {
        let mut db = make_db();
        let file_id;
        {
            let mut emitter = FactEmitter::new(&mut db);
            file_id = emitter.alloc();
            emitter.emit_file(file_id, "src/main.cpp");
        }
        assert_eq!(db.relation("files").unwrap().len(), 1);
        let tuples: Vec<_> = db.scan("files").unwrap().collect();
        assert_eq!(tuples[0][0], Value::Entity(file_id));
    }

    #[test]
    fn test_emit_generic() {
        let mut db = make_db();
        {
            let mut emitter = FactEmitter::new(&mut db);
            let id = emitter.alloc();
            let name = emitter.string("test");
            emitter.emit("files", vec![Value::Entity(id), name]);
        }
        assert_eq!(db.relation("files").unwrap().len(), 1);
    }
}
