use logos::Logos;

/// Lexical error type for the QL lexer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LexicalError {
    pub message: String,
}

impl std::fmt::Display for LexicalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Lexical error: {}", self.message)
    }
}

impl std::error::Error for LexicalError {}

/// QL language tokens.
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r\n\f]+")]
#[logos(skip r"//[^\n]*")]
#[logos(skip r"/\*([^*]|\*[^/])*\*/")]
#[logos(error = LexicalError)]
pub enum Token {
    // ── Keywords ──────────────────────────────────────────

    #[token("and")]
    And,
    #[token("any")]
    Any,
    #[token("as")]
    As,
    #[token("asc")]
    Asc,
    #[token("avg")]
    Avg,
    #[token("boolean")]
    Boolean,
    #[token("by")]
    By,
    #[token("class")]
    Class,
    #[token("concat")]
    Concat,
    #[token("count")]
    Count,
    #[token("date")]
    Date,
    #[token("default")]
    Default,
    #[token("deprecated")]
    Deprecated,
    #[token("desc")]
    Desc,
    #[token("else")]
    Else,
    #[token("exists")]
    Exists,
    #[token("extends")]
    Extends,
    #[token("false")]
    False,
    #[token("final")]
    Final,
    #[token("float")]
    FloatKw,
    #[token("forall")]
    Forall,
    #[token("forex")]
    Forex,
    #[token("from")]
    From,
    #[token("if")]
    If,
    #[token("implements")]
    Implements,
    #[token("import")]
    Import,
    #[token("implies")]
    Implies,
    #[token("in")]
    In,
    #[token("instanceof")]
    InstanceOf,
    #[token("int")]
    IntKw,
    #[token("language")]
    Language,
    #[token("max")]
    Max,
    #[token("min")]
    Min,
    #[token("module")]
    Module,
    #[token("newtype")]
    Newtype,
    #[token("none")]
    None_,
    #[token("not")]
    Not,
    #[token("or")]
    Or,
    #[token("order")]
    Order,
    #[token("override")]
    Override,
    #[token("pragma")]
    Pragma,
    #[token("predicate")]
    Predicate,
    #[token("private")]
    Private,
    #[token("query")]
    Query,
    #[token("rank")]
    Rank,
    #[token("result")]
    Result_,
    #[token("select")]
    Select,
    #[token("signature")]
    Signature,
    #[token("string")]
    StringKw,
    #[token("strictconcat")]
    StrictConcat,
    #[token("strictcount")]
    StrictCount,
    #[token("strictsum")]
    StrictSum,
    #[token("sum")]
    Sum,
    #[token("super")]
    Super,
    #[token("then")]
    Then,
    #[token("this")]
    This,
    #[token("true")]
    True,
    #[token("unique")]
    Unique,
    #[token("where")]
    Where,

    // Annotation-related keywords
    #[token("abstract")]
    Abstract,
    #[token("cached")]
    Cached,
    #[token("external")]
    External,
    #[token("extensible")]
    Extensible,
    #[token("transient")]
    Transient,
    #[token("additional")]
    Additional,
    #[token("library")]
    Library,
    #[token("bindingset")]
    BindingSet,
    #[token("monotonicAggregates")]
    MonotonicAggregates,
    #[token("inline")]
    Inline,
    #[token("inline_late")]
    InlineLate,
    #[token("noinline")]
    NoInline,
    #[token("nomagic")]
    NoMagic,
    #[token("noopt")]
    NoOpt,
    #[token("only_bind_out")]
    OnlyBindOut,
    #[token("only_bind_into")]
    OnlyBindInto,
    #[token("overlay")]
    Overlay,

    // ── Punctuation ──────────────────────────────────────

    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token(",")]
    Comma,
    #[token(";")]
    Semi,
    #[token(".")]
    Dot,
    #[token("..")]
    DotDot,
    #[token("::")]
    ColonColon,
    #[token("|")]
    Pipe,
    #[token("_")]
    Underscore,

    // ── Operators ────────────────────────────────────────

    #[token("=")]
    Eq,
    #[token("!=")]
    Ne,
    #[token("<=")]
    Le,
    #[token(">=")]
    Ge,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("@")]
    At,
    #[token("?")]
    Question,

    // Synthetic tokens for closure operators (emitted by Lexer, not by Logos)
    // ClosurePlus = `+(` with no whitespace between `+` and `(`
    ClosurePlus,
    // ClosureStar = `*(` with no whitespace between `*` and `(`
    ClosureStar,

    // ── Literals ─────────────────────────────────────────

    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    IntLit(i64),

    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().ok())]
    FloatLit(f64),

    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        Some(unescape_string(&s[1..s.len()-1]))
    })]
    StringLit(String),

    // ── Identifiers ──────────────────────────────────────

    #[regex(r"[a-z][a-zA-Z0-9_]*", |lex| lex.slice().to_string(), priority = 1)]
    LowerIdent(String),

    #[regex(r"[A-Z][a-zA-Z0-9_]*", |lex| lex.slice().to_string(), priority = 1)]
    UpperIdent(String),
}

