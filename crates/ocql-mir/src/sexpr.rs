//! S-expression format for MIR: printing and parsing.
//!
//! The S-expression format provides a human-readable, round-trippable
//! representation of MIR programs for debugging, testing, and hard-coded
//! MIR files.

use crate::nodes::*;

// ============================================================
// Printer
// ============================================================

/// Print a MIR program as S-expressions.
pub fn print_program(program: &MirProgram) -> String {
    let mut out = String::new();
    for (i, pred) in program.predicates.iter().enumerate() {
        if i > 0 {
            out.push_str("\n\n");
        }
        print_predicate(pred, &mut out);
    }
    out
}

fn print_predicate(pred: &MirPredicate, out: &mut String) {
    out.push_str("(predicate ");
    out.push_str(&pred.name);
    out.push('\n');

    // Parameters
    out.push_str("  (params");
    for p in &pred.params {
        out.push_str(&format!(" ({} : {})", p.name, p.ty));
    }
    out.push(')');

    // Body
    match &pred.body {
        MirBody::None => {
            // Abstract — no body
        }
        MirBody::Conjunction(atoms) => {
            out.push('\n');
            print_conjunction(atoms, "  ", out);
        }
        MirBody::Disjunction(clauses) => {
            out.push_str("\n  (or");
            for clause in clauses {
                out.push('\n');
                print_conjunction(clause, "    ", out);
            }
            out.push(')');
        }
    }
    out.push(')');
}

fn print_conjunction(atoms: &[MirAtom], indent: &str, out: &mut String) {
    out.push_str(indent);
    out.push_str("(and");
    for atom in atoms {
        out.push('\n');
        out.push_str(indent);
        out.push_str("  ");
        print_atom(atom, out);
    }
    out.push(')');
}

fn print_atom(atom: &MirAtom, out: &mut String) {
    match atom {
        MirAtom::Scan(scan) => {
            out.push_str("(scan ");
            out.push_str(&scan.predicate);
            for arg in &scan.args {
                out.push(' ');
                print_term(arg, out);
            }
            out.push(')');
        }
        MirAtom::NegScan(scan) => {
            out.push_str("(not ");
            out.push_str(&scan.predicate);
            for arg in &scan.args {
                out.push(' ');
                print_term(arg, out);
            }
            out.push(')');
        }
        MirAtom::Guard(guard) => {
            out.push_str(&format!("({} ", guard.op));
            print_term(&guard.left, out);
            out.push(' ');
            print_term(&guard.right, out);
            out.push(')');
        }
        MirAtom::Assign(assign) => {
            out.push_str(&format!("(= {} ({} ", assign.result_var, assign.expr.op));
            print_term(&assign.expr.left, out);
            out.push(' ');
            print_term(&assign.expr.right, out);
            out.push_str("))");
        }
        MirAtom::Aggregate(agg) => {
            out.push_str(&format!(
                "({} {} {} ({}) {})",
                agg.function,
                agg.result_var,
                agg.sub_predicate,
                agg.group_by.join(" "),
                agg.agg_var,
            ));
        }
        MirAtom::TypeCheck(tc) => {
            out.push_str(&format!("(instanceof {} {})", tc.var, tc.type_predicate));
        }
    }
}

fn print_term(term: &MirTerm, out: &mut String) {
    match term {
        MirTerm::Var(name) => out.push_str(name),
        MirTerm::Const(c) => out.push_str(&c.to_string()),
        MirTerm::Wildcard => out.push('_'),
    }
}

// ============================================================
// Parser
// ============================================================

/// Parse a MIR program from S-expression text.
pub fn parse_program(input: &str) -> Result<MirProgram, ParseError> {
    let tokens = tokenize(input)?;
    let mut pos = 0;
    let mut predicates = Vec::new();
    while pos < tokens.len() {
        let (pred, next) = parse_predicate(&tokens, pos)?;
        predicates.push(pred);
        pos = next;
    }
    Ok(MirProgram { predicates })
}

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MIR parse error: {}", self.message)
    }
}

