use ocql_schema::{parse_dbscheme, DbScheme};

/// A .dbscheme for C/C++ extraction, modeled after semmlecode.cpp.dbscheme.
///
/// This is a growing subset of the full CodeQL C++ schema. Types are stored
/// as strings (not entity references) until we build a proper type system.
pub fn cpp_schema() -> DbScheme {
    parse_dbscheme(CPP_DBSCHEME).expect("built-in C++ schema should parse")
}

const CPP_DBSCHEME: &str = r#"
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

/* ========== Functions ========== */

functions(
    unique int id: @function,
    string name: string ref,
    int kind: int ref
);

function_return_type(
    int id: @function ref,
    string return_type: string ref
);

function_entry_point(
    int id: @function ref,
    unique int entry_point: @stmt ref
);

/* ========== Parameters ========== */

#keyset[id, index]
params(
    int id: @function ref,
    int index: int ref,
    string name: string ref,
    string param_type: string ref
);

/* ========== Variables ========== */

variables(
    unique int id: @variable,
    string name: string ref,
    string var_type: string ref,
    int kind: int ref
);

globalvariables(
    unique int id: @globalvariable,
    string name: string ref,
    string global_type: string ref
);

localvariables(
    unique int id: @localvariable,
    string name: string ref,
    string local_type: string ref
);

membervariables(
    unique int id: @membervariable,
    string name: string ref,
    string member_type: string ref
);

/* ========== Types ========== */

usertypes(
    unique int id: @usertype,
    string name: string ref,
    int kind: int ref
);

/* Fields (members of struct/class/union) */
#keyset[parent, index]
fields(
    int parent: @usertype ref,
    int index: int ref,
    string name: string ref,
    string field_type: string ref
);

/* Enum constants */
enumconstants(
    unique int id: @enumconstant,
    int parent: @usertype ref,
    int index: int ref,
    string name: string ref,
    int location: @location_default ref
);

/* Base classes (inheritance) */
#keyset[derived, index]
derivations(
    int derived: @usertype ref,
    int index: int ref,
    string base_name: string ref
);

/* Parent-child membership in types */
#keyset[parent, index]
member(
    int parent: @usertype ref,
    int index: int ref,
    int child: @element ref
);

/* ========== Namespaces ========== */

namespaces(
    unique int id: @namespace,
    string name: string ref
);

namespacembrs(
    int parentid: @namespace ref,
    unique int memberid: @element ref
);

/* ========== Statements ========== */

stmts(
    unique int id: @stmt,
    int kind: int ref,
    int location: @location_default ref
);

/* Parent of each statement (stmt or function) */
stmtparents(
    unique int id: @stmt ref,
    int index: int ref,
    int parent_id: @element ref
);

/* Control flow structure */
if_then(
    unique int if_stmt: @stmt ref,
    int then_id: @stmt ref
);

if_else(
    unique int if_stmt: @stmt ref,
    int else_id: @stmt ref
);

while_body(
    unique int while_stmt: @stmt ref,
    int body_id: @stmt ref
);

do_body(
    unique int do_stmt: @stmt ref,
    int body_id: @stmt ref
);

#keyset[for_stmt]
for_body(
    int for_stmt: @stmt ref,
    int body_id: @stmt ref
);

switch_body(
    unique int switch_stmt: @stmt ref,
    int body_id: @stmt ref
);

/* ========== Expressions ========== */

exprs(
    unique int id: @expr,
    int kind: int ref,
    int location: @location_default ref
);

exprparents(
    int expr_id: @expr ref,
    int child_index: int ref,
    int parent_id: @element ref
);

/* Literal value text */
valuetext(
    unique int id: @expr ref,
    string text: string ref
);

/* ========== Comments ========== */

comments(
    unique int id: @comment,
    string contents: string ref,
    int location: @location_default ref
);

/* ========== Preprocessor ========== */

includes(
    unique int id: @include,
    int file: @file ref,
    string included: string ref
);

/* ========== Locations for all entities ========== */

element_location(
    int element: @element ref,
    int location: @location_default ref
);

@element = @function | @variable | @globalvariable | @localvariable
         | @membervariable | @usertype | @enumconstant | @namespace
         | @stmt | @expr | @comment | @include

/* ========== Enclosing function ========== */

enclosingfunction(
    unique int child: @element ref,
    int parent: @function ref
);
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpp_schema_parses() {
        let schema = cpp_schema();
        let table_count = schema.tables().count();
        eprintln!("Schema has {} tables", table_count);
        assert!(table_count >= 25, "Should have >= 25 tables, got {}", table_count);
    }
}
