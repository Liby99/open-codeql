//! Parser for Datalog rules in text format.
//!
//! Converts a text-based rule representation into `Program`/`Rule` structs
//! that the engine can evaluate.
//!
//! # Syntax
//!
//! ```text
//! // Line comments
//!
//! // A rule with body:
//! path(x, y) :- edge(x, y).
//!
//! // Transitive closure:
//! path(x, y) :- path(x, z), edge(z, y).
//!
//! // Negation:
//! unreachable(x) :- node(x), not path(1, x).
//!
//! // Guards (comparison filters):
//! big(x, y) :- edge(x, y), x > 10.
//! same(x, y) :- edge(x, y), x = y.
//!
//! // Constants: integers, strings (quoted)
//! call_expr(id) :- exprs(id, 74, _).
//! greeting(x) :- messages(x, "hello").
//!
//! // Wildcards: _ is expanded to unique anonymous variables
//! func_name(name) :- functions(_, name, _).
//!
//! // Facts (no body):
//! base(1, 2).
//! ```

use ocql_database::Value;
use crate::rule::{ArithExpr, ArithOp, Atom, BodyElement, CompOp, Guard, Program, Rule, Term};

/// Parse error with position information.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.col, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Parse a Datalog program from text.
pub fn parse_program(input: &str) -> Result<Program, ParseError> {
    let mut parser = Parser::new(input);
    parser.parse_program()
}

// ============================================================
// Tokenizer
// ============================================================

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),    // lowercase identifier or _-prefixed
    Int(i64),         // integer literal
    Str(String),      // string literal (without quotes)
    LParen,           // (
    RParen,           // )
    Comma,            // ,
    Dot,              // .
    ColonDash,        // :-
    Not,              // "not" keyword
    Eq,               // =
    Ne,               // !=
    Lt,               // <
    Le,               // <=
    Gt,               // >
    Ge,               // >=
    Plus,             // +
    Minus,            // - (when not part of negative int)
    Star,             // *
    Slash,            // /
    Percent,          // %
    Underscore,       // bare _ (wildcard)
    Eof,
}

#[derive(Debug, Clone)]
struct Span {
    line: usize,
    col: usize,
}

