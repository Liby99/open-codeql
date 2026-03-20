mod def;
mod diagnostics;
mod namespace;
mod resolve;
mod types;

pub use def::{DefId, DefKind, FileId, LocalDefId};
pub use diagnostics::{Diagnostic, DiagnosticNote, Severity};
pub use namespace::ModuleNamespaces;
pub use resolve::ResolvedRef;
pub use types::Type;

use ocql_ql_ast::module::SourceFile;

/// Per-file analysis results layered on top of the AST.
pub struct FileAnalysis {
    /// The parsed AST (owned).
    pub ast: SourceFile,

    /// The original source text.
    pub source: String,

    /// The file path.
    pub path: String,

    /// DefIds assigned to declarations in this file.
    pub defs: Vec<DefInfo>,

    /// Name resolution: maps reference spans → resolved target.
    pub name_resolution: Vec<(ocql_common::Span, ResolvedRef)>,

    /// Type of each expression, keyed by span.
    pub expr_types: Vec<(ocql_common::Span, Type)>,

    /// Diagnostics produced during analysis.
    pub diagnostics: Vec<Diagnostic>,
}

/// A definition registered during declaration collection.
pub struct DefInfo {
    pub id: DefId,
    pub kind: DefKind,
    pub name: String,
    pub span: ocql_common::Span,
}

/// The central result of HIR analysis.
pub struct HirDatabase {
    pub files: Vec<FileAnalysis>,
    pub diagnostics: Vec<Diagnostic>,
}

impl HirDatabase {
    /// Iterate all error diagnostics across all files and global diagnostics.
    pub fn all_errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .chain(self.files.iter().flat_map(|f| f.diagnostics.iter()))
            .filter(|d| d.severity == Severity::Error)
    }

    /// Returns true if there are no errors.
    pub fn is_ok(&self) -> bool {
        self.all_errors().next().is_none()
    }
}

