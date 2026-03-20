use std::collections::HashMap;

use crate::types::Type;
use crate::DefId;

/// The namespaces for a single module.
///
/// In QL, each module has 6 namespaces. For Milestone 1 we focus on
/// types and predicates; module/signature namespaces come later.
#[derive(Clone, Debug, Default)]
pub struct ModuleNamespaces {
    /// Type namespace: type name → DefId.
    pub types: HashMap<String, DefId>,

    /// Predicate namespace: (name, arity) → PredicateInfo.
    pub predicates: HashMap<(String, usize), PredicateInfo>,

    /// Module namespace: module name → DefId (Milestone 3).
    pub modules: HashMap<String, DefId>,
}

/// Information about a predicate needed during name resolution.
#[derive(Clone, Debug)]
pub struct PredicateInfo {
    pub def_id: DefId,
    /// `None` means this is a predicate without result (formula-context only).
    /// `Some(ty)` means it returns a value of that type.
    pub result_type: Option<Type>,
    /// Number of parameters (not counting result).
    pub arity: usize,
}