impl std::error::Error for ParseError {}

fn err(msg: impl Into<String>) -> ParseError {
    ParseError { message: msg.into() }
}

// ---- Tokenizer ----

#[derive(Debug, Clone, PartialEq)]
enum Token {
    LParen,
    RParen,
    Name(String),
    Int(i64),
    Float(f64),
    Str(String),
    Colon,
}

fn tokenize(input: &str) -> Result<Vec<Token>, ParseError> {
    let mut tokens = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b' ' | b'\t' | b'\n' | b'\r' => i += 1,
            b';' => {
                // Line comment
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'(' => { tokens.push(Token::LParen); i += 1; }
            b')' => { tokens.push(Token::RParen); i += 1; }
            b':' => { tokens.push(Token::Colon); i += 1; }
            b'"' => {
                i += 1;
                let mut s = String::new();
                while i < bytes.len() && bytes[i] != b'"' {
                    if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        i += 1;
                        match bytes[i] {
                            b'n' => s.push('\n'),
                            b't' => s.push('\t'),
                            b'\\' => s.push('\\'),
                            b'"' => s.push('"'),
                            _ => { s.push('\\'); s.push(bytes[i] as char); }
                        }
                    } else {
                        s.push(bytes[i] as char);
                    }
                    i += 1;
                }
                if i >= bytes.len() {
                    return Err(err("unterminated string literal"));
                }
                i += 1; // skip closing quote
                tokens.push(Token::Str(s));
            }
            b'-' if i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() => {
                let start = i;
                i += 1;
                let mut is_float = false;
                while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                    if bytes[i] == b'.' { is_float = true; }
                    i += 1;
                }
                let text = &input[start..i];
                if is_float {
                    let v: f64 = text.parse().map_err(|e| err(format!("bad float: {}", e)))?;
                    tokens.push(Token::Float(v));
                } else {
                    let v: i64 = text.parse().map_err(|e| err(format!("bad int: {}", e)))?;
                    tokens.push(Token::Int(v));
                }
            }
            c if c.is_ascii_digit() => {
                let start = i;
                let mut is_float = false;
                while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                    if bytes[i] == b'.' { is_float = true; }
                    i += 1;
                }
                let text = &input[start..i];
                if is_float {
                    let v: f64 = text.parse().map_err(|e| err(format!("bad float: {}", e)))?;
                    tokens.push(Token::Float(v));
                } else {
                    let v: i64 = text.parse().map_err(|e| err(format!("bad int: {}", e)))?;
                    tokens.push(Token::Int(v));
                }
            }
            _ => {
                // Name: alphanumeric, _, #, ., @, +, -, *, /, %, !, =, <, >
                let start = i;
                while i < bytes.len() && !matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r' | b'(' | b')' | b':' | b'"' | b';') {
                    i += 1;
                }
                if i == start {
                    return Err(err(format!("unexpected byte: {:?}", bytes[i] as char)));
                }
                tokens.push(Token::Name(input[start..i].to_string()));
            }
        }
    }
    Ok(tokens)
}

// ---- Recursive descent parser ----

fn expect_lparen(tokens: &[Token], pos: usize) -> Result<usize, ParseError> {
    match tokens.get(pos) {
        Some(Token::LParen) => Ok(pos + 1),
        _ => Err(err(format!("expected '(' at position {}", pos))),
    }
}

fn expect_rparen(tokens: &[Token], pos: usize) -> Result<usize, ParseError> {
    match tokens.get(pos) {
        Some(Token::RParen) => Ok(pos + 1),
        _ => Err(err(format!("expected ')' at position {}", pos))),
    }
}

fn expect_name(tokens: &[Token], pos: usize) -> Result<(&str, usize), ParseError> {
    match tokens.get(pos) {
        Some(Token::Name(s)) => Ok((s.as_str(), pos + 1)),
        _ => Err(err(format!("expected name at position {}", pos))),
    }
}