struct Lexer<'a> {
    input: &'a [u8],
    pos: usize,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    fn span(&self) -> Span {
        Span { line: self.line, col: self.col }
    }

    fn peek_byte(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) {
        if self.pos < self.input.len() {
            if self.input[self.pos] == b'\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
            self.pos += 1;
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            // Skip whitespace
            while self.pos < self.input.len() && self.input[self.pos].is_ascii_whitespace() {
                self.advance();
            }
            // Skip // comments
            if self.pos + 1 < self.input.len()
                && self.input[self.pos] == b'/'
                && self.input[self.pos + 1] == b'/'
            {
                while self.pos < self.input.len() && self.input[self.pos] != b'\n' {
                    self.advance();
                }
                continue;
            }
            // Note: Prolog-style % comments removed — conflicts with % modulo operator
            // Skip /* ... */ block comments
            if self.pos + 1 < self.input.len()
                && self.input[self.pos] == b'/'
                && self.input[self.pos + 1] == b'*'
            {
                self.advance(); // /
                self.advance(); // *
                while self.pos + 1 < self.input.len() {
                    if self.input[self.pos] == b'*' && self.input[self.pos + 1] == b'/' {
                        self.advance(); // *
                        self.advance(); // /
                        break;
                    }
                    self.advance();
                }
                continue;
            }
            break;
        }
    }

    fn next_token(&mut self) -> Result<(Token, Span), ParseError> {
        self.skip_whitespace_and_comments();
        let span = self.span();

        let Some(ch) = self.peek_byte() else {
            return Ok((Token::Eof, span));
        };

        match ch {
            b'(' => { self.advance(); Ok((Token::LParen, span)) }
            b')' => { self.advance(); Ok((Token::RParen, span)) }
            b',' => { self.advance(); Ok((Token::Comma, span)) }
            b'.' => { self.advance(); Ok((Token::Dot, span)) }
            b'=' => { self.advance(); Ok((Token::Eq, span)) }
            b'+' => { self.advance(); Ok((Token::Plus, span)) }
            b'*' => { self.advance(); Ok((Token::Star, span)) }
            b'/' => {
                // Check for comments first — handled in skip_whitespace_and_comments
                // If we get here, it's a division operator
                self.advance();
                Ok((Token::Slash, span))
            }
            b'%' => {
                // Check for comments first — handled in skip_whitespace_and_comments
                self.advance();
                Ok((Token::Percent, span))
            }
            b'!' => {
                self.advance();
                if self.peek_byte() == Some(b'=') {
                    self.advance();
                    Ok((Token::Ne, span))
                } else {
                    Err(ParseError {
                        message: "expected '=' after '!'".to_string(),
                        line: span.line,
                        col: span.col,
                    })
                }
            }
            b'<' => {
                self.advance();
                if self.peek_byte() == Some(b'=') {
                    self.advance();
                    Ok((Token::Le, span))
                } else {
                    Ok((Token::Lt, span))
                }
            }
            b'>' => {
                self.advance();
                if self.peek_byte() == Some(b'=') {
                    self.advance();
                    Ok((Token::Ge, span))
                } else {
                    Ok((Token::Gt, span))
                }
            }
            b':' => {
                self.advance();
                if self.peek_byte() == Some(b'-') {
                    self.advance();
                    Ok((Token::ColonDash, span))
                } else {
                    Err(ParseError {
                        message: "expected '-' after ':'".to_string(),
                        line: span.line,
                        col: span.col,
                    })
                }
            }
            b'"' => {
                self.advance(); // opening quote
                let start = self.pos;
                while self.pos < self.input.len() && self.input[self.pos] != b'"' {
                    if self.input[self.pos] == b'\\' && self.pos + 1 < self.input.len() {
                        self.advance(); // skip escape
                    }
                    self.advance();
                }
                let s = std::str::from_utf8(&self.input[start..self.pos])
                    .map_err(|_| ParseError {
                        message: "invalid UTF-8 in string literal".to_string(),
                        line: span.line,
                        col: span.col,
                    })?
                    .to_string();
                if self.peek_byte() == Some(b'"') {
                    self.advance(); // closing quote
                }
                Ok((Token::Str(s), span))
            }
            b'-' if self.pos + 1 < self.input.len() && self.input[self.pos + 1].is_ascii_digit() => {
                self.advance(); // -
                let start = self.pos;
                while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
                    self.advance();
                }
                let digits = std::str::from_utf8(&self.input[start..self.pos]).unwrap();
                let val: i64 = digits.parse().map_err(|_| ParseError {
                    message: format!("invalid integer: -{}", digits),
                    line: span.line,
                    col: span.col,
                })?;
                Ok((Token::Int(-val), span))
            }
            b'-' => {
                self.advance();
                Ok((Token::Minus, span))
            }
            b'0'..=b'9' => {
                let start = self.pos;
                while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
                    self.advance();
                }
                let digits = std::str::from_utf8(&self.input[start..self.pos]).unwrap();
                let val: i64 = digits.parse().map_err(|_| ParseError {
                    message: format!("invalid integer: {}", digits),
                    line: span.line,
                    col: span.col,
                })?;
                Ok((Token::Int(val), span))
            }
            b'_' => {
                // Could be bare _ (wildcard) or _prefixed variable
                let start = self.pos;
                self.advance();
                while self.pos < self.input.len()
                    && (self.input[self.pos].is_ascii_alphanumeric() || self.input[self.pos] == b'_')
                {
                    self.advance();
                }
                let name = std::str::from_utf8(&self.input[start..self.pos]).unwrap();
                if name == "_" {
                    Ok((Token::Underscore, span))
                } else {
                    Ok((Token::Ident(name.to_string()), span))
                }
            }
            b'a'..=b'z' | b'A'..=b'Z' => {
                let start = self.pos;
                while self.pos < self.input.len()
                    && (self.input[self.pos].is_ascii_alphanumeric() || self.input[self.pos] == b'_')
                {
                    self.advance();
                }
                let name = std::str::from_utf8(&self.input[start..self.pos]).unwrap();
                if name == "not" {
                    Ok((Token::Not, span))
                } else {
                    Ok((Token::Ident(name.to_string()), span))
                }
            }
            _ => Err(ParseError {
                message: format!("unexpected character: '{}'", ch as char),
                line: span.line,
                col: span.col,
            }),
        }
    }
}

// ============================================================
// Parser
// ============================================================

struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Token,
    current_span: Span,
    anon_counter: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        let mut lexer = Lexer::new(input);
        let (tok, span) = lexer.next_token().unwrap_or((Token::Eof, Span { line: 1, col: 1 }));
        Self {
            lexer,
            current: tok,
            current_span: span,
            anon_counter: 0,
        }
    }

    fn advance(&mut self) -> Result<(), ParseError> {
        let (tok, span) = self.lexer.next_token()?;
        self.current = tok;
        self.current_span = span;
        Ok(())
    }

    fn expect(&mut self, expected: &Token) -> Result<(), ParseError> {
        if &self.current == expected {
            self.advance()
        } else {
            Err(ParseError {
                message: format!("expected {:?}, found {:?}", expected, self.current),
                line: self.current_span.line,
                col: self.current_span.col,
            })
        }
    }

    fn fresh_anon(&mut self) -> String {
        let name = format!("_anon{}", self.anon_counter);
        self.anon_counter += 1;
        name
    }

    // ---- Program ----

    fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut rules = Vec::new();
        while self.current != Token::Eof {
            rules.push(self.parse_rule()?);
        }
        Ok(Program::new(rules))
    }

    // ---- Rule ----

    fn parse_rule(&mut self) -> Result<Rule, ParseError> {
        let head = self.parse_atom()?;

        if self.current == Token::Dot {
            // Fact (no body)
            self.advance()?;
            return Ok(Rule::new(head, vec![]));
        }

        self.expect(&Token::ColonDash)?;

        let mut body = vec![self.parse_body_element()?];
        while self.current == Token::Comma {
            self.advance()?;
            body.push(self.parse_body_element()?);
        }

        self.expect(&Token::Dot)?;
        Ok(Rule::new(head, body))
    }

    // ---- Body element ----
    //
    // Disambiguate:
    //   "not" ident "("  →  negated atom
    //   ident "("        →  positive atom
    //   term comp_op     →  guard

    fn parse_body_element(&mut self) -> Result<BodyElement, ParseError> {
        if self.current == Token::Not {
            self.advance()?;
            let atom = self.parse_atom()?;
            return Ok(BodyElement::Negated(atom));
        }

        // Check if this is an atom (ident followed by lparen) or a guard
        if let Token::Ident(ref name) = self.current {
            // Save state to decide
            let name = name.clone();
            let _span = self.current_span.clone();

            // Peek: is next token LParen?
            // We need to advance to check, but we'll handle both cases
            self.advance()?;

            if self.current == Token::LParen {
                // It's an atom: name(...)
                self.advance()?; // consume (
                let terms = self.parse_term_list()?;
                self.expect(&Token::RParen)?;
                return Ok(BodyElement::Positive(Atom::new(&name, terms)));
            }

            // It's a guard or assignment: name op ...
            // For `=`, we need to disambiguate:
            //   name = term arith_op term  →  Assign { result_var: name, expr: ... }
            //   name = term                →  Guard { left: name, op: Eq, right: term }
            //   name comp_op term          →  Guard { left: name, op, right: term }
            if self.current == Token::Eq {
                self.advance()?; // consume =
                let first_term = self.parse_term()?;
                // Check if followed by an arithmetic operator
                if let Some(arith_op) = self.try_arith_op() {
                    self.advance()?; // consume arith op
                    let second_term = self.parse_term()?;
                    return Ok(BodyElement::Assign {
                        result_var: name,
                        expr: ArithExpr { left: first_term, op: arith_op, right: second_term },
                    });
                }
                // Handle `z = x - 1` where lexer parsed `-1` as Int(-1):
                // If first_term is a variable and we see a negative integer,
                // treat as subtraction.
                if let Token::Int(neg) = &self.current {
                    if *neg < 0 {
                        if let Term::Var(_) = &first_term {
                            let abs_val = neg.checked_neg().unwrap_or(0);
                            let second_term = Term::Const(Value::Int(abs_val));
                            self.advance()?;
                            return Ok(BodyElement::Assign {
                                result_var: name,
                                expr: ArithExpr { left: first_term, op: ArithOp::Sub, right: second_term },
                            });
                        }
                    }
                }
                // Just equality guard: name = term
                return Ok(BodyElement::Guard(Guard {
                    left: Term::Var(name),
                    op: CompOp::Eq,
                    right: first_term,
                }));
            }

            let left = Term::Var(name);
            let op = self.parse_comp_op()?;
            let right = self.parse_term()?;
            return Ok(BodyElement::Guard(Guard { left, op, right }));
        }

        // Could be a guard starting with a constant: 42 > x
        if matches!(self.current, Token::Int(_) | Token::Str(_)) {
            let left = self.parse_term()?;
            let op = self.parse_comp_op()?;
            let right = self.parse_term()?;
            return Ok(BodyElement::Guard(Guard { left, op, right }));
        }

        Err(ParseError {
            message: format!("expected body element, found {:?}", self.current),
            line: self.current_span.line,
            col: self.current_span.col,
        })
    }

    // ---- Atom ----

    fn parse_atom(&mut self) -> Result<Atom, ParseError> {
        let name = match &self.current {
            Token::Ident(n) => n.clone(),
            other => {
                return Err(ParseError {
                    message: format!("expected predicate name, found {:?}", other),
                    line: self.current_span.line,
                    col: self.current_span.col,
                });
            }
        };
        self.advance()?;
        self.expect(&Token::LParen)?;
        let terms = self.parse_term_list()?;
        self.expect(&Token::RParen)?;
        Ok(Atom::new(&name, terms))
    }

    // ---- Term list ----

    fn parse_term_list(&mut self) -> Result<Vec<Term>, ParseError> {
        if self.current == Token::RParen {
            return Ok(vec![]); // empty argument list
        }
        let mut terms = vec![self.parse_term()?];
        while self.current == Token::Comma {
            self.advance()?;
            terms.push(self.parse_term()?);
        }
        Ok(terms)
    }

    // ---- Term ----

    fn parse_term(&mut self) -> Result<Term, ParseError> {
        match &self.current {
            Token::Ident(name) => {
                let term = Term::Var(name.clone());
                self.advance()?;
                Ok(term)
            }
            Token::Underscore => {
                let term = Term::Var(self.fresh_anon());
                self.advance()?;
                Ok(term)
            }
            Token::Int(v) => {
                let term = Term::Const(Value::Int(*v));
                self.advance()?;
                Ok(term)
            }
            Token::Str(s) => {
                let s = s.clone();
                self.advance()?;
                Ok(Term::StrLit(s))
            }
            other => Err(ParseError {
                message: format!("expected term, found {:?}", other),
                line: self.current_span.line,
                col: self.current_span.col,
            }),
        }
    }

    // ---- Arithmetic operator (lookahead) ----

    fn try_arith_op(&self) -> Option<ArithOp> {
        match &self.current {
            Token::Plus => Some(ArithOp::Add),
            Token::Minus => Some(ArithOp::Sub),
            Token::Star => Some(ArithOp::Mul),
            Token::Slash => Some(ArithOp::Div),
            Token::Percent => Some(ArithOp::Mod),
            _ => None,
        }
    }

    // ---- Comparison operator ----

    fn parse_comp_op(&mut self) -> Result<CompOp, ParseError> {
        let op = match &self.current {
            Token::Eq => CompOp::Eq,
            Token::Ne => CompOp::Ne,
            Token::Lt => CompOp::Lt,
            Token::Le => CompOp::Le,
            Token::Gt => CompOp::Gt,
            Token::Ge => CompOp::Ge,
            other => {
                return Err(ParseError {
                    message: format!("expected comparison operator, found {:?}", other),
                    line: self.current_span.line,
                    col: self.current_span.col,
                });
            }
        };
        self.advance()?;
        Ok(op)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_rule() {
        let program = parse_program("path(x, y) :- edge(x, y).").unwrap();
        assert_eq!(program.rules.len(), 1);
        assert_eq!(program.rules[0].head.predicate, "path");
        assert_eq!(program.rules[0].head.terms.len(), 2);
        assert_eq!(program.rules[0].body.len(), 1);
    }

    #[test]
    fn test_parse_transitive_closure() {
        let input = r#"
            path(x, y) :- edge(x, y).
            path(x, y) :- path(x, z), edge(z, y).
        "#;
        let program = parse_program(input).unwrap();
        assert_eq!(program.rules.len(), 2);
        assert_eq!(program.rules[1].body.len(), 2);
    }

    #[test]
    fn test_parse_negation() {
        let input = "unreachable(x) :- node(x), not path(1, x).";
        let program = parse_program(input).unwrap();
        assert_eq!(program.rules[0].body.len(), 2);
        assert!(matches!(program.rules[0].body[1], BodyElement::Negated(_)));
    }

    #[test]
    fn test_parse_guard() {
        let input = "big_edge(x, y) :- edge(x, y), x > 10.";
        let program = parse_program(input).unwrap();
        assert_eq!(program.rules[0].body.len(), 2);
        match &program.rules[0].body[1] {
            BodyElement::Guard(g) => {
                assert_eq!(g.op, CompOp::Gt);
                assert!(matches!(&g.right, Term::Const(Value::Int(10))));
            }
            other => panic!("expected Guard, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_constant_in_atom() {
        let input = "call_expr(id) :- exprs(id, 74, _).";
        let program = parse_program(input).unwrap();
        let body_atom = match &program.rules[0].body[0] {
            BodyElement::Positive(a) => a,
            _ => panic!("expected positive atom"),
        };
        assert_eq!(body_atom.terms.len(), 3);
        assert!(matches!(&body_atom.terms[1], Term::Const(Value::Int(74))));
        // Wildcard _ should be a unique anonymous variable
        match &body_atom.terms[2] {
            Term::Var(name) => assert!(name.starts_with("_anon")),
            _ => panic!("expected anonymous variable for _"),
        }
    }

    #[test]
    fn test_parse_fact() {
        let input = "base(1, 2).";
        let program = parse_program(input).unwrap();
        assert_eq!(program.rules.len(), 1);
        assert!(program.rules[0].body.is_empty());
    }

    #[test]
    fn test_parse_comments() {
        let input = r#"
            // This is a comment
            path(x, y) :- edge(x, y).  // inline comment
            /* block comment */
            path(x, y) :- path(x, z), edge(z, y).
        "#;
        let program = parse_program(input).unwrap();
        assert_eq!(program.rules.len(), 2);
    }

    #[test]
    fn test_parse_multiple_wildcards() {
        let input = "result(x) :- foo(x, _, _).";
        let program = parse_program(input).unwrap();
        let body_atom = match &program.rules[0].body[0] {
            BodyElement::Positive(a) => a,
            _ => panic!("expected positive atom"),
        };
        // Each _ should get a unique name
        let v1 = match &body_atom.terms[1] { Term::Var(n) => n.clone(), _ => panic!() };
        let v2 = match &body_atom.terms[2] { Term::Var(n) => n.clone(), _ => panic!() };
        assert_ne!(v1, v2, "wildcards should have different names");
    }

    #[test]
    fn test_parse_equality_guard() {
        let input = "same(x) :- edge(x, y), x = y.";
        let program = parse_program(input).unwrap();
        match &program.rules[0].body[1] {
            BodyElement::Guard(g) => {
                assert_eq!(g.op, CompOp::Eq);
                assert!(matches!(&g.left, Term::Var(n) if n == "x"));
                assert!(matches!(&g.right, Term::Var(n) if n == "y"));
            }
            other => panic!("expected Guard, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_negative_int() {
        let input = "neg(x) :- vals(x), x > -5.";
        let program = parse_program(input).unwrap();
        match &program.rules[0].body[1] {
            BodyElement::Guard(g) => {
                assert!(matches!(&g.right, Term::Const(Value::Int(-5))));
            }
            _ => panic!("expected guard"),
        }
    }

    #[test]
    fn test_parse_all_comp_ops() {
        let ops = vec![
            ("x = 1", CompOp::Eq),
            ("x != 1", CompOp::Ne),
            ("x < 1", CompOp::Lt),
            ("x <= 1", CompOp::Le),
            ("x > 1", CompOp::Gt),
            ("x >= 1", CompOp::Ge),
        ];
        for (guard_str, expected_op) in ops {
            let input = format!("r(x) :- s(x), {}.", guard_str);
            let program = parse_program(&input).unwrap();
            match &program.rules[0].body[1] {
                BodyElement::Guard(g) => assert_eq!(g.op, expected_op, "failed for: {}", guard_str),
                _ => panic!("expected guard for: {}", guard_str),
            }
        }
    }

    #[test]
    fn test_parse_callgraph_rules() {
        let input = r#"
            // Direct call graph resolution
            direct_call(caller_name, callee_name) :-
                exprs(call_id, 74, _loc1),
                exprparents(callee_var, 0, call_id),
                exprs(callee_var, 84, _loc2),
                valuetext(callee_var, callee_name),
                enclosingfunction(call_id, caller_func),
                functions(caller_func, caller_name, _kind).

            // Transitive call reachability
            transitive_call(a, b) :- direct_call(a, b).
            transitive_call(a, b) :- transitive_call(a, c), direct_call(c, b).
        "#;
        let program = parse_program(input).unwrap();
        assert_eq!(program.rules.len(), 3);
        assert_eq!(program.rules[0].head.predicate, "direct_call");
        assert_eq!(program.rules[0].body.len(), 6);
        assert_eq!(program.rules[1].head.predicate, "transitive_call");
        assert_eq!(program.rules[2].head.predicate, "transitive_call");
        assert_eq!(program.rules[2].body.len(), 2);
    }

    #[test]
    fn test_parse_empty_program() {
        let program = parse_program("").unwrap();
        assert!(program.rules.is_empty());
    }

    #[test]
    fn test_parse_error_missing_dot() {
        let err = parse_program("path(x, y) :- edge(x, y)").unwrap_err();
        assert!(err.message.contains("expected"));
    }

    #[test]
    fn test_parse_error_bad_token() {
        let err = parse_program("path(x, y) :- edge(x, y) @.").unwrap_err();
        assert!(err.message.contains("unexpected character"));
    }

    #[test]
    fn test_parse_arithmetic_add() {
        let input = "sum(x, z) :- vals(x, y), z = x + y.";
        let program = parse_program(input).unwrap();
        assert_eq!(program.rules[0].body.len(), 2);
        match &program.rules[0].body[1] {
            BodyElement::Assign { result_var, expr } => {
                assert_eq!(result_var, "z");
                assert_eq!(expr.op, ArithOp::Add);
                assert!(matches!(&expr.left, Term::Var(n) if n == "x"));
                assert!(matches!(&expr.right, Term::Var(n) if n == "y"));
            }
            other => panic!("expected Assign, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_arithmetic_all_ops() {
        let ops = vec![
            ("z = x + y", ArithOp::Add),
            ("z = x - y", ArithOp::Sub),
            ("z = x * y", ArithOp::Mul),
            ("z = x / y", ArithOp::Div),
            ("z = x % y", ArithOp::Mod),
        ];
        for (assign_str, expected_op) in ops {
            let input = format!("r(z) :- s(x, y), {}.", assign_str);
            let program = parse_program(&input).unwrap();
            match &program.rules[0].body[1] {
                BodyElement::Assign { expr, .. } => {
                    assert_eq!(expr.op, expected_op, "failed for: {}", assign_str);
                }
                other => panic!("expected Assign for: {}, got {:?}", assign_str, other),
            }
        }
    }

    #[test]
    fn test_parse_arithmetic_with_constant() {
        let input = "inc(x, z) :- vals(x), z = x + 1.";
        let program = parse_program(input).unwrap();
        match &program.rules[0].body[1] {
            BodyElement::Assign { result_var, expr } => {
                assert_eq!(result_var, "z");
                assert!(matches!(&expr.right, Term::Const(Value::Int(1))));
            }
            other => panic!("expected Assign, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_equality_vs_arithmetic() {
        // x = y should be a Guard (equality), not an Assign
        let input = "r(x) :- s(x, y), x = y.";
        let program = parse_program(input).unwrap();
        assert!(matches!(&program.rules[0].body[1], BodyElement::Guard(_)));

        // z = x + 1 should be an Assign
        let input2 = "r(z) :- s(x), z = x + 1.";
        let program2 = parse_program(input2).unwrap();
        assert!(matches!(&program2.rules[0].body[1], BodyElement::Assign { .. }));
    }

    #[test]
    fn test_parse_subtraction_with_constant() {
        // z = x - 1 should be Assign (subtraction), not z = x followed by -1
        let input = "r(z) :- s(x), z = x - 1.";
        let program = parse_program(input).unwrap();
        match &program.rules[0].body[1] {
            BodyElement::Assign { expr, .. } => {
                assert_eq!(expr.op, ArithOp::Sub);
                assert!(matches!(&expr.right, Term::Const(Value::Int(1))));
            }
            other => panic!("expected Assign with Sub, got {:?}", other),
        }
    }
}
