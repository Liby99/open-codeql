use ocql_schema::{parse_dbscheme, DbScheme};

/// A minimal .dbscheme for C/C++ extraction (Phase 3a).
///
/// This is a subset of the full semmlecode.cpp.dbscheme, containing only
/// the tables we populate in this MVP extractor. We'll expand it as we
/// add more extraction capabilities.
pub fn cpp_schema() -> DbScheme {
    parse_dbscheme(CPP_DBSCHEME).expect("built-in C++ schema should parse")
}

const CPP_DBSCHEME: &str = r#"
/* Files and locations */
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

/* Functions */
functions(
    unique int id: @function,
    string name: string ref,
    int kind: int ref
);

function_return_type(
    int id: @function ref,
    string return_type: string ref
);

/* Parameters */
#keyset[id, index]
params(
    int id: @function ref,
    int index: int ref,
    string name: string ref,
    string param_type: string ref
);

/* Variables (globals and locals) */
variables(
    unique int id: @variable,
    string name: string ref,
    string var_type: string ref,
    int kind: int ref
);

/* Types: structs, classes, unions, enums */
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

/* Base classes (inheritance) */
#keyset[derived, index]
derivations(
    int derived: @usertype ref,
    int index: int ref,
    string base_name: string ref
);

/* Locations for all entities */
element_location(
    int element: @element ref,
    int location: @location_default ref
);

@element = @function | @variable | @usertype

/* Preprocessor includes */
includes(
    unique int id: @include,
    int file: @file ref,
    string included: string ref
);
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpp_schema_parses() {
        let schema = cpp_schema();
        assert!(schema.tables().count() > 5);
    }
}