fn expect_keyword(tokens: &[Token], pos: usize, kw: &str) -> Result<usize, ParseError> {
    match tokens.get(pos) {
        Some(Token::Name(s)) if s == kw => Ok(pos + 1),
        _ => Err(err(format!("expected '{}' at position {}", kw, pos))),
    }
}

#[allow(dead_code)]
fn peek_name(tokens: &[Token], pos: usize) -> Option<&str> {
    match tokens.get(pos) {
        Some(Token::Name(s)) => Some(s.as_str()),
        _ => None,
    }
}

fn parse_predicate(tokens: &[Token], pos: usize) -> Result<(MirPredicate, usize), ParseError> {
    let pos = expect_lparen(tokens, pos)?;
    let pos = expect_keyword(tokens, pos, "predicate")?;
    let (name, pos) = expect_name(tokens, pos)?;
    let name = name.to_string();

    // Parse params
    let (params, pos) = parse_params(tokens, pos)?;

    // Parse body (optional — abstract if missing)
    let (body, is_abstract, pos) = if matches!(tokens.get(pos), Some(Token::RParen)) {
        (MirBody::None, true, pos)
    } else {
        let (body, pos) = parse_body(tokens, pos)?;
        (body, false, pos)
    };

    let pos = expect_rparen(tokens, pos)?;

    Ok((MirPredicate {
        name,
        params,
        body,
        annotations: MirAnnotations::default(),
        is_abstract,
    }, pos))
}

fn parse_params(tokens: &[Token], pos: usize) -> Result<(Vec<MirParam>, usize), ParseError> {
    let pos = expect_lparen(tokens, pos)?;
    let pos = expect_keyword(tokens, pos, "params")?;
    let mut params = Vec::new();
    let mut pos = pos;
    while matches!(tokens.get(pos), Some(Token::LParen)) {
        let (param, next) = parse_param(tokens, pos)?;
        params.push(param);
        pos = next;
    }
    let pos = expect_rparen(tokens, pos)?;
    Ok((params, pos))
}

fn parse_param(tokens: &[Token], pos: usize) -> Result<(MirParam, usize), ParseError> {
    let pos = expect_lparen(tokens, pos)?;
    let (name, pos) = expect_name(tokens, pos)?;
    let name = name.to_string();
    // expect ':'
    let pos = match tokens.get(pos) {
        Some(Token::Colon) => pos + 1,
        _ => return Err(err(format!("expected ':' in param at position {}", pos))),
    };
    let (ty, pos) = parse_type(tokens, pos)?;
    let pos = expect_rparen(tokens, pos)?;
    Ok((MirParam { name, ty }, pos))
}

fn parse_type(tokens: &[Token], pos: usize) -> Result<(MirType, usize), ParseError> {
    let (name, pos) = expect_name(tokens, pos)?;
    let ty = match name {
        "int" => MirType::Int,
        "float" => MirType::Float,
        "string" => MirType::String,
        "boolean" => MirType::Boolean,
        "date" => MirType::Date,
        "any" => MirType::Any,
        s if s.starts_with('@') => MirType::Entity(s[1..].to_string()),
        s => MirType::Class(s.to_string()),
    };
    Ok((ty, pos))
}

fn parse_body(tokens: &[Token], pos: usize) -> Result<(MirBody, usize), ParseError> {
    let pos = expect_lparen(tokens, pos)?;
    let (kw, pos) = expect_name(tokens, pos)?;
    match kw {
        "and" => {
            let (atoms, pos) = parse_atoms(tokens, pos)?;
            let pos = expect_rparen(tokens, pos)?;
            Ok((MirBody::Conjunction(atoms), pos))
        }
        "or" => {
            let mut clauses = Vec::new();
            let mut pos = pos;
            while matches!(tokens.get(pos), Some(Token::LParen)) {
                let inner_pos = expect_lparen(tokens, pos)?;
                let _ = expect_keyword(tokens, inner_pos, "and")?;
                // Re-parse as conjunction
                let cpos = expect_keyword(tokens, inner_pos, "and")?;
                let (atoms, cpos) = parse_atoms(tokens, cpos)?;
                let cpos = expect_rparen(tokens, cpos)?;
                clauses.push(atoms);
                pos = cpos;
            }
            let pos = expect_rparen(tokens, pos)?;
            Ok((MirBody::Disjunction(clauses), pos))
        }
        _ => Err(err(format!("expected 'and' or 'or', got '{}'", kw))),
    }
}

