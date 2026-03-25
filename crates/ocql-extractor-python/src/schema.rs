use ocql_schema::{parse_dbscheme, DbScheme};

/// A .dbscheme for Python extraction, modeled after semmlecode.python.dbscheme.
///
/// This is a growing subset of the full CodeQL Python schema.
pub fn python_schema() -> DbScheme {
    parse_dbscheme(PYTHON_DBSCHEME).expect("built-in Python schema should parse")
}

const PYTHON_DBSCHEME: &str = r#"
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

py_Modules(
    unique int id: @py_Module,
    int kind: int ref,
    string name: string ref,
    int file: @file ref
);

/* ========== Functions ========== */

py_Functions(
    unique int id: @py_Function,
    string name: string ref,
    int parent: @py_scope ref,
    int idx: int ref
);

py_function_is_async(
    unique int id: @py_Function ref
);

/* ========== Classes ========== */

py_Classes(
    unique int id: @py_Class,
    string name: string ref,
    int parent: @py_scope ref,
    int idx: int ref
);

py_base_classes(
    int classid: @py_Class ref,
    int idx: int ref,
    string name: string ref
);

/* ========== Statements ========== */

#keyset[parent,idx]
py_stmts(
    unique int id: @py_stmt,
    int kind: int ref,
    int parent: @py_scope ref,
    int idx: int ref
);

/* ========== Expressions ========== */

#keyset[parent,idx]
py_exprs(
    unique int id: @py_expr,
    int kind: int ref,
    int parent: @py_element ref,
    int idx: int ref
);

/* ========== Parameters ========== */

#keyset[parentid,idx]
py_parameters(
    unique int id: @py_parameter,
    string name: string ref,
    int idx: int ref,
    int parentid: @py_Function ref,
    int kind: int ref
);

py_parameter_defaults(
    unique int id: @py_parameter ref,
    string defaultText: string ref
);

py_parameter_annotations(
    unique int id: @py_parameter ref,
    string annotationText: string ref
);

/* ========== Variables ========== */

py_variables(
    unique int id: @py_variable,
    string name: string ref,
    int scope: @py_scope ref
);

/* ========== Imports ========== */

py_imports(
    unique int id: @py_import,
    int kind: int ref,
    string moduleName: string ref,
    int parent: @py_scope ref,
    int idx: int ref
);

py_import_names(
    int importid: @py_import ref,
    int idx: int ref,
    string name: string ref,
    string alias: string ref
);

/* ========== Decorators ========== */

py_decorators(
    unique int id: @py_decorator,
    int parentid: @py_element ref,
    int idx: int ref,
    string text: string ref
);

/* ========== Comments ========== */

py_comments(
    unique int id: @py_comment,
    string contents: string ref,
    int location: @location_default ref
);

/* ========== Scope hierarchy ========== */

py_scope_nesting(
    unique int inner: @py_scope ref,
    int outer: @py_scope ref
);

/* ========== Expression names (for name/identifier exprs) ========== */

py_expr_names(
    unique int id: @py_expr ref,
    string name: string ref
);

/* ========== Expression values (for literals) ========== */

py_expr_values(
    unique int id: @py_expr ref,
    string value: string ref
);

/* ========== Union types ========== */

@py_scope = @py_Module | @py_Function | @py_Class

@py_element = @py_Module | @py_Function | @py_Class
            | @py_stmt | @py_expr | @py_import | @py_parameter
            | @py_variable | @py_decorator | @py_comment

@locatable = @py_element | @file
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_schema_parses() {
        let schema = python_schema();
        let table_count = schema.tables().count();
        eprintln!("Python schema has {} tables", table_count);
        assert!(table_count >= 15, "Should have >= 15 tables, got {}", table_count);
    }
}
