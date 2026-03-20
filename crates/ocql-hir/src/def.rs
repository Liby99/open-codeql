/// Identifies a source file in the project.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct FileId(pub u32);

/// Identifies a declaration within a file.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct LocalDefId(pub u32);

/// Globally unique identifier for any declaration.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct DefId {
    pub file: FileId,
    pub local: LocalDefId,
}

impl std::fmt::Display for DefId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DefId({}.{})", self.file.0, self.local.0)
    }
}

/// What kind of thing a DefId refers to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DefKind {
    /// A non-member predicate.
    Predicate,
    /// A member predicate (in a class).
    MemberPredicate,
    /// A characteristic predicate.
    CharPredicate,
    /// A class declaration.
    Class,
    /// A class field.
    Field,
    /// An explicit module.
    Module,
    /// A newtype declaration.
    Newtype,
    /// A newtype branch.
    NewtypeBranch,
    /// A variable (parameter, quantifier-bound, from-clause).
    Variable,
    /// A type alias.
    TypeAlias,
    /// A module alias.
    ModuleAlias,
    /// A predicate alias.
    PredicateAlias,
}
