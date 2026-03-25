use ocql_schema::{parse_dbscheme, DbScheme};

/// A .dbscheme for JavaScript/TypeScript extraction, modeled after
/// semmlecode.javascript.dbscheme.
///
/// This is a growing subset of the full CodeQL JavaScript schema.
pub fn javascript_schema() -> DbScheme {
    parse_dbscheme(JS_DBSCHEME).expect("built-in JavaScript schema should parse")
}

const JS_DBSCHEME: &str = r#"
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

hasLocation(
    int locatableid: @locatable ref,
    int id: @location_default ref
);

/* ========== Top-levels ========== */

toplevels(
    unique int id: @toplevel,
    int kind: int ref
);

/* ========== Statements ========== */

#keyset[parent, idx]
stmts(
    unique int id: @stmt,
    int kind: int ref,
    int parent: @stmtparent ref,
    int idx: int ref
);

/* ========== Expressions ========== */

#keyset[parent, idx]
exprs(
    unique int id: @expr,
    int kind: int ref,
    int parent: @exprparent ref,
    int idx: int ref
);

/* ========== Literals ========== */

literals(
    string value: string ref,
    int parent: @expr ref
);

/* ========== Properties ========== */

#keyset[parent, idx]
properties(
    unique int id: @property,
    int parent: @propertyparent ref,
    int idx: int ref,
    int kind: int ref
);

/* ========== Variables ========== */

variables(
    unique int id: @variable,
    string name: string ref,
    int scope: @scope ref
);

/* ========== Scopes ========== */

scopes(
    unique int id: @scope,
    int kind: int ref
);

/* ========== Functions ========== */

functions(
    unique int id: @function,
    string name: string ref,
    int parent: @element ref
);

/* ========== Classes ========== */

classes(
    unique int id: @class,
    string name: string ref,
    int parent: @element ref
);

/* ========== Imports ========== */

imports(
    unique int id: @import,
    int parent: @element ref,
    string name: string ref,
    string path: string ref
);

/* ========== Exports ========== */

exports(
    unique int id: @export,
    int parent: @element ref
);

/* ========== Comments ========== */

comments(
    unique int id: @comment,
    int kind: int ref,
    int parent: @element ref,
    string text: string ref
);

/* ========== TypeScript extras ========== */

type_annotations(
    unique int id: @type_annotation,
    int parent: @element ref,
    string text: string ref
);

interfaces(
    unique int id: @interface,
    string name: string ref,
    int parent: @element ref
);

enums(
    unique int id: @enum,
    string name: string ref,
    int parent: @element ref
);

type_aliases(
    unique int id: @type_alias,
    string name: string ref,
    int parent: @element ref
);

decorators(
    unique int id: @decorator,
    string name: string ref,
    int parent: @element ref
);

/* ========== Union types ========== */

@stmtparent = @stmt | @toplevel | @function | @arrow_func

@exprparent = @expr | @stmt | @property | @toplevel | @function | @arrow_func

@propertyparent = @expr | @class | @interface

@element = @file | @toplevel | @stmt | @expr | @property | @function
         | @arrow_func | @class | @interface | @enum | @type_alias
         | @variable | @scope | @import | @export | @comment
         | @type_annotation | @decorator

@locatable = @element | @location_default

@arrow_func = @function
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_javascript_schema_parses() {
        let schema = javascript_schema();
        let table_count = schema.tables().count();
        eprintln!("JavaScript schema has {} tables", table_count);
        assert!(table_count >= 15, "Should have >= 15 tables, got {}", table_count);
    }
}
