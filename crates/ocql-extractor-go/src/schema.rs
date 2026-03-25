use ocql_schema::{parse_dbscheme, DbScheme};

/// A .dbscheme for Go extraction, modeled after go.dbscheme.
///
/// This is a growing subset of the full CodeQL Go schema.
pub fn go_schema() -> DbScheme {
    parse_dbscheme(GO_DBSCHEME).expect("built-in Go schema should parse")
}

const GO_DBSCHEME: &str = r#"
/* ========== Files and locations ========== */

files(
    unique int id: @file,
    string name: string ref
);

folders(
    unique int id: @folder,
    string name: string ref
);

@container = @file | @folder

locations_default(
    unique int id: @location_default,
    int file: @file ref,
    int beginLine: int ref,
    int beginColumn: int ref,
    int endLine: int ref,
    int endColumn: int ref
);

has_location(
    unique int locatable: @locatable ref,
    int location: @location_default ref
);

/* ========== Packages ========== */

packages(
    unique int id: @package,
    string name: string ref,
    string path: string ref
);

/* ========== Expressions ========== */

#keyset[parent, idx]
exprs(
    unique int id: @expr,
    int kind: int ref,
    int parent: @exprparent ref,
    int idx: int ref
);

literals(
    unique int expr: @expr ref,
    string value: string ref,
    string raw: string ref
);

/* ========== Statements ========== */

#keyset[parent, idx]
stmts(
    unique int id: @stmt,
    int kind: int ref,
    int parent: @stmtparent ref,
    int idx: int ref
);

/* ========== Declarations ========== */

#keyset[parent, idx]
decls(
    unique int id: @decl,
    int kind: int ref,
    int parent: @declparent ref,
    int idx: int ref
);

/* ========== Specs ========== */

#keyset[parent, idx]
specs(
    unique int id: @spec,
    int kind: int ref,
    int parent: @gendecl ref,
    int idx: int ref
);

/* ========== Fields ========== */

fields(
    unique int id: @field,
    int parent: @fieldparent ref,
    int idx: int ref
);

/* ========== Objects ========== */

objects(
    unique int id: @object,
    int kind: int ref,
    string name: string ref
);

/* ========== Scopes ========== */

scopes(
    unique int id: @scope,
    int kind: int ref
);

/* ========== Comments ========== */

#keyset[parent, idx]
comment_groups(
    unique int id: @comment_group,
    int parent: @file ref,
    int idx: int ref
);

comments(
    unique int id: @comment,
    int kind: int ref,
    int parent: @comment_group ref,
    int idx: int ref,
    string text: string ref
);

/* ========== Union types ========== */

@location = @location_default

@gendecl = @importdecl | @constdecl | @typedecl | @vardecl

@funcdef = @funclit | @funcdecl

@exprparent = @funcdef | @file | @expr | @field | @stmt | @decl | @spec

@fieldparent = @decl | @expr

@stmtparent = @funcdef | @stmt | @decl

@declparent = @file | @stmt

@locatable = @file | @expr | @stmt | @decl | @spec | @field | @object
           | @scope | @comment_group | @comment | @package

@scopenode = @file | @expr | @stmt

@node = @exprparent | @stmtparent | @declparent | @fieldparent
      | @scopenode | @comment_group | @comment

@element = @file | @folder | @expr | @stmt | @decl | @spec | @field
         | @object | @scope | @comment_group | @comment | @package
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_go_schema_parses() {
        let schema = go_schema();
        let table_count = schema.tables().count();
        eprintln!("Go schema has {} tables", table_count);
        assert!(table_count >= 10, "Should have >= 10 tables, got {}", table_count);
    }
}
