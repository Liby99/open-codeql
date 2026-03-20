use ocql_common::Span;

use crate::annotation::Annotation;
use crate::expr::VarDecl;
use crate::formula::Formula;
use crate::ty::TypeExpr;
use crate::LowerName;

/// A predicate declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct Predicate {
    pub annotations: Vec<Annotation>,
    pub head: PredicateHead,
    pub body: Option<Formula>,
    pub span: Span,
}

/// The head of a predicate declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct PredicateHead {
    /// `None` means `predicate` keyword (no result type).
    /// `Some(ty)` means the predicate returns a value of that type.
    pub result_type: Option<TypeExpr>,
    pub name: LowerName,
    pub params: Vec<VarDecl>,
    pub span: Span,
}
