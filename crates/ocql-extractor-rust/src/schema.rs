use ocql_schema::{parse_dbscheme, DbScheme};

/// A .dbscheme for Rust extraction.
///
/// This is a growing subset covering core Rust language constructs.
pub fn rust_schema() -> DbScheme {
    parse_dbscheme(RUST_DBSCHEME).expect("built-in Rust schema should parse")
}

const RUST_DBSCHEME: &str = r#"
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

rs_modules(
    unique int id: @rs_module,
    string name: string ref,
    int parent: @rs_parent ref
);

/* ========== Functions ========== */

rs_functions(
    unique int id: @rs_function,
    string name: string ref,
    int parent: @rs_parent ref
);

/* ========== Structs ========== */

rs_structs(
    unique int id: @rs_struct,
    string name: string ref,
    int parent: @rs_parent ref
);

/* ========== Enums ========== */

rs_enums(
    unique int id: @rs_enum,
    string name: string ref,
    int parent: @rs_parent ref
);

rs_enum_variants(
    unique int id: @rs_enum_variant,
    string name: string ref,
    int parent_enum: @rs_enum ref
);

/* ========== Traits ========== */

rs_traits(
    unique int id: @rs_trait,
    string name: string ref,
    int parent: @rs_parent ref
);

/* ========== Impls ========== */

rs_impls(
    unique int id: @rs_impl,
    string type_name: string ref,
    string trait_name: string ref,
    int parent: @rs_parent ref
);

/* ========== Type aliases ========== */

rs_type_aliases(
    unique int id: @rs_type_alias,
    string name: string ref,
    int parent: @rs_parent ref
);

/* ========== Consts and statics ========== */

rs_consts(
    unique int id: @rs_const,
    string name: string ref,
    string type_name: string ref,
    int parent: @rs_parent ref
);

/* ========== Fields ========== */

rs_fields(
    unique int id: @rs_field,
    string name: string ref,
    string type_name: string ref,
    int parent: @rs_field_parent ref
);

/* ========== Parameters ========== */

#keyset[parent,pos]
rs_params(
    unique int id: @rs_param,
    string name: string ref,
    string type_name: string ref,
    int pos: int ref,
    int parent: @rs_callable ref
);

/* ========== Local variables ========== */

rs_local_vars(
    unique int id: @rs_local_var,
    string name: string ref,
    string type_name: string ref,
    int parent: @rs_parent ref
);

/* ========== Statements ========== */

#keyset[parent,idx]
rs_stmts(
    unique int id: @rs_stmt,
    int kind: int ref,
    int parent: @rs_parent ref,
    int idx: int ref
);

/* ========== Expressions ========== */

#keyset[parent,idx]
rs_exprs(
    unique int id: @rs_expr,
    int kind: int ref,
    int parent: @rs_parent ref,
    int idx: int ref
);

/* ========== Use declarations ========== */

rs_use_decls(
    unique int id: @rs_use_decl,
    string path: string ref,
    string alias: string ref,
    int parent: @rs_parent ref
);

/* ========== Macros ========== */

rs_macros(
    unique int id: @rs_macro,
    string name: string ref,
    int parent: @rs_parent ref
);

/* ========== Attributes ========== */

rs_attributes(
    unique int id: @rs_attribute,
    string name: string ref,
    int parent: @rs_parent ref
);

/* ========== Comments ========== */

rs_comments(
    unique int id: @rs_comment,
    string text: string ref,
    int location: @location_default ref
);

/* ========== Generics ========== */

#keyset[parent,pos]
rs_generics(
    unique int id: @rs_generic,
    string name: string ref,
    int pos: int ref,
    int parent: @rs_parent ref
);

rs_trait_bounds(
    unique int id: @rs_trait_bound,
    string bound_name: string ref,
    int parent_generic: @rs_generic ref
);

/* ========== Union types ========== */

@rs_callable = @rs_function

@rs_field_parent = @rs_struct | @rs_enum_variant

@rs_parent = @file | @rs_module | @rs_function | @rs_struct | @rs_enum
           | @rs_trait | @rs_impl | @rs_stmt | @rs_expr

@rs_element = @rs_module | @rs_function | @rs_struct | @rs_enum | @rs_enum_variant
            | @rs_trait | @rs_impl | @rs_type_alias | @rs_const | @rs_field
            | @rs_param | @rs_local_var | @rs_stmt | @rs_expr | @rs_use_decl
            | @rs_macro | @rs_attribute | @rs_comment | @rs_generic | @rs_trait_bound

@locatable = @rs_element | @file
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_schema_parses() {
        let schema = rust_schema();
        let table_count = schema.tables().count();
        eprintln!("Rust schema has {} tables", table_count);
        assert!(table_count >= 20, "Should have >= 20 tables, got {}", table_count);
    }
}
