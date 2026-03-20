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
            }
            ModuleMember::ModuleAlias(ma) => {
                let id = self.alloc_def(DefKind::ModuleAlias, &ma.name.name, ma.span);
                self.namespaces.modules.insert(ma.name.name.clone(), id);
            }
            ModuleMember::PredicateAlias(pa) => {
                let _id = self.alloc_def(DefKind::PredicateAlias, &pa.name.name, pa.span);
                // TODO: register with target arity once we resolve the target
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
