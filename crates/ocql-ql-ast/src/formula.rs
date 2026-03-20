use ocql_common::Span;

use crate::expr::{Expr, VarDecl};
use crate::ty::TypeExpr;
use crate::{ClosureOp, CompOp, LowerName, UpperName};

/// A formula (logical constraint) in QL.
#[derive(Debug, Clone, PartialEq)]
pub struct Formula {
    pub kind: FormulaKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FormulaKind {
    /// Conjunction: `A and B`
    Conjunction {
        lhs: Box<Formula>,
        rhs: Box<Formula>,
    },

    /// Disjunction: `A or B`
    Disjunction {
        lhs: Box<Formula>,
        rhs: Box<Formula>,
    },

    /// Negation: `not A`
    Negation {
        inner: Box<Formula>,
    },

    /// Comparison: `x = y`, `x != y`, `x < y`, etc.
    Comparison {
        lhs: Expr,
        op: CompOp,
        rhs: Expr,
    },

    /// Type check: `expr instanceof Type`
    InstanceOf {
        expr: Expr,
        ty: TypeExpr,
    },

    /// Range check: `expr in range_expr`
    InRange {
        expr: Expr,
        range: Expr,
    },

    /// Existential quantifier: `exists(Type x | body)` or `exists(Type x | guard | body)`
    Exists {
        vars: Vec<VarDecl>,
        guard: Option<Box<Formula>>,
        body: Box<Formula>,
    },

    /// Short-form exists: `exists(expr)`
    ExistsExpr {
        expr: Expr,
    },

    /// Universal quantifier: `forall(Type x | guard | body)`
    Forall {
        vars: Vec<VarDecl>,
        guard: Box<Formula>,
        body: Box<Formula>,
    },

    /// Universal + existential: `forex(Type x | guard | body)`
    Forex {
        vars: Vec<VarDecl>,
        guard: Box<Formula>,
        body: Box<Formula>,
    },

    /// Implication: `A implies B`
    Implies {
        lhs: Box<Formula>,
        rhs: Box<Formula>,
    },

    /// Conditional: `if A then B else C`
    IfThenElse {
        cond: Box<Formula>,
        then: Box<Formula>,
        else_: Box<Formula>,
    },

    /// Non-member predicate call (no result): `isSmall(x)`
    PredicateCall {
        name: LowerName,
        args: Vec<Expr>,
    },

    /// Member predicate call: `x.isValid()`
    MemberCall {
        receiver: Expr,
        name: LowerName,
        closure: Option<ClosureOp>,
        args: Vec<Expr>,
    },

    /// Qualified predicate call: `Module::pred(args)`
    QualifiedCall {
        qualifier: UpperName,
        name: LowerName,
        args: Vec<Expr>,
    },

    /// `any()` — always true
    Any,

    /// `none()` — always false
    None,

    /// Parenthesized formula: `(formula)`
    Paren {
        inner: Box<Formula>,
    },

    /// An expression in formula context (bridge node from parser).
    /// Typically a call expression used as a predicate call, e.g. `isSmall(x)`.
    /// Resolved in a later phase.
    ExprFormula(Expr),
}