fn parse_atoms(tokens: &[Token], pos: usize) -> Result<(Vec<MirAtom>, usize), ParseError> {
    let mut atoms = Vec::new();
    let mut pos = pos;
    while matches!(tokens.get(pos), Some(Token::LParen)) {
        let (atom, next) = parse_atom(tokens, pos)?;
        atoms.push(atom);
        pos = next;
    }
    Ok((atoms, pos))
}

fn parse_atom(tokens: &[Token], pos: usize) -> Result<(MirAtom, usize), ParseError> {
    let pos = expect_lparen(tokens, pos)?;
    let (kw, pos) = expect_name(tokens, pos)?;
    match kw {
        "scan" => {
            let (pred_name, pos) = expect_name(tokens, pos)?;
            let pred_name = pred_name.to_string();
            let (args, pos) = parse_terms(tokens, pos)?;
            let pos = expect_rparen(tokens, pos)?;
            Ok((MirAtom::Scan(MirScan { predicate: pred_name, args }), pos))
        }
        "not" => {
            let (pred_name, pos) = expect_name(tokens, pos)?;
            let pred_name = pred_name.to_string();
            let (args, pos) = parse_terms(tokens, pos)?;
            let pos = expect_rparen(tokens, pos)?;
            Ok((MirAtom::NegScan(MirScan { predicate: pred_name, args }), pos))
        }
        "instanceof" => {
            let (var, pos) = expect_name(tokens, pos)?;
            let var = var.to_string();
            let (type_pred, pos) = expect_name(tokens, pos)?;
            let type_pred = type_pred.to_string();
            let pos = expect_rparen(tokens, pos)?;
            Ok((MirAtom::TypeCheck(MirTypeCheck { var, type_predicate: type_pred }), pos))
        }
        // Comparison operators: =, !=, <, <=, >, >=
        "=" => {
            // Could be assignment (= var (op ...)) or guard (= term term)
            let (first_name, peek_pos) = expect_name(tokens, pos)?;
            if matches!(tokens.get(peek_pos), Some(Token::LParen)) {
                // Assignment: (= result_var (op left right))
                let result_var = first_name.to_string();
                let apos = expect_lparen(tokens, peek_pos)?;
                let (op_name, apos) = expect_name(tokens, apos)?;
                let op = parse_arith_op(op_name)?;
                let (left, apos) = parse_term(tokens, apos)?;
                let (right, apos) = parse_term(tokens, apos)?;
                let apos = expect_rparen(tokens, apos)?;
                let pos = expect_rparen(tokens, apos)?;
                Ok((MirAtom::Assign(MirAssign {
                    result_var,
                    expr: MirArithExpr { left, op, right },
                }), pos))
            } else {
                // Guard: (= term term)
                let left = MirTerm::Var(first_name.to_string());
                let (right, pos) = parse_term(tokens, peek_pos)?;
                let pos = expect_rparen(tokens, pos)?;
                Ok((MirAtom::Guard(MirGuard { left, op: MirCompOp::Eq, right }), pos))
            }
        }
        "!=" => parse_guard_rest(tokens, pos, MirCompOp::Ne),
        "<" => parse_guard_rest(tokens, pos, MirCompOp::Lt),
        "<=" => parse_guard_rest(tokens, pos, MirCompOp::Le),
        ">" => parse_guard_rest(tokens, pos, MirCompOp::Gt),
        ">=" => parse_guard_rest(tokens, pos, MirCompOp::Ge),
        // Aggregate functions
        "count" | "sum" | "min" | "max" | "avg" | "any" | "rank"
        | "concat" | "strictcount" | "strictsum" | "strictconcat" => {
            let function = parse_agg_function(kw)?;
            let (result_var, pos) = expect_name(tokens, pos)?;
            let result_var = result_var.to_string();
            let (sub_pred, pos) = expect_name(tokens, pos)?;
            let sub_pred = sub_pred.to_string();
            // group_by: (var1 var2 ...)
            let pos = expect_lparen(tokens, pos)?;
            let mut group_by = Vec::new();
            let mut gpos = pos;
            while let Some(Token::Name(_)) = tokens.get(gpos) {
                let (n, next) = expect_name(tokens, gpos)?;
                group_by.push(n.to_string());
                gpos = next;
            }
            let gpos = expect_rparen(tokens, gpos)?;
            let (agg_var, pos) = expect_name(tokens, gpos)?;
            let agg_var = agg_var.to_string();
            let pos = expect_rparen(tokens, pos)?;
            Ok((MirAtom::Aggregate(MirAggregate {
                result_var,
                function,
                sub_predicate: sub_pred,
                group_by,
                agg_var,
            }), pos))
        }
        _ => Err(err(format!("unknown atom kind: '{}'", kw))),
    }
}