/// Unescape a QL string literal (between quotes).
fn unescape_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('u') => {
                    let hex: String = chars.by_ref().take(4).collect();
                    if let Ok(code) = u32::from_str_radix(&hex, 16) {
                        if let Some(ch) = char::from_u32(code) {
                            result.push(ch);
                        }
                    }
                }
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// An iterator adapter that wraps a Logos lexer to produce LALRPOP-compatible
/// `(start, Token, end)` triples.
pub struct Lexer<'input> {
    input: &'input str,
    inner: logos::SpannedIter<'input, Token>,
}

impl<'input> Lexer<'input> {
    pub fn new(input: &'input str) -> Self {
        Self {
            input,
            inner: Token::lexer(input).spanned(),
        }
    }
}

impl<'input> Iterator for Lexer<'input> {
    type Item = Result<(usize, Token, usize), LexicalError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(token, span)| match token {
            Ok(tok) => {
                // Convert Plus/Star to ClosurePlus/ClosureStar when immediately
                // followed by '(' (no whitespace). This disambiguates closure
                // calls like `name+(args)` from arithmetic `name + (args)`.
                let tok = match tok {
                    Token::Plus if self.input.as_bytes().get(span.end) == Some(&b'(') => Token::ClosurePlus,
                    Token::Star if self.input.as_bytes().get(span.end) == Some(&b'(') => Token::ClosureStar,
                    other => other,
                };
                Ok((span.start, tok, span.end))
            }
            Err(err) => Err(err),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lex_simple_query() {
        let input = r#"from int x where x = 42 select x"#;
        let lexer = Lexer::new(input);
        let tokens: Vec<_> = lexer.collect::<Result<Vec<_>, _>>().unwrap();
        // from, int, x, where, x, =, 42, select, x = 9 tokens
        assert_eq!(tokens.len(), 9);
        assert!(matches!(tokens[0].1, Token::From));
        assert!(matches!(tokens[1].1, Token::IntKw));
        assert!(matches!(tokens[2].1, Token::LowerIdent(ref s) if s == "x"));
        assert!(matches!(tokens[3].1, Token::Where));
        assert!(matches!(tokens[4].1, Token::LowerIdent(ref s) if s == "x"));
        assert!(matches!(tokens[5].1, Token::Eq));
        assert!(matches!(tokens[6].1, Token::IntLit(42)));
        assert!(matches!(tokens[7].1, Token::Select));
        assert!(matches!(tokens[8].1, Token::LowerIdent(ref s) if s == "x"));
    }

    #[test]
    fn test_lex_string_literal() {
        let input = r#""hello\nworld""#;
        let lexer = Lexer::new(input);
        let tokens: Vec<_> = lexer.collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0].1, Token::StringLit(ref s) if s == "hello\nworld"));
    }

    #[test]
    fn test_lex_predicate() {
        let input = "predicate isSmall(int i) { i in [1 .. 9] }";
        let lexer = Lexer::new(input);
        let tokens: Vec<_> = lexer.collect::<Result<Vec<_>, _>>().unwrap();
        assert!(tokens.len() > 5);
        assert!(matches!(tokens[0].1, Token::Predicate));
    }

    #[test]
    fn test_lex_class() {
        let input = "class SmallInt extends int { }";
        let lexer = Lexer::new(input);
        let tokens: Vec<_> = lexer.collect::<Result<Vec<_>, _>>().unwrap();
        assert!(matches!(tokens[0].1, Token::Class));
        assert!(matches!(tokens[1].1, Token::UpperIdent(ref s) if s == "SmallInt"));
        assert!(matches!(tokens[2].1, Token::Extends));
        assert!(matches!(tokens[3].1, Token::IntKw));
    }

    #[test]
    fn test_lex_qldoc_backtick() {
        let input = "/** Gets `delta`. */\npredicate test() { 1 = 1 }";
        let lexer = Lexer::new(input);
        let tokens: Result<Vec<_>, _> = lexer.collect();
        assert!(tokens.is_ok(), "Lex error: {:?}", tokens.err());
        let tokens = tokens.unwrap();
        assert!(matches!(tokens[0].1, Token::Predicate), "Expected Predicate, got {:?}", tokens[0].1);
    }

    #[test]
    fn test_lex_real_file_with_backtick() {
        // This test is for manual debugging - only run when the vendor directory exists
        let _ = std::fs::read_to_string("../../vendor/codeql/csharp/ql/lib/semmle/code/csharp/dataflow/Bound.qll");
    }

    #[test]
    fn test_lex_comments_skipped() {
        let input = "// this is a comment\nfrom int x /* block */ select x";
        let lexer = Lexer::new(input);
        let tokens: Vec<_> = lexer.collect::<Result<Vec<_>, _>>().unwrap();
        assert!(matches!(tokens[0].1, Token::From));
    }
}
