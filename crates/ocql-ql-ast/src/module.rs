use ocql_common::Span;

use crate::annotation::Annotation;
use crate::expr::VarDecl;
use crate::formula::Formula;
use crate::predicate::Predicate;
use crate::query::Select;
use crate::ty::TypeExpr;
use crate::{LowerName, QualifiedName, UpperName};

/// A complete QL source file (query module or library module).
#[derive(Debug, Clone, PartialEq)]
pub struct SourceFile {
    pub members: Vec<ModuleMember>,
    pub span: Span,
}

/// A member of a module.
#[derive(Debug, Clone, PartialEq)]
pub enum ModuleMember {
    Import(Import),
    Predicate(Predicate),
    Class(ClassDecl),
    Module(ModuleDecl),
    Newtype(NewtypeDecl),
    Select(Select),
    ModuleAlias(ModuleAlias),
    TypeAlias(TypeAlias),
    PredicateAlias(PredicateAlias),
    /// Signature declaration: `signature predicate ...;`, `signature class ...;`, `signature module ...;`
    Signature(SignatureDecl),
}

/// An import statement.
#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    pub annotations: Vec<Annotation>,
    pub path: QualifiedName,
    pub alias: Option<UpperName>,
    pub span: Span,
}

/// A class declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassDecl {
    pub annotations: Vec<Annotation>,
    pub name: UpperName,
    pub supertypes: Vec<TypeExpr>,
    pub instanceof: Vec<TypeExpr>,
    pub members: Vec<ClassMember>,
    /// True for type union aliases: `class T = A or B;` (OR semantics).
    /// False for regular extends classes: `class T extends A, B { }` (AND semantics).
    pub is_union: bool,
    pub span: Span,
}

/// A member of a class.
#[derive(Debug, Clone, PartialEq)]
pub enum ClassMember {
    /// Characteristic predicate: `ClassName() { body }`
    CharacteristicPredicate {
        annotations: Vec<Annotation>,
        name: UpperName,
        body: Formula,
        span: Span,
    },

    /// Member predicate
    MemberPredicate(Predicate),

    /// Field declaration: `Type name;`
    Field {
        annotations: Vec<Annotation>,
        ty: TypeExpr,
        name: LowerName,
        span: Span,
    },
}

/// An explicit module declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleDecl {
    pub annotations: Vec<Annotation>,
    pub name: UpperName,
    pub type_params: Vec<SignatureParam>,
    pub implements: Vec<ModuleExpr>,
    pub members: Vec<ModuleMember>,
    pub span: Span,
}

/// A module expression (for imports, implements, etc.)
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleExpr {
    pub name: UpperName,
    pub args: Vec<ModuleArg>,
    pub span: Span,
}

/// An argument to a parameterized module instantiation.
#[derive(Debug, Clone, PartialEq)]
pub enum ModuleArg {
    Type(TypeExpr),
    PredicateRef(LowerName, usize), // name/arity
    Module(ModuleExpr),
}

/// A parameter in a parameterized module signature.
#[derive(Debug, Clone, PartialEq)]
pub struct SignatureParam {
    pub sig: UpperName,
    pub name: UpperName,
    pub span: Span,
}

/// `newtype` declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct NewtypeDecl {
    pub annotations: Vec<Annotation>,
    pub name: UpperName,
    pub branches: Vec<NewtypeBranch>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewtypeBranch {
    pub name: UpperName,
    pub params: Vec<VarDecl>,
    pub body: Option<Formula>,
    pub span: Span,
}

/// Module alias: `module M = OtherModule;`
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleAlias {
    pub annotations: Vec<Annotation>,
    pub name: UpperName,
    pub target: ModuleExpr,
    pub span: Span,
}

/// Type alias: `class T = OtherType;`
#[derive(Debug, Clone, PartialEq)]
pub struct TypeAlias {
    pub annotations: Vec<Annotation>,
    pub name: UpperName,
    pub target: TypeExpr,
    pub span: Span,
}

/// A signature declaration (predicate, class, or module signature).
#[derive(Debug, Clone, PartialEq)]
pub struct SignatureDecl {
    pub span: Span,
}

/// Predicate alias: `predicate p = other/2;`
#[derive(Debug, Clone, PartialEq)]
pub struct PredicateAlias {
    pub annotations: Vec<Annotation>,
    pub name: LowerName,
    pub target_name: QualifiedName,
    pub target_arity: usize,
    pub span: Span,
}
