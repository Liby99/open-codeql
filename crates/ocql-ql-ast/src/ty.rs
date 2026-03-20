use ocql_common::Span;

use crate::UpperName;

/// A type expression in QL source code.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeExpr {
    pub kind: TypeExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeExprKind {
    /// Primitive type: `boolean`, `int`, `float`, `string`, `date`
    Primitive(PrimitiveType),

    /// Database type: `@name`
    Database(String),

    /// Class/type name reference: `UpperName`
    ClassName(UpperName),

    /// Module selection: `Module::Type`
    ModuleAccess(UpperName, UpperName),
}

/// QL primitive types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveType {
    Boolean,
    Int,
    Float,
    String,
    Date,
}