fn parse_guard_rest(tokens: &[Token], pos: usize, op: MirCompOp) -> Result<(MirAtom, usize), ParseError> {
    let (left, pos) = parse_term(tokens, pos)?;
    let (right, pos) = parse_term(tokens, pos)?;
    let pos = expect_rparen(tokens, pos)?;
    Ok((MirAtom::Guard(MirGuard { left, op, right }), pos))
}

fn parse_terms(tokens: &[Token], pos: usize) -> Result<(Vec<MirTerm>, usize), ParseError> {
    let mut terms = Vec::new();
    let mut pos = pos;
    loop {
        match tokens.get(pos) {
            Some(Token::RParen) | None => break,
            Some(Token::LParen) => break, // next atom starts
            _ => {
                let (term, next) = parse_term(tokens, pos)?;
                terms.push(term);
                pos = next;
            }
        }
    }
    Ok((terms, pos))
}

fn parse_term(tokens: &[Token], pos: usize) -> Result<(MirTerm, usize), ParseError> {
    match tokens.get(pos) {
        Some(Token::Int(v)) => Ok((MirTerm::Const(MirConst::Int(*v)), pos + 1)),
        Some(Token::Float(v)) => Ok((MirTerm::Const(MirConst::Float(*v)), pos + 1)),
        Some(Token::Str(s)) => Ok((MirTerm::Const(MirConst::String(s.clone())), pos + 1)),
        Some(Token::Name(s)) => match s.as_str() {
            "_" => Ok((MirTerm::Wildcard, pos + 1)),
            "true" => Ok((MirTerm::Const(MirConst::Bool(true)), pos + 1)),
            "false" => Ok((MirTerm::Const(MirConst::Bool(false)), pos + 1)),
            _ => Ok((MirTerm::Var(s.clone()), pos + 1)),
        },
        _ => Err(err(format!("expected term at position {}", pos))),
    }
}

fn parse_arith_op(s: &str) -> Result<MirArithOp, ParseError> {
    match s {
        "+" => Ok(MirArithOp::Add),
        "-" => Ok(MirArithOp::Sub),
        "*" => Ok(MirArithOp::Mul),
        "/" => Ok(MirArithOp::Div),
        "%" => Ok(MirArithOp::Mod),
        _ => Err(err(format!("unknown arith op: '{}'", s))),
    }
}

