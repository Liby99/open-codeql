use std::collections::HashMap;

use crate::def::DefId;
use crate::namespace::PredicateInfo;
use crate::types::Type;

/// Information about a class in the hierarchy.
#[derive(Clone, Debug)]
pub struct ClassInfo {
    /// The class's own DefId.
    pub def_id: DefId,
    /// Supertype DefIds (resolved from class declaration).
    pub supertypes: Vec<DefId>,
    /// Supertype names (unresolved, for diagnostics).
    pub supertype_names: Vec<String>,
    /// Member predicates defined directly on this class: (name, arity) → PredicateInfo.
    pub member_predicates: HashMap<(String, usize), PredicateInfo>,
    /// Fields defined directly on this class: name → Type.
    pub fields: HashMap<String, Type>,
}

/// The class hierarchy for the entire project.
pub struct ClassHierarchy {
    /// Class DefId → ClassInfo.
    classes: HashMap<DefId, ClassInfo>,
    /// Class name → DefId (for looking up classes by name across all files).
    name_to_def: HashMap<String, Vec<DefId>>,
}

impl ClassHierarchy {
    pub fn new() -> Self {
        Self {
            classes: HashMap::new(),
            name_to_def: HashMap::new(),
        }
    }

    /// Register a class in the hierarchy.
    pub fn register_class(&mut self, info: ClassInfo) {
        self.classes.insert(info.def_id, info);
    }

    /// Register a name → DefId mapping.
    pub fn register_name(&mut self, name: String, def_id: DefId) {
        self.name_to_def.entry(name).or_default().push(def_id);
    }

    /// Resolve supertype names to DefIds using a type lookup function.
    /// Call this after all classes are registered.
    pub fn resolve_supertypes<F>(&mut self, lookup: F)
    where
        F: Fn(&str) -> Option<DefId>,
    {
        // Collect updates to avoid borrow issues
        let updates: Vec<(DefId, Vec<DefId>)> = self
            .classes
            .iter()
            .map(|(&def_id, info)| {
                let resolved: Vec<DefId> = info
                    .supertype_names
                    .iter()
                    .filter_map(|name| lookup(name))
                    .collect();
                (def_id, resolved)
            })
            .collect();

        for (def_id, supertypes) in updates {
            if let Some(info) = self.classes.get_mut(&def_id) {
                info.supertypes = supertypes;
            }
        }
    }

    /// Look up a member predicate on a class, walking up the hierarchy.
    /// Returns the first match found (depth-first, parents first).
    pub fn lookup_member_predicate(
        &self,
        class_def: DefId,
        name: &str,
        arity: usize,
    ) -> Option<&PredicateInfo> {
        self.lookup_member_predicate_impl(class_def, name, arity, 0)
    }

    fn lookup_member_predicate_impl(
        &self,
        class_def: DefId,
        name: &str,
        arity: usize,
        depth: usize,
    ) -> Option<&PredicateInfo> {
        if depth > 20 {
            return None; // prevent infinite recursion
        }
        let info = self.classes.get(&class_def)?;
        // Check this class's own predicates
        if let Some(pred) = info.member_predicates.get(&(name.to_string(), arity)) {
            return Some(pred);
        }
        // Walk supertypes
        for &sup in &info.supertypes {
            if let Some(pred) = self.lookup_member_predicate_impl(sup, name, arity, depth + 1) {
                return Some(pred);
            }
        }
        None
    }

    /// Look up a field on a class, walking up the hierarchy.
    pub fn lookup_field(&self, class_def: DefId, name: &str) -> Option<&Type> {
        self.lookup_field_impl(class_def, name, 0)
    }

    fn lookup_field_impl(&self, class_def: DefId, name: &str, depth: usize) -> Option<&Type> {
        if depth > 20 {
            return None;
        }
        let info = self.classes.get(&class_def)?;
        if let Some(ty) = info.fields.get(name) {
            return Some(ty);
        }
        for &sup in &info.supertypes {
            if let Some(ty) = self.lookup_field_impl(sup, name, depth + 1) {
                return Some(ty);
            }
        }
        None
    }

    /// Get all inherited fields for a class (including from supertypes).
    pub fn all_fields(&self, class_def: DefId) -> Vec<(String, Type)> {
        let mut result = Vec::new();
        self.collect_fields(class_def, &mut result, 0);
        result
    }

    fn collect_fields(&self, class_def: DefId, result: &mut Vec<(String, Type)>, depth: usize) {
        if depth > 20 {
            return;
        }
        let Some(info) = self.classes.get(&class_def) else {
            return;
        };
        // Add supertypes' fields first (so they appear before own fields)
        for &sup in &info.supertypes {
            self.collect_fields(sup, result, depth + 1);
        }
        // Add own fields
        for (name, ty) in &info.fields {
            if !result.iter().any(|(n, _)| n == name) {
                result.push((name.clone(), ty.clone()));
            }
        }
    }

    /// Get class info.
    pub fn get(&self, def_id: DefId) -> Option<&ClassInfo> {
        self.classes.get(&def_id)
    }
}
