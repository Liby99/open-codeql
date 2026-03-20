pub mod annotation;
pub mod expr;
pub mod formula;
pub mod module;
pub mod predicate;
pub mod query;
pub mod ty;

use ocql_common::Span;

/// A name identifier (lowercase: predicates, variables, fields).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LowerName {
    pub name: String,
    pub span: Span,
}

/// An uppercase identifier (types, classes, modules).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UpperName {
    pub name: String,
    pub span: Span,
}

/// A qualified name like `foo.bar.Baz`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QualifiedName {
    pub parts: Vec<String>,
    pub span: Span,
}

/// A literal value.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
}

/// A comparison operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompOp {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

/// A binary arithmetic/string operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

/// A unary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Neg,
    Plus,
}

/// Closure operator for predicate calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClosureOp {
    Plus,   // Transitive closure (+)
    Star,   // Reflexive transitive closure (*)
}

/// Aggregation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AggKind {
    Count,
    Min,
    Max,
    Avg,
    Sum,
    Concat,
    Rank,
    Unique,
    Any,
    StrictCount,
    StrictSum,
    StrictConcat,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortDir {
    Asc,
    Desc,
}