fn parse_agg_function(s: &str) -> Result<MirAggFunction, ParseError> {
    match s {
        "count" => Ok(MirAggFunction::Count),
        "sum" => Ok(MirAggFunction::Sum),
        "min" => Ok(MirAggFunction::Min),
        "max" => Ok(MirAggFunction::Max),
        "avg" => Ok(MirAggFunction::Avg),
        "concat" => Ok(MirAggFunction::Concat),
        "rank" => Ok(MirAggFunction::Rank),
        "strictcount" => Ok(MirAggFunction::StrictCount),
        "strictsum" => Ok(MirAggFunction::StrictSum),
        "strictconcat" => Ok(MirAggFunction::StrictConcat),
        "any" => Ok(MirAggFunction::Any),
        _ => Err(err(format!("unknown agg function: '{}'", s))),
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_simple_predicate() {
        let program = MirProgram {
            predicates: vec![MirPredicate::new(
                "isSmall",
                vec![MirParam::new("x", MirType::Int)],
                vec![
                    MirAtom::Scan(MirScan::new("vals", vec![MirTerm::var("x")])),
                    MirAtom::Guard(MirGuard {
                        left: MirTerm::var("x"),
                        op: MirCompOp::Lt,
                        right: MirTerm::int(10),
                    }),
                ],
            )],
        };

        let text = print_program(&program);
        let parsed = parse_program(&text).unwrap();

        assert_eq!(parsed.predicates.len(), 1);
        assert_eq!(parsed.predicates[0].name, "isSmall");
        assert_eq!(parsed.predicates[0].params.len(), 1);
        assert_eq!(parsed.predicates[0].params[0].name, "x");
        assert_eq!(parsed.predicates[0].params[0].ty, MirType::Int);
    }

    #[test]
    fn round_trip_class_predicates() {
        let program = MirProgram {
            predicates: vec![
                MirPredicate::new(
                    "SmallInt#char",
                    vec![MirParam::new("this", MirType::Int)],
                    vec![
                        MirAtom::Guard(MirGuard {
                            left: MirTerm::var("this"),
                            op: MirCompOp::Ge,
                            right: MirTerm::int(1),
                        }),
                        MirAtom::Guard(MirGuard {
                            left: MirTerm::var("this"),
                            op: MirCompOp::Le,
                            right: MirTerm::int(9),
                        }),
                    ],
                ),
                MirPredicate::new(
                    "SmallInt#double",
                    vec![
                        MirParam::new("this", MirType::Int),
                        MirParam::new("result", MirType::Int),
                    ],
                    vec![
                        MirAtom::Scan(MirScan::new("SmallInt#char", vec![MirTerm::var("this")])),
                        MirAtom::Assign(MirAssign {
                            result_var: "result".to_string(),
                            expr: MirArithExpr {
                                left: MirTerm::var("this"),
                                op: MirArithOp::Mul,
                                right: MirTerm::int(2),
                            },
                        }),
                    ],
                ),
            ],
        };

        let text = print_program(&program);
        let parsed = parse_program(&text).unwrap();

        assert_eq!(parsed.predicates.len(), 2);
        assert_eq!(parsed.predicates[0].name, "SmallInt#char");
        assert_eq!(parsed.predicates[1].name, "SmallInt#double");
    }

    #[test]
    fn parse_abstract_predicate() {
        let input = r#"
            (predicate myAbstract
              (params (x : int)))
        "#;
        let program = parse_program(input).unwrap();
        assert_eq!(program.predicates.len(), 1);
        assert!(program.predicates[0].is_abstract);
    }

    #[test]
    fn round_trip_disjunction() {
        let program = MirProgram {
            predicates: vec![MirPredicate {
                name: "dispatch".to_string(),
                params: vec![MirParam::new("this", MirType::Any), MirParam::new("result", MirType::Any)],
                body: MirBody::Disjunction(vec![
                    vec![
                        MirAtom::Scan(MirScan::new("Sub1#char", vec![MirTerm::var("this")])),
                        MirAtom::Scan(MirScan::new("Sub1#method", vec![MirTerm::var("this"), MirTerm::var("result")])),
                    ],
                    vec![
                        MirAtom::NegScan(MirScan::new("Sub1#char", vec![MirTerm::var("this")])),
                        MirAtom::Scan(MirScan::new("Base#method_impl", vec![MirTerm::var("this"), MirTerm::var("result")])),
                    ],
                ]),
                annotations: MirAnnotations::default(),
                is_abstract: false,
            }],
        };

        let text = print_program(&program);
        let parsed = parse_program(&text).unwrap();
        assert_eq!(parsed.predicates.len(), 1);
        assert!(matches!(parsed.predicates[0].body, MirBody::Disjunction(ref c) if c.len() == 2));
    }

    #[test]
    fn parse_from_string() {
        let input = r#"
            (predicate isSmall
              (params (x : int))
              (and
                (scan vals x)
                (< x 10)))
        "#;

        let program = parse_program(input).unwrap();
        assert_eq!(program.predicates.len(), 1);
        assert_eq!(program.predicates[0].name, "isSmall");
    }

    #[test]
    fn round_trip_aggregate() {
        let program = MirProgram {
            predicates: vec![MirPredicate::new(
                "countSmall",
                vec![MirParam::new("result", MirType::Int)],
                vec![MirAtom::Aggregate(MirAggregate {
                    result_var: "result".to_string(),
                    function: MirAggFunction::Count,
                    sub_predicate: "_agg_body_0".to_string(),
                    group_by: vec![],
                    agg_var: "x".to_string(),
                })],
            )],
        };

        let text = print_program(&program);
        let parsed = parse_program(&text).unwrap();
        assert_eq!(parsed.predicates.len(), 1);
        if let MirBody::Conjunction(atoms) = &parsed.predicates[0].body {
            assert!(matches!(&atoms[0], MirAtom::Aggregate(_)));
        } else {
            panic!("expected conjunction");
        }
    }

    #[test]
    fn round_trip_wildcard_and_string() {
        let program = MirProgram {
            predicates: vec![MirPredicate::new(
                "findHello",
                vec![MirParam::new("x", MirType::Any)],
                vec![
                    MirAtom::Scan(MirScan::new("messages", vec![MirTerm::var("x"), MirTerm::string("hello"), MirTerm::Wildcard])),
                ],
            )],
        };

        let text = print_program(&program);
        assert!(text.contains("\"hello\""));
        assert!(text.contains("_"));

        let parsed = parse_program(&text).unwrap();
        if let MirBody::Conjunction(atoms) = &parsed.predicates[0].body {
            if let MirAtom::Scan(scan) = &atoms[0] {
                assert_eq!(scan.args[1], MirTerm::string("hello"));
                assert_eq!(scan.args[2], MirTerm::Wildcard);
            }
        }
    }

    #[test]
    fn round_trip_entity_type() {
        let program = MirProgram {
            predicates: vec![MirPredicate::new(
                "isFunc",
                vec![MirParam::new("this", MirType::Entity("function".to_string()))],
                vec![MirAtom::Scan(MirScan::new("functions", vec![MirTerm::var("this"), MirTerm::Wildcard]))],
            )],
        };

        let text = print_program(&program);
        assert!(text.contains("@function"));
        let parsed = parse_program(&text).unwrap();
        assert_eq!(parsed.predicates[0].params[0].ty, MirType::Entity("function".to_string()));
    }

    #[test]
    fn round_trip_instanceof() {
        let program = MirProgram {
            predicates: vec![MirPredicate::new(
                "test",
                vec![MirParam::new("x", MirType::Any)],
                vec![MirAtom::TypeCheck(MirTypeCheck {
                    var: "x".to_string(),
                    type_predicate: "Function#char".to_string(),
                })],
            )],
        };

        let text = print_program(&program);
        let parsed = parse_program(&text).unwrap();
        if let MirBody::Conjunction(atoms) = &parsed.predicates[0].body {
            assert!(matches!(&atoms[0], MirAtom::TypeCheck(tc) if tc.var == "x"));
        }
    }
}
