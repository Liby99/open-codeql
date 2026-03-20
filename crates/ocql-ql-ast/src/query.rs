use ocql_common::Span;

use crate::expr::{Expr, OrderByItem, VarDecl};
use crate::formula::Formula;

/// A select clause: `from ... where ... select ...`
#[derive(Debug, Clone, PartialEq)]
pub struct Select {
    pub from: Vec<VarDecl>,
    pub where_clause: Option<Formula>,
    pub select_exprs: Vec<SelectExpr>,
    pub order_by: Vec<OrderByItem>,
    pub span: Span,
}

/// A single expression in the `select` clause, optionally labeled.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectExpr {
    pub expr: Expr,
    pub label: Option<String>,
    pub span: Span,
}
