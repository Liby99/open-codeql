use crate::DefId;
use ocql_ql_ast::ty::PrimitiveType;

/// A resolved type in the HIR.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    /// Primitive: boolean, int, float, string, date.
    Primitive(PrimitiveType),

    /// A class type, referring to its DefId.
    Class(DefId),

    /// A database entity type (from .dbscheme). Placeholder for Milestone 4.
    DbEntity(String),

    /// A newtype branch.
    NewtypeBranch(DefId),

    /// Error type (produced on type errors to prevent cascading).
    Error,
}

impl Type {
    /// Returns true if this type is numeric (int or float).
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Type::Primitive(PrimitiveType::Int) | Type::Primitive(PrimitiveType::Float)
        )
    }

    /// Returns the result type of a binary arithmetic operation.
    pub fn numeric_result(&self, other: &Type) -> Type {
        match (self, other) {
            (Type::Primitive(PrimitiveType::Float), _) | (_, Type::Primitive(PrimitiveType::Float)) => {
                Type::Primitive(PrimitiveType::Float)
            }
            (Type::Primitive(PrimitiveType::Int), Type::Primitive(PrimitiveType::Int)) => {
                Type::Primitive(PrimitiveType::Int)
            }
            _ => Type::Error,
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            Type::Primitive(p) => match p {
                PrimitiveType::Boolean => "boolean".to_string(),
                PrimitiveType::Int => "int".to_string(),
                PrimitiveType::Float => "float".to_string(),
                PrimitiveType::String => "string".to_string(),
                PrimitiveType::Date => "date".to_string(),
            },
            Type::Class(id) => format!("class({id})"),
            Type::DbEntity(name) => format!("@{name}"),
            Type::NewtypeBranch(id) => format!("newtype_branch({id})"),
            Type::Error => "<error>".to_string(),
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}
