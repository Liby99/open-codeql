use ocql_schema::{parse_dbscheme, DbScheme};

/// A .dbscheme for Ruby extraction, covering core Ruby syntax.
///
/// This is a growing subset designed for Ruby source analysis.
pub fn ruby_schema() -> DbScheme {
    parse_dbscheme(RUBY_DBSCHEME).expect("built-in Ruby schema should parse")
}

const RUBY_DBSCHEME: &str = r#"
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

rb_modules(
    unique int id: @rb_module,
    string name: string ref,
    int parent: @rb_scope ref
);

/* ========== Classes ========== */

rb_classes(
    unique int id: @rb_class,
    string name: string ref,
    int parent: @rb_scope ref,
    string superclass_name: string ref
);

/* ========== Methods ========== */

rb_methods(
    unique int id: @rb_method,
    string name: string ref,
    int parent: @rb_scope ref
);

rb_singleton_methods(
    unique int id: @rb_singleton_method,
    string name: string ref,
    int parent: @rb_scope ref
);

/* ========== Blocks ========== */

rb_blocks(
    unique int id: @rb_block,
    int parent: @rb_scope ref
);

/* ========== Parameters ========== */

#keyset[parent,pos]
rb_params(
    unique int id: @rb_param,
    string name: string ref,
    int kind: int ref,
    int pos: int ref,
    int parent: @rb_callable ref
);

/* ========== Statements ========== */

#keyset[parent,idx]
rb_stmts(
    unique int id: @rb_stmt,
    int kind: int ref,
    int parent: @rb_scope ref,
    int idx: int ref
);

/* ========== Expressions ========== */

#keyset[parent,idx]
rb_exprs(
    unique int id: @rb_expr,
    int kind: int ref,
    int parent: @rb_scope ref,
    int idx: int ref
);

/* ========== Calls ========== */

rb_calls(
    unique int id: @rb_call,
    string name: string ref,
    int receiver: @rb_expr ref,
    string method_name: string ref
);

/* ========== Constants ========== */

rb_constants(
    unique int id: @rb_constant,
    string name: string ref,
    int parent: @rb_scope ref
);

/* ========== Requires ========== */

rb_requires(
    unique int id: @rb_require,
    string path: string ref
);

/* ========== Comments ========== */

rb_comments(
    unique int id: @rb_comment,
    string text: string ref,
    int location: @location_default ref
);

/* ========== Local variables ========== */

rb_local_vars(
    unique int id: @rb_local_var,
    string name: string ref,
    int parent: @rb_scope ref
);

/* ========== Union types ========== */

@rb_callable = @rb_method | @rb_singleton_method | @rb_block

@rb_scope = @file | @rb_module | @rb_class | @rb_method | @rb_singleton_method
          | @rb_block | @rb_stmt | @rb_expr

@rb_element = @rb_module | @rb_class | @rb_method | @rb_singleton_method
            | @rb_block | @rb_param | @rb_stmt | @rb_expr | @rb_call
            | @rb_constant | @rb_require | @rb_comment | @rb_local_var

@locatable = @rb_element | @file
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ruby_schema_parses() {
        let schema = ruby_schema();
        let table_count = schema.tables().count();
        eprintln!("Ruby schema has {} tables", table_count);
        assert!(table_count >= 14, "Should have >= 14 tables, got {}", table_count);
    }
}
