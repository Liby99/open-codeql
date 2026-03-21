use ocql_common::Span;
use ocql_ql_ast::module::{ClassDecl, ClassMember, ModuleMember, SourceFile};
use ocql_ql_ast::predicate::Predicate;
use ocql_ql_ast::ty::{TypeExpr, TypeExprKind};

use crate::def::{DefId, DefKind, FileId, LocalDefId};
use crate::namespace::{ModuleNamespaces, PredicateInfo};
use crate::types::Type;
use crate::DefInfo;

/// Walks the AST and assigns DefIds to all declarations, populating namespaces.
pub struct DeclarationCollector {
    file_id: FileId,
    next_local: u32,
    defs: Vec<DefInfo>,
    pub namespaces: ModuleNamespaces,
}

impl DeclarationCollector {
    pub fn new(file_id: FileId) -> Self {
        Self {
            file_id,
            next_local: 0,
            defs: Vec::new(),
            namespaces: ModuleNamespaces::default(),
        }
    }

    pub fn next_local_id(&self) -> u32 {
        self.next_local
    }

    fn alloc_def(&mut self, kind: DefKind, name: &str, span: Span) -> DefId {
        let id = DefId {
            file: self.file_id,
            local: LocalDefId(self.next_local),
        };
        self.next_local += 1;
        self.defs.push(DefInfo {
            id,
            kind,
            name: name.to_string(),
            span,
        });
        id
    }

    pub fn collect_source_file(&mut self, sf: &SourceFile) {
        for member in &sf.members {
            self.collect_member(member);
        }
    }

    fn collect_member(&mut self, member: &ModuleMember) {
        match member {
            ModuleMember::Predicate(pred) => self.collect_predicate(pred),
            ModuleMember::Class(class) => self.collect_class(class),
            ModuleMember::Select(_) => {}
            ModuleMember::Module(m) => {
                let id = self.alloc_def(DefKind::Module, &m.name.name, m.span);
                self.namespaces.modules.insert(m.name.name.clone(), id);
                // Recurse into module members
                for member in &m.members {
                    self.collect_member(member);
                }
            }
            ModuleMember::Newtype(nt) => {
                let id = self.alloc_def(DefKind::Newtype, &nt.name.name, nt.span);
                self.namespaces.types.insert(nt.name.name.clone(), id);
                for branch in &nt.branches {
                    let branch_id =
                        self.alloc_def(DefKind::NewtypeBranch, &branch.name.name, branch.span);
                    self.namespaces
                        .types
                        .insert(branch.name.name.clone(), branch_id);
                }
            }
            ModuleMember::TypeAlias(ta) => {
                let id = self.alloc_def(DefKind::TypeAlias, &ta.name.name, ta.span);
                self.namespaces.types.insert(ta.name.name.clone(), id);
                // Track the alias target for type checking (e.g., `class X = int`)
                if let Some(target_type) = Self::resolve_simple_type(&ta.target) {
                    self.namespaces.type_aliases.insert(id, target_type);
                }
            }
            ModuleMember::ModuleAlias(ma) => {
                let id = self.alloc_def(DefKind::ModuleAlias, &ma.name.name, ma.span);
                self.namespaces.modules.insert(ma.name.name.clone(), id);
            }
            ModuleMember::PredicateAlias(pa) => {
                let id = self.alloc_def(DefKind::PredicateAlias, &pa.name.name, pa.span);
                let arity = pa.target_arity as usize;
                // Try to find the target predicate to copy its result type
                let target_name = pa.target_name.parts.last().unwrap_or(&pa.name.name).clone();
                let result_type = self.namespaces.predicates
                    .get(&(target_name, arity))
                    .and_then(|info| info.result_type.clone())
                    // If the target is qualified (not found locally), assume it may
                    // have a result type to avoid false "no result type" errors.
                    .or_else(|| {
                        if pa.target_name.parts.len() > 1 {
                            Some(Type::Error)
                        } else {
                            None
                        }
                    });
                self.namespaces.predicates.insert(
                    (pa.name.name.clone(), arity),
                    PredicateInfo {
                        def_id: id,
                        result_type,
                        arity,
                    },
                );
            }
            ModuleMember::Import(_) => {} // Handled by module graph
            ModuleMember::Signature(_) => {} // Milestone 6
        }
    }

    fn collect_predicate(&mut self, pred: &Predicate) {
        let name = &pred.head.name.name;
        let arity = pred.head.params.len();
        let id = self.alloc_def(DefKind::Predicate, name, pred.span);

        let result_type = pred
            .head
            .result_type
            .as_ref()
            .map(|te| resolve_type_expr_simple(te));

        self.namespaces.predicates.insert(
            (name.clone(), arity),
            PredicateInfo {
                def_id: id,
                result_type,
                arity,
            },
        );
    }

    fn collect_class(&mut self, class: &ClassDecl) {
        let class_id = self.alloc_def(DefKind::Class, &class.name.name, class.span);
        self.namespaces
            .types
            .insert(class.name.name.clone(), class_id);

        // Record primitive supertype for operator resolution.
        // In QL, `class Foo extends int { ... }` inherits int's operators.
        // Also chase through class aliases (e.g., `class Foo extends MyString`
        // where `class MyString = string`).
        for sup in &class.supertypes {
            if let Some(target) = Self::resolve_simple_type(sup) {
                self.namespaces.type_aliases.insert(class_id, target);
                break;
            }
            // If the supertype is a class name, check if it's already a known alias
            if let TypeExprKind::ClassName(name) = &sup.kind {
                if let Some(&sup_def) = self.namespaces.types.get(&name.name) {
                    if let Some(target) = self.namespaces.type_aliases.get(&sup_def) {
                        self.namespaces.type_aliases.insert(class_id, target.clone());
                        break;
                    }
                }
            }
        }

        for member in &class.members {
            match member {
                ClassMember::CharacteristicPredicate { name, span, .. } => {
                    self.alloc_def(DefKind::CharPredicate, &name.name, *span);
                }
                ClassMember::MemberPredicate(pred) => {
                    let pred_name = &pred.head.name.name;
                    let arity = pred.head.params.len();
                    let id = self.alloc_def(DefKind::MemberPredicate, pred_name, pred.span);

                    let result_type = pred
                        .head
                        .result_type
                        .as_ref()
                        .map(|te| resolve_type_expr_simple(te));

                    let qualified_key = format!("{}::{}", class.name.name, pred_name);
                    self.namespaces.predicates.insert(
                        (qualified_key, arity),
                        PredicateInfo {
                            def_id: id,
                            result_type,
                            arity,
                        },
                    );
                }
                ClassMember::Field { name, span, .. } => {
                    self.alloc_def(DefKind::Field, &name.name, *span);
                }
            }
        }
    }

    pub fn into_defs(self) -> Vec<DefInfo> {
        self.defs
    }

    /// Resolve a simple type alias target (primitives only).
    fn resolve_simple_type(te: &TypeExpr) -> Option<Type> {
        match &te.kind {
            TypeExprKind::Primitive(p) => Some(Type::Primitive(*p)),
            _ => None,
        }
    }
}

/// Resolve a TypeExpr to a Type without full name resolution.
/// Used during declaration collection for predicate result types.
pub fn resolve_type_expr_simple(te: &TypeExpr) -> Type {
    match &te.kind {
        TypeExprKind::Primitive(p) => Type::Primitive(*p),
        TypeExprKind::Database(name) => Type::DbEntity(name.clone()),
        TypeExprKind::ClassName(_) => Type::Error, // needs full resolution
        TypeExprKind::ModuleAccess(_, _) => Type::Error,
    }
}