/// Analyze a single QL source file (Milestone 1 entry point).
///
/// Parses the source, resolves names, and type-checks expressions.
/// Does not handle imports or multi-file projects yet.
pub fn analyze_single_file(source: &str, path: &str) -> HirDatabase {
    // Phase 1: Parse
    let ast = match ocql_ql_parser::parse_source_file(source) {
        Ok(ast) => ast,
        Err(err) => {
            let diag = Diagnostic {
                severity: Severity::Error,
                message: format!("Parse error: {err:?}"),
                span: ocql_common::Span::dummy(),
                file: FileId(0),
                notes: vec![],
            };
            return HirDatabase {
                files: vec![],
                diagnostics: vec![diag],
            };
        }
    };

    let file_id = FileId(0);

    // Phase 3: Declaration collection
    let mut collector = resolve::DeclarationCollector::new(file_id);
    collector.collect_source_file(&ast);

    // Phase 4: Name resolution + Phase 5: Type checking (combined walk)
    let mut resolver = resolve::NameResolver::new(file_id, &collector);
    resolver.resolve_source_file(&ast);
    let name_resolution = std::mem::take(&mut resolver.name_resolutions);
    let expr_types = std::mem::take(&mut resolver.expr_types);
    let diagnostics = std::mem::take(&mut resolver.diagnostics);
    drop(resolver);

    let analysis = FileAnalysis {
        ast,
        source: source.to_string(),
        path: path.to_string(),
        defs: collector.into_defs(),
        name_resolution,
        expr_types,
        diagnostics,
    };

    HirDatabase {
        files: vec![analysis],
        diagnostics: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocql_ql_ast::ty::PrimitiveType;

    /// Helper: analyze source and assert no errors.
    fn analyze_ok(source: &str) -> HirDatabase {
        let db = analyze_single_file(source, "test.ql");
        let errors: Vec<_> = db.all_errors().collect();
        assert!(errors.is_empty(), "unexpected errors: {errors:#?}");
        db
    }

    /// Helper: analyze source and return all error messages.
    fn analyze_errors(source: &str) -> Vec<String> {
        let db = analyze_single_file(source, "test.ql");
        db.all_errors().map(|d| d.message.clone()).collect()
    }

    #[test]
    fn simple_select() {
        let db = analyze_ok(
            "from int x\nwhere x > 0\nselect x",
        );
        let file = &db.files[0];
        // Should have expression types for x, 0, and the select expr
        assert!(!file.expr_types.is_empty());
        // x should be typed as int
        let x_types: Vec<_> = file
            .expr_types
            .iter()
            .filter(|(_, ty)| *ty == Type::Primitive(PrimitiveType::Int))
            .collect();
        assert!(!x_types.is_empty(), "expected int-typed expressions");
    }

    #[test]
    fn predicate_call_in_where() {
        analyze_ok(
            r#"
            predicate isSmall(int x) { x >= 0 and x < 10 }
            from int x
            where isSmall(x)
            select x
            "#,
        );
    }

    #[test]
    fn predicate_with_result() {
        analyze_ok(
            r#"
            int doubleIt(int x) { result = x + x }
            from int x
            where x = doubleIt(3)
            select x
            "#,
        );
    }

    #[test]
    fn undefined_variable_error() {
        let errors = analyze_errors("from int x\nwhere y > 0\nselect x");
        assert!(
            errors.iter().any(|e| e.contains("undefined variable `y`")),
            "expected 'undefined variable' error, got: {errors:?}"
        );
    }

    #[test]
    fn undefined_predicate_error() {
        let errors = analyze_errors(
            "from int x\nwhere notDefined(x)\nselect x",
        );
        assert!(
            errors.iter().any(|e| e.contains("undefined predicate `notDefined`")),
            "expected 'undefined predicate' error, got: {errors:?}"
        );
    }

    #[test]
    fn type_mismatch_comparison() {
        let errors = analyze_errors(
            r#"from int x, string s where x = s select x"#,
        );
        assert!(
            errors.iter().any(|e| e.contains("cannot compare")),
            "expected type mismatch error, got: {errors:?}"
        );
    }

    #[test]
    fn arithmetic_type_checking() {
        let db = analyze_ok("from int x\nwhere x = 1 + 2\nselect x");
        let file = &db.files[0];
        // The addition `1 + 2` should type as int
        let int_exprs: Vec<_> = file
            .expr_types
            .iter()
            .filter(|(_, ty)| *ty == Type::Primitive(PrimitiveType::Int))
            .collect();
        assert!(int_exprs.len() >= 3, "expected at least 3 int expressions (1, 2, 1+2)");
    }

    #[test]
    fn string_concat() {
        let db = analyze_ok(
            r#"from string s where s = "hello" + " world" select s"#,
        );
        let file = &db.files[0];
        let string_exprs: Vec<_> = file
            .expr_types
            .iter()
            .filter(|(_, ty)| *ty == Type::Primitive(PrimitiveType::String))
            .collect();
        assert!(
            string_exprs.len() >= 3,
            "expected string-typed expressions for literals and concat"
        );
    }

    #[test]
    fn exists_quantifier() {
        analyze_ok(
            r#"
            predicate isSmall(int x) { x >= 0 and x < 10 }
            from int x
            where exists(int y | isSmall(y) and x = y)
            select x
            "#,
        );
    }

    #[test]
    fn quantifier_scoping() {
        // y should not be visible outside the exists
        let errors = analyze_errors(
            r#"
            from int x
            where exists(int y | y > 0) and y < 10
            select x
            "#,
        );
        assert!(
            errors.iter().any(|e| e.contains("undefined variable `y`")),
            "expected scoping error for y, got: {errors:?}"
        );
    }

    #[test]
    fn aggregation_count() {
        let db = analyze_ok(
            r#"
            from int n
            where n = count(int x | x = [1 .. 10] | x)
            select n
            "#,
        );
        let file = &db.files[0];
        // count result should be int
        let has_int = file
            .expr_types
            .iter()
            .any(|(_, ty)| *ty == Type::Primitive(PrimitiveType::Int));
        assert!(has_int);
    }

    #[test]
    fn class_basic() {
        analyze_ok(
            r#"
            class SmallInt extends int {
                SmallInt() { this >= 0 and this < 100 }
            }
            from SmallInt x
            select x
            "#,
        );
    }

    #[test]
    fn result_outside_predicate() {
        let errors = analyze_errors("from int x\nwhere x = result\nselect x");
        assert!(
            errors.iter().any(|e| e.contains("`result` used outside")),
            "expected result-outside-predicate error, got: {errors:?}"
        );
    }

    #[test]
    fn this_outside_class() {
        let errors = analyze_errors(
            "predicate foo() { this = 1 }",
        );
        assert!(
            errors.iter().any(|e| e.contains("`this` used outside")),
            "expected this-outside-class error, got: {errors:?}"
        );
    }

    #[test]
    fn multiple_predicates_different_arity() {
        analyze_ok(
            r#"
            predicate p(int x) { x > 0 }
            predicate p(int x, int y) { x > y }
            from int a, int b
            where p(a) and p(a, b)
            select a, b
            "#,
        );
    }

    #[test]
    fn range_expression() {
        analyze_ok(
            "from int x\nwhere x = [1 .. 10]\nselect x",
        );
    }

    #[test]
    fn float_arithmetic() {
        let db = analyze_ok(
            "from float x\nwhere x = 1.0 + 2.5\nselect x",
        );
        let file = &db.files[0];
        let float_exprs: Vec<_> = file
            .expr_types
            .iter()
            .filter(|(_, ty)| *ty == Type::Primitive(PrimitiveType::Float))
            .collect();
        assert!(float_exprs.len() >= 3);
    }

    #[test]
    fn mixed_int_float() {
        let db = analyze_ok(
            "from float x\nwhere x = 1 + 2.5\nselect x",
        );
        let file = &db.files[0];
        // 1 + 2.5 should be float
        let float_add = file
            .expr_types
            .iter()
            .any(|(_, ty)| *ty == Type::Primitive(PrimitiveType::Float));
        assert!(float_add, "expected float result from int + float");
    }
}
