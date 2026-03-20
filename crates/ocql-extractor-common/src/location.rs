use ocql_database::{EntityId, Value};

use crate::FactEmitter;

/// Helper for emitting source location facts.
///
/// Manages the `locations_default` table:
///   locations_default(id, file, beginLine, beginColumn, endLine, endColumn)
pub struct LocationEmitter;

impl LocationEmitter {
    /// Emit a source location span into `locations_default`.
    pub fn emit(
        emitter: &mut FactEmitter,
        loc_id: EntityId,
        file_id: EntityId,
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
    ) {
        emitter.emit("locations_default", vec![
            Value::Entity(loc_id),
            Value::Entity(file_id),
            Value::Int(start_line as i64),
            Value::Int(start_col as i64),
            Value::Int(end_line as i64),
            Value::Int(end_col as i64),
        ]);
    }

    /// Allocate a location ID and emit the location in one step.
    pub fn emit_for_node(
        emitter: &mut FactEmitter,
        file_id: EntityId,
        node: &tree_sitter::Node,
    ) -> EntityId {
        let loc_id = emitter.alloc();
        let start = node.start_position();
        let end = node.end_position();
        Self::emit(
            emitter,
            loc_id,
            file_id,
            (start.row + 1) as u32,   // tree-sitter is 0-based, dbscheme is 1-based
            (start.column + 1) as u32,
            (end.row + 1) as u32,
            end.column as u32,         // end column is exclusive in tree-sitter
        );
        loc_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocql_database::Database;
    use ocql_schema::parse_dbscheme;

    #[test]
    fn test_emit_location() {
        let schema = parse_dbscheme(r#"
            files(unique int id: @file, string name: string ref);
            locations_default(
                unique int id: @location_default,
                int file: @file ref,
                int beginLine: int ref,
                int beginColumn: int ref,
                int endLine: int ref,
                int endColumn: int ref
            );
        "#).unwrap();
        let mut db = Database::from_schema(schema);
        let file_id;
        let loc_id;
        {
            let mut emitter = FactEmitter::new(&mut db);
            file_id = emitter.alloc();
            emitter.emit_file(file_id, "test.cpp");
            loc_id = emitter.alloc();
            LocationEmitter::emit(&mut emitter, loc_id, file_id, 1, 1, 10, 20);
        }

        let locs: Vec<_> = db.scan("locations_default").unwrap().collect();
        assert_eq!(locs.len(), 1);
        assert_eq!(locs[0][0], Value::Entity(loc_id));
        assert_eq!(locs[0][1], Value::Entity(file_id));
        assert_eq!(locs[0][2], Value::Int(1));   // beginLine
        assert_eq!(locs[0][5], Value::Int(20));  // endColumn
    }
}
