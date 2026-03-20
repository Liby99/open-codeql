pub mod lexer;

use lalrpop_util::lalrpop_mod;

lalrpop_mod!(
    #[allow(clippy::all)]
    #[allow(unused)]
    pub ql
);

use lexer::{Lexer, LexicalError, Token};
use ocql_ql_ast::module::SourceFile;
use ocql_ql_ast::expr::Expr;

/// Error type for parsing.
pub type ParseError = lalrpop_util::ParseError<usize, Token, LexicalError>;

/// Preprocess input: convert turbofish `::<` to `<` so the parser
/// handles it with the existing `<` type-arg rules.
fn preprocess(input: &str) -> String {
    input.replace("::<", "<")
}

/// Parse a QL source file (query module or library module).
pub fn parse_source_file(input: &str) -> Result<SourceFile, ParseError> {
    let processed = preprocess(input);
    let lexer = Lexer::new(&processed);
    ql::SourceFileParser::new().parse(lexer)
}

/// Parse a single QL expression (for testing).
pub fn parse_expr(input: &str) -> Result<Expr, ParseError> {
    let processed = preprocess(input);
    let lexer = Lexer::new(&processed);
    ql::ExprRuleParser::new().parse(lexer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocql_ql_ast::*;
    use ocql_ql_ast::expr::ExprKind;


    #[test]
    fn test_parse_simple_select() {
        let input = "from int x where x = 42 select x";
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
        let file = result.unwrap();
        assert_eq!(file.members.len(), 1);
    }

    #[test]
    fn test_parse_select_no_from() {
        let input = "select 1 + 2";
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_predicate() {
        let input = r#"predicate isSmall(int i) { i = 1 or i = 2 }"#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_predicate_with_result() {
        let input = r#"int getSuccessor(int i) { result = i + 1 and i = 1 }"#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_class() {
        let input = r#"
            class SmallInt extends int {
                SmallInt() { this = 1 or this = 2 }
                int doubled() { result = this + this }
            }
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_import() {
        let input = "import My.Library.Module";
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_import_with_alias() {
        let input = "import My.Library as Lib";
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_expr_arithmetic() {
        let input = "1 + 2 * 3";
        let result = parse_expr(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
        let expr = result.unwrap();
        // Should be Add(1, Mul(2, 3)) due to precedence
        match &expr.kind {
            ExprKind::BinaryOp { op: BinOp::Add, lhs, rhs } => {
                assert!(matches!(lhs.kind, ExprKind::Literal(Literal::Int(1))));
                assert!(matches!(rhs.kind, ExprKind::BinaryOp { op: BinOp::Mul, .. }));
            }
            other => panic!("Expected Add, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_expr_string_literal() {
        let input = r#""hello world""#;
        let result = parse_expr(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
        match &result.unwrap().kind {
            ExprKind::Literal(Literal::String(s)) => assert_eq!(s, "hello world"),
            other => panic!("Expected string, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_exists() {
        let input = r#"
            predicate hasSmall() {
                exists(int x | x = 1)
            }
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_negation() {
        let input = r#"
            predicate notSmall(int x) {
                not x = 1
            }
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_annotation() {
        let input = r#"
            private predicate helper(int x) { x = 1 }
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_module() {
        let input = r#"
            module MyUtils {
                predicate isPositive(int x) { x > 0 }
            }
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_this_and_result() {
        let input = r#"
            class Pos extends int {
                Pos() { this > 0 }
                int doubled() { result = this * 2 }
            }
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_range_expr() {
        let input = "[1 .. 10]";
        let result = parse_expr(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
        assert!(matches!(result.unwrap().kind, ExprKind::Range { .. }));
    }

    #[test]
    fn test_parse_dont_care() {
        let input = "_";
        let result = parse_expr(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
        assert!(matches!(result.unwrap().kind, ExprKind::DontCare));
    }

    #[test]
    fn test_parse_aggregation() {
        let input = r#"
            from int x
            where x = count(int y | y > 0 | y)
            select x
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_call_in_comparison() {
        // x = getSuccessor(y) — call expression on RHS of comparison
        let input = r#"
            predicate test(int x, int y) { x = getSuccessor(y) }
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_call_lhs_comparison() {
        // getSuccessor(x) = y — call expression on LHS of comparison
        let input = r#"
            predicate test(int x, int y) { getSuccessor(x) = y }
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_call_in_arithmetic() {
        // result = getSuccessor(x) + 1 — call in arithmetic expression
        let input = r#"
            int addTwo(int x) { result = getSuccessor(x) + 1 }
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_call_expr() {
        // Parse a bare function call as an expression
        let input = "getSuccessor(x)";
        let result = parse_expr(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
        match &result.unwrap().kind {
            ExprKind::Call { name, args, .. } => {
                assert_eq!(name.name, "getSuccessor");
                assert_eq!(args.len(), 1);
            }
            other => panic!("Expected Call, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_paren_formula_grouping() {
        // (A or B) and C — paren formula for grouping
        let input = r#"
            predicate test(int x) { (x = 1 or x = 2) and x > 0 }
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_paren_expr() {
        // (1 + 2) * 3 — paren expression for arithmetic grouping
        let input = "(1 + 2) * 3";
        let result = parse_expr(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
        // Should be Mul(Paren(Add(1, 2)), 3)
        match &result.unwrap().kind {
            ExprKind::BinaryOp { op: BinOp::Mul, lhs, .. } => {
                assert!(matches!(lhs.kind, ExprKind::Paren(_)));
            }
            other => panic!("Expected Mul with Paren, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_bare_predicate_call_in_formula() {
        // isSmall(x) used as a bare predicate call in formula context
        let input = r#"
            predicate test(int x) { isSmall(x) and x > 0 }
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_class_with_db_type() {
        let input = r#"
            class Type extends Locatable, @type {
                Type() { this = this }
                string getName() { result = "foo" or result = "bar" }
            }
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_newtype_with_or() {
        let input = r#"
            newtype TBound =
                TBoundZero()
                or
                TBoundSsa(int x) { x = 1 }
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_overlay_annotation() {
        let input = r#"
            overlay[local]
            predicate test() { 1 = 1 }
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_module_alias() {
        let input = r#"
            module Foo = Bar;
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_class_without_extends() {
        let input = r#"
            class Foo {
                predicate test() { 1 = 1 }
            }
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_exists_three_part() {
        let input = r#"
            predicate test() {
                exists(int x | x > 0 | x < 10)
            }
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_multiple_members() {
        let input = r#"
            import My.Lib

            predicate isSmall(int i) { i = 1 }

            class SmallInt extends int {
                SmallInt() { this = 1 }
            }

            from SmallInt x
            select x
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
        let file = result.unwrap();
        assert_eq!(file.members.len(), 4);
    }

    #[test]
    fn test_parse_closure_call() {
        // Non-member closure call: pred+(x, y)
        let input = r#"
            predicate test(int x, int y) { pred+(x, y) }
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_member_closure_call() {
        // Member closure call: x.getSuccessor+(y)
        let input = r#"
            predicate test(int x, int y) { x.getSuccessor+(y) }
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_closure_star_call() {
        // Reflexive-transitive closure: pred*(x, y)
        let input = r#"
            predicate test(int x, int y) { pred*(x, y) }
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_closure_in_expr() {
        // Closure call in expression context: result = pred+(x)
        let input = r#"
            int test(int x) { result = getNext+(x) }
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_turbofish_module_alias() {
        // Turbofish syntax: module Foo = Bar::<Config>;
        let input = r#"
            module Foo = Bar::<Config>;
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_turbofish_qualified() {
        // Turbofish in qualified context: Module::<T>::pred(x)
        let input = r#"
            predicate test(int x) { Module::<Config>::pred(x) }
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }

    #[test]
    fn test_parse_turbofish_implements() {
        // Turbofish in implements clause
        let input = r#"
            module Impl implements Iface::<Config> {
                predicate test() { 1 = 1 }
            }
        "#;
        let result = parse_source_file(input);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
    }
}
