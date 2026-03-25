use ocql_schema::{parse_dbscheme, DbScheme};

/// A .dbscheme for C# extraction, modeled after semmlecode.csharp.dbscheme.
///
/// This is a growing subset of the full CodeQL C# schema.
pub fn csharp_schema() -> DbScheme {
    parse_dbscheme(CSHARP_DBSCHEME).expect("built-in C# schema should parse")
}

const CSHARP_DBSCHEME: &str = r#"
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

/* ========== Namespaces ========== */

namespaces(
    unique int id: @namespace,
    string name: string ref
);

namespace_declarations(
    unique int id: @namespace_declaration,
    int parent_namespace: @namespace ref
);

/* ========== Types ========== */

types(
    unique int id: @type,
    int kind: int ref,
    string name: string ref,
    int parent: @type_container ref
);

type_location(
    unique int id: @type ref,
    int location: @location_default ref
);

/* ========== Methods ========== */

methods(
    unique int id: @method,
    string name: string ref,
    string signature: string ref,
    string return_type: string ref,
    int parent_type: @type ref
);

/* ========== Constructors ========== */

constructors(
    unique int id: @constructor,
    string name: string ref,
    int parent_type: @type ref
);

@callable = @method | @constructor

/* ========== Properties ========== */

properties(
    unique int id: @property,
    string name: string ref,
    string type_name: string ref,
    int parent_type: @type ref
);

/* ========== Fields ========== */

fields(
    unique int id: @field,
    string name: string ref,
    string type_name: string ref,
    int parent_type: @type ref
);

/* ========== Events ========== */

events(
    unique int id: @event,
    string name: string ref,
    string type_name: string ref,
    int parent_type: @type ref
);

/* ========== Parameters ========== */

#keyset[parent_callable,pos]
params(
    unique int id: @param,
    string name: string ref,
    string type_name: string ref,
    int pos: int ref,
    int parent_callable: @callable ref
);

/* ========== Local variables ========== */

local_vars(
    unique int id: @local_var,
    string name: string ref,
    string type_name: string ref,
    int parent: @element ref
);

/* ========== Statements ========== */

#keyset[parent,idx]
stmts(
    unique int id: @stmt,
    int kind: int ref,
    int parent: @element ref,
    int idx: int ref
);

/* ========== Expressions ========== */

#keyset[parent,idx]
exprs(
    unique int id: @expr,
    int kind: int ref,
    string type_name: string ref,
    int parent: @element ref,
    int idx: int ref
);

/* ========== Modifiers ========== */

modifiers(
    unique int id: @modifier,
    string name: string ref
);

hasModifier(
    int element: @modifiable ref,
    int modifier: @modifier ref
);

/* ========== Attributes ========== */

attributes(
    unique int id: @attribute,
    int parent: @element ref,
    string type_name: string ref
);

/* ========== Using directives ========== */

using_directives(
    unique int id: @using_directive,
    string name: string ref,
    int kind: int ref
);

/* ========== Comments ========== */

comments(
    unique int id: @comment,
    string text: string ref,
    int location: @location_default ref
);

/* ========== Inheritance ========== */

implements(
    int type_id: @type ref,
    int interface_id: @type ref
);

extends(
    int type_id: @type ref,
    int base_type_id: @type ref
);

/* ========== Type parameters (generics) ========== */

#keyset[parent,pos]
type_parameters(
    unique int id: @type_parameter,
    string name: string ref,
    int pos: int ref,
    int parent: @element ref
);

/* ========== Union types ========== */

@type_container = @namespace | @type

@variable = @local_var | @field | @param

@modifiable = @type | @method | @constructor | @field | @param
            | @local_var | @property | @event

@element = @namespace | @namespace_declaration | @modifier | @attribute
         | @type | @method | @constructor | @field | @param | @local_var
         | @property | @event | @type_parameter
         | @stmt | @expr | @using_directive | @comment

@locatable = @element | @file
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csharp_schema_parses() {
        let schema = csharp_schema();
        let table_count = schema.tables().count();
        eprintln!("C# schema has {} tables", table_count);
        assert!(table_count >= 20, "Should have >= 20 tables, got {}", table_count);
    }
}
