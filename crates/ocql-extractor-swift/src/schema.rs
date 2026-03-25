use ocql_schema::{parse_dbscheme, DbScheme};

/// A .dbscheme for Swift extraction.
///
/// This is a growing subset of a full CodeQL Swift schema.
pub fn swift_schema() -> DbScheme {
    parse_dbscheme(SWIFT_DBSCHEME).expect("built-in Swift schema should parse")
}

const SWIFT_DBSCHEME: &str = r#"
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

/* ========== Modules ========== */

swift_modules(
    unique int id: @swift_module,
    string name: string ref
);

/* ========== Type declarations ========== */

swift_classes(
    unique int id: @swift_class,
    string name: string ref,
    int parent: @element ref
);

swift_structs(
    unique int id: @swift_struct,
    string name: string ref,
    int parent: @element ref
);

swift_enums(
    unique int id: @swift_enum,
    string name: string ref,
    int parent: @element ref
);

swift_enum_cases(
    unique int id: @swift_enum_case,
    string name: string ref,
    int parent_enum: @swift_enum ref
);

swift_protocols(
    unique int id: @swift_protocol,
    string name: string ref,
    int parent: @element ref
);

swift_extensions(
    unique int id: @swift_extension,
    string type_name: string ref,
    int parent: @element ref
);

/* ========== Functions and initializers ========== */

swift_functions(
    unique int id: @swift_function,
    string name: string ref,
    int parent: @element ref
);

swift_initializers(
    unique int id: @swift_initializer,
    int parent: @element ref
);

swift_subscripts(
    unique int id: @swift_subscript,
    int parent: @element ref
);

/* ========== Properties and variables ========== */

swift_properties(
    unique int id: @swift_property,
    string name: string ref,
    string type_name: string ref,
    int parent: @element ref
);

#keyset[parent,pos]
swift_params(
    unique int id: @swift_param,
    string name: string ref,
    string type_name: string ref,
    int pos: int ref,
    int parent: @element ref
);

swift_local_vars(
    unique int id: @swift_local_var,
    string name: string ref,
    string type_name: string ref,
    int parent: @element ref
);

/* ========== Statements ========== */

#keyset[parent,idx]
swift_stmts(
    unique int id: @swift_stmt,
    int kind: int ref,
    int parent: @element ref,
    int idx: int ref
);

/* ========== Expressions ========== */

#keyset[parent,idx]
swift_exprs(
    unique int id: @swift_expr,
    int kind: int ref,
    int parent: @element ref,
    int idx: int ref
);

/* ========== Imports ========== */

swift_imports(
    unique int id: @swift_import,
    string path: string ref,
    int kind: int ref
);

/* ========== Type inheritance ========== */

#keyset[type_id,idx]
swift_type_inheritance(
    int type_id: @element ref,
    string inherited_type_name: string ref,
    int idx: int ref
);

/* ========== Generics ========== */

#keyset[parent,pos]
swift_generics(
    unique int id: @swift_generic,
    string name: string ref,
    int pos: int ref,
    int parent: @element ref
);

swift_where_clauses(
    unique int id: @swift_where_clause,
    string requirement: string ref,
    int parent: @element ref
);

/* ========== Modifiers ========== */

swift_modifiers(
    unique int id: @swift_modifier,
    string name: string ref
);

swift_hasModifier(
    int element_id: @element ref,
    int modifier_id: @swift_modifier ref
);

/* ========== Attributes ========== */

swift_attributes(
    unique int id: @swift_attribute,
    string name: string ref,
    int parent: @element ref
);

/* ========== Comments ========== */

swift_comments(
    unique int id: @swift_comment,
    string text: string ref,
    int location: @location_default ref
);

/* ========== Union types ========== */

@swift_type_decl = @swift_class | @swift_struct | @swift_enum | @swift_protocol

@swift_callable = @swift_function | @swift_initializer | @swift_subscript

@swift_variable = @swift_property | @swift_param | @swift_local_var

@element = @swift_module | @swift_class | @swift_struct | @swift_enum | @swift_enum_case
         | @swift_protocol | @swift_extension | @swift_function | @swift_initializer
         | @swift_subscript | @swift_property | @swift_param | @swift_local_var
         | @swift_stmt | @swift_expr | @swift_import | @swift_generic
         | @swift_where_clause | @swift_modifier | @swift_attribute | @swift_comment
         | @file

@locatable = @element | @file
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swift_schema_parses() {
        let schema = swift_schema();
        let table_count = schema.tables().count();
        eprintln!("Swift schema has {} tables", table_count);
        assert!(table_count >= 20, "Should have >= 20 tables, got {}", table_count);
    }
}
