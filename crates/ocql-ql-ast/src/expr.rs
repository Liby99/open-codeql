use ocql_common::Span;

use crate::formula::Formula;
use crate::ty::TypeExpr;
use crate::{AggKind, BinOp, ClosureOp, Literal, LowerName, SortDir, UnaryOp, UpperName};

/// An expression in QL.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

/// A variable declaration: `Type name`.
#[derive(Debug, Clone, PartialEq)]
pub struct VarDecl {
    pub ty: TypeExpr,
    pub name: LowerName,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    /// A literal value: `42`, `"hello"`, `true`
    Literal(Literal),

    /// A variable reference: `x`
    Variable(LowerName),

    /// `this`
    This,

    /// `result`
    Result,

    /// Don't-care expression: `_`
    DontCare,

    /// Binary operation: `a + b`, `a * b`
    BinaryOp {
        lhs: Box<Expr>,
        op: BinOp,
        rhs: Box<Expr>,
    },

    /// Unary operation: `-x`, `+x`
    UnaryOp {
        op: UnaryOp,
        operand: Box<Expr>,
    },

    /// Non-member predicate call with result: `getName(x)` or closure call `getName+(x)`
    Call {
        name: LowerName,
        closure: Option<ClosureOp>,
        args: Vec<Expr>,
    },

    /// Member predicate call with result: `x.getName()`
    MemberCall {
        receiver: Box<Expr>,
        name: LowerName,
        closure: Option<ClosureOp>,
        args: Vec<Expr>,
    },

    /// Qualified call: `Module::predName(args)`
    QualifiedCall {
        qualifier: UpperName,
        name: LowerName,
        args: Vec<Expr>,
    },

    /// Type/constructor call: `TMyType(args)` or `Module::TMyType(args)`
    TypeCall {
        qualifier: Option<UpperName>,
        name: UpperName,
        args: Vec<Expr>,
    },

    /// Cast expression (postfix): `x.(Type)`
    PostfixCast {
        expr: Box<Expr>,
        ty: TypeExpr,
    },

    /// Cast expression (prefix): `(Type)x`
    PrefixCast {
        ty: TypeExpr,
        expr: Box<Expr>,
    },

    /// Range: `[low .. high]`
    Range {
        low: Box<Expr>,
        high: Box<Expr>,
    },

    /// Set literal: `[a, b, c]`
    SetLiteral {
        elements: Vec<Expr>,
    },

    /// Aggregation: `count(Type x | guard | expr)` or `concat(... | expr, sep)`
    Aggregation {
        kind: AggKind,
        vars: Vec<VarDecl>,
        guard: Option<Box<Formula>>,
        expr: Option<Box<Expr>>,
        separator: Option<Box<Expr>>,
        order_by: Vec<OrderByItem>,
    },

    /// Rank aggregation: `rank[i](vars | guard | expr)`
    RankExpr {
        index: Box<Expr>,
        vars: Vec<VarDecl>,
        guard: Option<Box<Formula>>,
        expr: Box<Expr>,
        order_by: Vec<OrderByItem>,
    },

    /// `any(Type x | guard | expr)` expression
    AnyExpr {
        vars: Vec<VarDecl>,
        guard: Option<Box<Formula>>,
        expr: Option<Box<Expr>>,
    },

    /// Super expression: `Type.super.method(args)`
    Super {
        super_type: UpperName,
        name: LowerName,
        args: Vec<Expr>,
    },

    /// Parenthesized expression: `(expr)`
    Paren(Box<Expr>),

    /// A formula embedded in expression context (bridge node from parser).
    /// This occurs when a parenthesized formula like `(x = 1 or y = 2)`
    /// appears where an expression is expected. Resolved in a later phase.
    FormulaExpr(Box<Formula>),
}

/// An `order by` item.
#[derive(Debug, Clone, PartialEq)]
pub struct OrderByItem {
    pub expr: Expr,
    pub dir: Option<SortDir>,
    pub span: Span,
}
