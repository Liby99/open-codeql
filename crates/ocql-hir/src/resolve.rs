use ocql_common::Span;
use ocql_ql_ast::expr::{Expr, ExprKind};
use ocql_ql_ast::formula::{Formula, FormulaKind};
use ocql_ql_ast::module::{ClassDecl, ClassMember, ModuleMember, SourceFile};
use ocql_ql_ast::predicate::Predicate;
use ocql_ql_ast::query::Select;
use ocql_ql_ast::ty::{PrimitiveType, TypeExpr, TypeExprKind};
use ocql_ql_ast::{BinOp, CompOp, Literal};

use std::collections::HashMap;

use crate::collect::DeclarationCollector;
use crate::def::{DefId, FileId, LocalDefId};
use crate::diagnostics::{Diagnostic, Severity};
use crate::namespace::{ModuleNamespaces, PredicateInfo};
use crate::types::Type;

// ---------------------------------------------------------------------------
// Resolved reference
// ---------------------------------------------------------------------------

/// What a name reference resolved to.
#[derive(Clone, Debug)]
pub enum ResolvedRef {
    /// A user-defined declaration.
    Def(DefId),
    /// A builtin operation (e.g., string methods, numeric builtins).
    Builtin(String),
    /// Failed to resolve (error already reported).
    Unresolved,
}

// ---------------------------------------------------------------------------
// Name resolution + type checking (Phases 4 & 5)
// ---------------------------------------------------------------------------

/// Variable scope entry.
struct ScopeVar {
    name: String,
    def_id: DefId,
    ty: Type,
}

/// A scope frame (predicate body, quantifier, select clause, etc.)
struct Scope {
    vars: Vec<ScopeVar>,
}

impl Scope {
    fn new() -> Self {
        Self { vars: Vec::new() }
    }

    fn find(&self, name: &str) -> Option<(DefId, Type)> {
        self.vars
            .iter()
            .rev()
            .find(|v| v.name == name)
            .map(|v| (v.def_id, v.ty.clone()))
    }
}

pub struct NameResolver<'a> {
    file_id: FileId,
    /// This file's own declarations.
    local_ns: &'a ModuleNamespaces,
    /// Imported namespaces (from resolved imports, merged).
    imported_ns: &'a ModuleNamespaces,
    /// Builtin namespaces (string methods, etc.)
    builtin_ns: &'a ModuleNamespaces,
    /// Project-wide type/predicate namespace (last-resort fallback).
    project_ns: &'a ModuleNamespaces,
    /// All files' exported namespaces (for module-qualified type access).
    exported_ns: &'a HashMap<FileId, ModuleNamespaces>,
    scopes: Vec<Scope>,
    next_local: u32,
    /// Whether we're currently inside a class member predicate or characteristic predicate.
    /// Used to suppress "undefined variable" errors for potential inherited fields.
    in_class_context: bool,
    /// Module type parameters currently in scope (from parameterized modules).
    /// Maps type parameter name → synthetic DefId.
    module_type_params: HashMap<String, DefId>,
    /// Nesting depth of parameterized modules (> 0 means inside one).
    /// Used to suppress errors for unresolved predicates/types that may
    /// come from module signature parameters.
    in_parameterized_module: u32,
    pub name_resolutions: Vec<(Span, ResolvedRef)>,
    pub expr_types: Vec<(Span, Type)>,
    pub diagnostics: Vec<Diagnostic>,
}

impl<'a> NameResolver<'a> {
    pub fn new(
        file_id: FileId,
        local_ns: &'a ModuleNamespaces,
        imported_ns: &'a ModuleNamespaces,
        builtin_ns: &'a ModuleNamespaces,
        project_ns: &'a ModuleNamespaces,
        exported_ns: &'a HashMap<FileId, ModuleNamespaces>,
        next_local_id: u32,
    ) -> Self {
        Self {
            file_id,
            local_ns,
            imported_ns,
            builtin_ns,
            project_ns,
            exported_ns,
            scopes: Vec::new(),
            next_local: next_local_id,
            in_class_context: false,
            module_type_params: HashMap::new(),
            in_parameterized_module: 0,
            name_resolutions: Vec::new(),
            expr_types: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Convenience constructor for single-file analysis (backward compat).
    pub fn new_single_file(
        file_id: FileId,
        collector: &'a DeclarationCollector,
        empty1: &'a ModuleNamespaces,
        empty2: &'a ModuleNamespaces,
        empty_project: &'a ModuleNamespaces,
        empty_exported: &'a HashMap<FileId, ModuleNamespaces>,
    ) -> Self {
        Self::new(
            file_id,
            &collector.namespaces,
            empty1,
            empty2,
            empty_project,
            empty_exported,
            collector.next_local_id(),
        )
    }

    // -- scope management --

    fn push_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn fresh_def(&mut self) -> DefId {
        let id = DefId {
            file: self.file_id,
            local: LocalDefId(self.next_local),
        };
        self.next_local += 1;
        id
    }

    fn define_var(&mut self, name: &str, ty: Type, _span: Span) -> DefId {
        let id = DefId {
            file: self.file_id,
            local: LocalDefId(self.next_local),
        };
        self.next_local += 1;
        if let Some(scope) = self.scopes.last_mut() {
            scope.vars.push(ScopeVar {
                name: name.to_string(),
                def_id: id,
                ty,
            });
        }
        id
    }

    fn lookup_var(&self, name: &str) -> Option<(DefId, Type)> {
        for scope in self.scopes.iter().rev() {
            if let Some(found) = scope.find(name) {
                return Some(found);
            }
        }
        None
    }

    /// Look up a type by name: local → imported → builtin → project
    fn lookup_type(&self, name: &str) -> Option<DefId> {
        self.module_type_params
            .get(name)
            .or_else(|| self.local_ns.types.get(name))
            .or_else(|| self.imported_ns.types.get(name))
            .or_else(|| self.builtin_ns.types.get(name))
            .or_else(|| self.project_ns.types.get(name))
            .copied()
    }

    /// If the given type is a Class that ultimately maps to a primitive
    /// (via type aliases or extends-primitive), return the primitive.
    /// Chases through alias chains (e.g., UnboundList → String → string).
    fn resolve_type_alias(&self, ty: &Type) -> Type {
        let mut current = ty.clone();
        for _ in 0..10 {
            if let Type::Class(def_id) = &current {
                if let Some(target) = self.local_ns.type_aliases.get(def_id)
                    .or_else(|| self.imported_ns.type_aliases.get(def_id))
                    .or_else(|| self.project_ns.type_aliases.get(def_id))
                {
                    current = target.clone();
                    continue;
                }
            }
            break;
        }
        current
    }

    /// Look up a predicate by (name, arity): local → imported → builtin → project
    fn lookup_predicate(&self, name: &str, arity: usize) -> Option<&PredicateInfo> {
        let key = (name.to_string(), arity);
        self.local_ns
            .predicates
            .get(&key)
            .or_else(|| self.imported_ns.predicates.get(&key))
            .or_else(|| self.builtin_ns.predicates.get(&key))
            .or_else(|| self.project_ns.predicates.get(&key))
    }

    /// Look up a module by name: local → imported → builtin → project
    fn lookup_module(&self, name: &str) -> Option<DefId> {
        self.local_ns
            .modules
            .get(name)
            .or_else(|| self.imported_ns.modules.get(name))
            .or_else(|| self.builtin_ns.modules.get(name))
            .or_else(|| self.project_ns.modules.get(name))
            .copied()
    }

    fn error(&mut self, span: Span, msg: impl Into<String>) {
        self.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            message: msg.into(),
            span,
            file: self.file_id,
            notes: vec![],
        });
    }

    #[allow(dead_code)]
    fn warning(&mut self, span: Span, msg: impl Into<String>) {
        self.diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            message: msg.into(),
            span,
            file: self.file_id,
            notes: vec![],
        });
    }

    // -- entry points --

    pub fn resolve_source_file(&mut self, sf: &SourceFile) {
        for member in &sf.members {
            self.resolve_member(member);
        }
    }

    fn resolve_member(&mut self, member: &ModuleMember) {
        match member {
            ModuleMember::Predicate(pred) => self.resolve_predicate(pred),
            ModuleMember::Class(class) => self.resolve_class(class),
            ModuleMember::Select(select) => self.resolve_select(select),
            ModuleMember::Module(m) => {
                // Register module type parameters in scope
                let saved_type_params = if !m.type_params.is_empty() {
                    let saved = self.module_type_params.clone();
                    for param in &m.type_params {
                        let param_def = self.fresh_def();
                        self.module_type_params.insert(param.name.name.clone(), param_def);
                    }
                    self.in_parameterized_module += 1;
                    Some(saved)
                } else {
                    None
                };
                for member in &m.members {
                    self.resolve_member(member);
                }
                if let Some(saved) = saved_type_params {
                    self.module_type_params = saved;
                    self.in_parameterized_module -= 1;
                }
            }
            ModuleMember::Newtype(nt) => {
                for branch in &nt.branches {
                    if let Some(body) = &branch.body {
                        self.push_scope();
                        for param in &branch.params {
                            let ty = self.resolve_type_expr(&param.ty);
                            self.define_var(&param.name.name, ty, param.span);
                        }
                        self.resolve_formula(body);
                        self.pop_scope();
                    }
                }
            }
            _ => {} // Import, TypeAlias, etc. handled elsewhere
        }
    }

    // -- predicates --

    fn resolve_predicate(&mut self, pred: &Predicate) {
        self.push_scope();
        for param in &pred.head.params {
            let ty = self.resolve_type_expr(&param.ty);
            self.define_var(&param.name.name, ty, param.span);
        }
        if let Some(result_ty_expr) = &pred.head.result_type {
            let ty = self.resolve_type_expr(result_ty_expr);
            self.define_var("result", ty, pred.head.span);
        }
        if let Some(body) = &pred.body {
            self.resolve_formula(body);
        }
        self.pop_scope();
    }

    // -- classes --

    fn resolve_class(&mut self, class: &ClassDecl) {
        for sup in &class.supertypes {
            self.resolve_type_expr(sup);
        }
        for sup in &class.instanceof {
            self.resolve_type_expr(sup);
        }

        let class_type = self
            .lookup_type(&class.name.name)
            .map(Type::Class)
            .unwrap_or(Type::Error);

        // Pre-resolve all field types so they can be added to member predicate scopes
        let field_info: Vec<(String, Type, Span)> = class
            .members
            .iter()
            .filter_map(|m| {
                if let ClassMember::Field { ty, name, span, .. } = m {
                    let resolved_ty = self.resolve_type_expr(ty);
                    Some((name.name.clone(), resolved_ty, *span))
                } else {
                    None
                }
            })
            .collect();

        for member in &class.members {
            match member {
                ClassMember::CharacteristicPredicate { body, .. } => {
                    self.push_scope();
                    self.in_class_context = true;
                    self.define_var("this", class_type.clone(), class.span);
                    for (fname, fty, fspan) in &field_info {
                        self.define_var(fname, fty.clone(), *fspan);
                    }
                    self.resolve_formula(body);
                    self.in_class_context = false;
                    self.pop_scope();
                }
                ClassMember::MemberPredicate(pred) => {
                    self.push_scope();
                    self.in_class_context = true;
                    self.define_var("this", class_type.clone(), class.span);
                    for (fname, fty, fspan) in &field_info {
                        self.define_var(fname, fty.clone(), *fspan);
                    }
                    for param in &pred.head.params {
                        let ty = self.resolve_type_expr(&param.ty);
                        self.define_var(&param.name.name, ty, param.span);
                    }
                    if let Some(result_ty_expr) = &pred.head.result_type {
                        let ty = self.resolve_type_expr(result_ty_expr);
                        self.define_var("result", ty, pred.head.span);
                    }
                    if let Some(body) = &pred.body {
                        self.resolve_formula(body);
                    }
                    self.in_class_context = false;
                    self.pop_scope();
                }
                ClassMember::Field { .. } => {
                    // Fields already resolved above
                }
            }
        }
    }

    // -- select --

    fn resolve_select(&mut self, select: &Select) {
        self.push_scope();
        for var_decl in &select.from {
            let ty = self.resolve_type_expr(&var_decl.ty);
            let id = self.define_var(&var_decl.name.name, ty, var_decl.span);
            self.name_resolutions
                .push((var_decl.name.span, ResolvedRef::Def(id)));
        }
        if let Some(where_clause) = &select.where_clause {
            self.resolve_formula(where_clause);
        }
        for sel_expr in &select.select_exprs {
            let ty = self.resolve_expr(&sel_expr.expr);
            // `select expr as alias` defines the alias as a variable for ORDER BY
            if let Some(label) = &sel_expr.label {
                self.define_var(label, ty, sel_expr.span);
            }
        }
        for order_item in &select.order_by {
            self.resolve_expr(&order_item.expr);
        }
        self.pop_scope();
    }

    // -- type resolution --

    fn resolve_type_expr(&mut self, te: &TypeExpr) -> Type {
        match &te.kind {
            TypeExprKind::Primitive(p) => Type::Primitive(*p),
            TypeExprKind::Database(name) => Type::DbEntity(name.clone()),
            TypeExprKind::ClassName(upper) => {
                if let Some(def_id) = self.lookup_type(&upper.name) {
                    self.name_resolutions
                        .push((upper.span, ResolvedRef::Def(def_id)));
                    Type::Class(def_id)
                } else {
                    if self.in_parameterized_module == 0 {
                        self.error(upper.span, format!("undefined type `{}`", upper.name));
                    }
                    Type::Error
                }
            }
            TypeExprKind::ModuleAccess(module, ty_name) => {
                // Module-qualified type access (e.g., Module::Type).
                if let Some(mod_def) = self.lookup_module(&module.name) {
                    self.name_resolutions
                        .push((module.span, ResolvedRef::Def(mod_def)));
                    // Look up the type in the module's exported namespace
                    if let Some(mod_ns) = self.exported_ns.get(&mod_def.file) {
                        if let Some(&type_def) = mod_ns.types.get(ty_name.name.as_str()) {
                            self.name_resolutions
                                .push((ty_name.span, ResolvedRef::Def(type_def)));
                            return Type::Class(type_def);
                        }
                    }
                    // Couldn't resolve the type within the module
                    Type::Error
                } else {
                    Type::Error
                }
            }
        }
    }

    // -- formulas --

    fn resolve_formula(&mut self, formula: &Formula) {
        match &formula.kind {
            FormulaKind::Conjunction { lhs, rhs } | FormulaKind::Disjunction { lhs, rhs } => {
                self.resolve_formula(lhs);
                self.resolve_formula(rhs);
            }
            FormulaKind::Negation { inner } => {
                self.resolve_formula(inner);
            }
            FormulaKind::Implies { lhs, rhs } => {
                self.resolve_formula(lhs);
                self.resolve_formula(rhs);
            }
            FormulaKind::IfThenElse { cond, then, else_ } => {
                self.resolve_formula(cond);
                self.resolve_formula(then);
                self.resolve_formula(else_);
            }
            FormulaKind::Comparison { lhs, op, rhs } => {
                let lty = self.resolve_expr(lhs);
                let rty = self.resolve_expr(rhs);
                self.check_comparison_types(&lty, &rty, *op, formula.span);
            }
            FormulaKind::InstanceOf { expr, ty } => {
                self.resolve_expr(expr);
                self.resolve_type_expr(ty);
            }
            FormulaKind::InRange { expr, range } => {
                self.resolve_expr(expr);
                self.resolve_expr(range);
            }
            FormulaKind::Exists { vars, guard, body } => {
                self.push_scope();
                for var in vars {
                    let ty = self.resolve_type_expr(&var.ty);
                    self.define_var(&var.name.name, ty, var.span);
                }
                if let Some(guard) = guard {
                    self.resolve_formula(guard);
                }
                self.resolve_formula(body);
                self.pop_scope();
            }
            FormulaKind::ExistsExpr { expr } => {
                self.resolve_expr(expr);
            }
            FormulaKind::Forall { vars, guard, body }
            | FormulaKind::Forex { vars, guard, body } => {
                self.push_scope();
                for var in vars {
                    let ty = self.resolve_type_expr(&var.ty);
                    self.define_var(&var.name.name, ty, var.span);
                }
                self.resolve_formula(guard);
                self.resolve_formula(body);
                self.pop_scope();
            }
            FormulaKind::PredicateCall { name, args } => {
                self.resolve_predicate_call(&name.name, name.span, args);
            }
            FormulaKind::MemberCall {
                receiver,
                name: _,
                args,
                ..
            } => {
                let _recv_ty = self.resolve_expr(receiver);
                for arg in args {
                    self.resolve_expr(arg);
                }
            }
            FormulaKind::QualifiedCall {
                qualifier: _,
                name: _,
                args,
            } => {
                for arg in args {
                    self.resolve_expr(arg);
                }
                // TODO: resolve qualified calls
            }
            FormulaKind::Any | FormulaKind::None => {}
            FormulaKind::Paren { inner } => {
                self.resolve_formula(inner);
            }
            FormulaKind::ExprFormula(expr) => {
                self.resolve_bridge_expr_formula(expr);
            }
        }
    }

    fn resolve_bridge_expr_formula(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Call { name, args, .. } => {
                self.resolve_predicate_call(&name.name, name.span, args);
            }
            ExprKind::MemberCall {
                receiver,
                name: _,
                args,
                ..
            } => {
                let _recv_ty = self.resolve_expr(receiver);
                for arg in args {
                    self.resolve_expr(arg);
                }
            }
            ExprKind::QualifiedCall { args, .. } => {
                for arg in args {
                    self.resolve_expr(arg);
                }
            }
            ExprKind::Variable(lower) => {
                if let Some((def_id, _ty)) = self.lookup_var(&lower.name) {
                    self.name_resolutions
                        .push((lower.span, ResolvedRef::Def(def_id)));
                } else {
                    self.resolve_predicate_call(&lower.name, lower.span, &[]);
                }
            }
            _ => {
                self.resolve_expr(expr);
            }
        }
    }

    fn resolve_predicate_call(&mut self, name: &str, span: Span, args: &[Expr]) {
        let arity = args.len();
        for arg in args {
            self.resolve_expr(arg);
        }
        if let Some(info) = self.lookup_predicate(name, arity) {
            self.name_resolutions
                .push((span, ResolvedRef::Def(info.def_id)));
        } else if self.in_parameterized_module == 0 {
            self.error(
                span,
                format!("undefined predicate `{name}` with arity {arity}"),
            );
        }
    }

    // -- expressions --

    fn resolve_expr(&mut self, expr: &Expr) -> Type {
        let ty = self.resolve_expr_inner(expr);
        self.expr_types.push((expr.span, ty.clone()));
        ty
    }

    fn resolve_expr_inner(&mut self, expr: &Expr) -> Type {
        match &expr.kind {
            ExprKind::Literal(lit) => match lit {
                Literal::Int(_) => Type::Primitive(PrimitiveType::Int),
                Literal::Float(_) => Type::Primitive(PrimitiveType::Float),
                Literal::String(_) => Type::Primitive(PrimitiveType::String),
                Literal::Bool(_) => Type::Primitive(PrimitiveType::Boolean),
            },

            ExprKind::Variable(lower) => {
                if lower.name == "super" {
                    // `super` resolves to the same type as `this` in class context
                    if let Some((_id, ty)) = self.lookup_var("this") {
                        ty
                    } else {
                        Type::Error
                    }
                } else if let Some((def_id, ty)) = self.lookup_var(&lower.name) {
                    self.name_resolutions
                        .push((lower.span, ResolvedRef::Def(def_id)));
                    ty
                } else if let Some(info) = self.lookup_predicate(&lower.name, 0) {
                    // Zero-arity predicate call (e.g., `result = getField()` shorthand without parens)
                    let info = info.clone();
                    self.name_resolutions
                        .push((lower.span, ResolvedRef::Def(info.def_id)));
                    info.result_type.unwrap_or(Type::Error)
                } else if self.in_class_context {
                    // In a class context, unresolved variables may be inherited fields
                    // from superclasses. Suppress the error.
                    Type::Error
                } else {
                    self.error(lower.span, format!("undefined variable `{}`", lower.name));
                    Type::Error
                }
            }

            ExprKind::This => {
                if let Some((_id, ty)) = self.lookup_var("this") {
                    ty
                } else {
                    self.error(expr.span, "`this` used outside of class context");
                    Type::Error
                }
            }

            ExprKind::Result => {
                if let Some((_id, ty)) = self.lookup_var("result") {
                    ty
                } else {
                    self.error(
                        expr.span,
                        "`result` used outside of a predicate with result type",
                    );
                    Type::Error
                }
            }

            ExprKind::DontCare => Type::Error,

            ExprKind::BinaryOp { lhs, op, rhs } => {
                let lt = self.resolve_expr(lhs);
                let rt = self.resolve_expr(rhs);
                self.check_binary_op(&lt, &rt, *op, expr.span)
            }

            ExprKind::UnaryOp { op: _, operand } => {
                let t = self.resolve_expr(operand);
                let resolved_t = self.resolve_type_alias(&t);
                if !resolved_t.is_numeric() && resolved_t != Type::Error {
                    self.error(
                        expr.span,
                        format!("unary operator requires numeric type, got `{t}`"),
                    );
                }
                t
            }

            ExprKind::Call { name, args, .. } => {
                let arity = args.len();
                for arg in args {
                    self.resolve_expr(arg);
                }
                if let Some(info) = self.lookup_predicate(&name.name, arity) {
                    let info = info.clone();
                    self.name_resolutions
                        .push((name.span, ResolvedRef::Def(info.def_id)));
                    info.result_type.unwrap_or_else(|| {
                        self.error(
                            name.span,
                            format!(
                                "predicate `{}` has no result type; cannot use in expression context",
                                name.name
                            ),
                        );
                        Type::Error
                    })
                } else {
                    if self.in_parameterized_module == 0 {
                        self.error(
                            name.span,
                            format!("undefined predicate `{}` with arity {arity}", name.name),
                        );
                    }
                    Type::Error
                }
            }

            ExprKind::MemberCall {
                receiver,
                name: _,
                args,
                ..
            } => {
                let _recv_ty = self.resolve_expr(receiver);
                for arg in args {
                    self.resolve_expr(arg);
                }
                // TODO: member predicate resolution
                Type::Error
            }

            ExprKind::QualifiedCall { args, .. } => {
                for arg in args {
                    self.resolve_expr(arg);
                }
                Type::Error
            }

            ExprKind::TypeCall { args, .. } => {
                for arg in args {
                    self.resolve_expr(arg);
                }
                Type::Error
            }

            ExprKind::PostfixCast { expr: inner, ty }
            | ExprKind::PrefixCast { ty, expr: inner } => {
                self.resolve_expr(inner);
                self.resolve_type_expr(ty)
            }

            ExprKind::Range { low, high } => {
                let lt = self.resolve_expr(low);
                let ht = self.resolve_expr(high);
                if lt != ht && lt != Type::Error && ht != Type::Error {
                    self.error(
                        expr.span,
                        format!("range bounds have different types: `{lt}` vs `{ht}`"),
                    );
                }
                lt
            }

            ExprKind::SetLiteral { elements } => {
                let mut result_ty = Type::Error;
                for elem in elements {
                    let t = self.resolve_expr(elem);
                    if result_ty == Type::Error {
                        result_ty = t;
                    }
                }
                result_ty
            }

            ExprKind::Aggregation {
                vars,
                guard,
                expr: agg_expr,
                separator,
                order_by,
                ..
            } => {
                self.push_scope();
                for var in vars {
                    let ty = self.resolve_type_expr(&var.ty);
                    self.define_var(&var.name.name, ty, var.span);
                }
                if let Some(guard) = guard {
                    self.resolve_formula(guard);
                }
                let expr_ty = if let Some(e) = agg_expr {
                    self.resolve_expr(e)
                } else {
                    Type::Error
                };
                if let Some(sep) = separator {
                    self.resolve_expr(sep);
                }
                for item in order_by {
                    self.resolve_expr(&item.expr);
                }
                self.pop_scope();
                self.aggregation_result_type(expr, &expr_ty)
            }

            ExprKind::RankExpr {
                index,
                vars,
                guard,
                expr: rank_expr,
                order_by,
            } => {
                self.resolve_expr(index);
                self.push_scope();
                for var in vars {
                    let ty = self.resolve_type_expr(&var.ty);
                    self.define_var(&var.name.name, ty, var.span);
                }
                if let Some(guard) = guard {
                    self.resolve_formula(guard);
                }
                let t = self.resolve_expr(rank_expr);
                for item in order_by {
                    self.resolve_expr(&item.expr);
                }
                self.pop_scope();
                t
            }

            ExprKind::AnyExpr {
                vars,
                guard,
                expr: any_expr,
            } => {
                self.push_scope();
                let mut var_ty = Type::Error;
                for var in vars {
                    let ty = self.resolve_type_expr(&var.ty);
                    var_ty = ty.clone();
                    self.define_var(&var.name.name, ty, var.span);
                }
                if let Some(guard) = guard {
                    self.resolve_formula(guard);
                }
                let t = if let Some(e) = any_expr {
                    self.resolve_expr(e)
                } else {
                    var_ty
                };
                self.pop_scope();
                t
            }

            ExprKind::Super { args, .. } => {
                for arg in args {
                    self.resolve_expr(arg);
                }
                Type::Error
            }

            ExprKind::Paren(inner) => self.resolve_expr(inner),

            ExprKind::FormulaExpr(formula) => {
                self.resolve_formula(formula);
                Type::Primitive(PrimitiveType::Boolean)
            }
        }
    }

    // -- type checking helpers --

    fn check_binary_op(&mut self, lt: &Type, rt: &Type, op: BinOp, span: Span) -> Type {
        // Resolve type aliases (e.g., `class IntValue = int` → int)
        let lt = &self.resolve_type_alias(lt);
        let rt = &self.resolve_type_alias(rt);
        match op {
            BinOp::Add => {
                if lt == &Type::Primitive(PrimitiveType::String)
                    || rt == &Type::Primitive(PrimitiveType::String)
                {
                    Type::Primitive(PrimitiveType::String)
                } else if lt.is_numeric() && rt.is_numeric() {
                    lt.numeric_result(rt)
                } else if *lt == Type::Error || *rt == Type::Error {
                    Type::Error
                } else {
                    self.error(
                        span,
                        format!("operator `+` not defined for `{lt}` and `{rt}`"),
                    );
                    Type::Error
                }
            }
            BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                if lt.is_numeric() && rt.is_numeric() {
                    lt.numeric_result(rt)
                } else if *lt == Type::Error || *rt == Type::Error {
                    Type::Error
                } else {
                    self.error(
                        span,
                        format!("arithmetic operator not defined for `{lt}` and `{rt}`"),
                    );
                    Type::Error
                }
            }
        }
    }

    fn check_comparison_types(&mut self, lt: &Type, rt: &Type, _op: CompOp, span: Span) {
        if *lt == Type::Error || *rt == Type::Error {
            return;
        }
        if let (Type::Primitive(lp), Type::Primitive(rp)) = (lt, rt) {
            let compatible = matches!(
                (lp, rp),
                (PrimitiveType::Int, PrimitiveType::Float)
                    | (PrimitiveType::Float, PrimitiveType::Int)
                    | (PrimitiveType::Int, PrimitiveType::Int)
                    | (PrimitiveType::Float, PrimitiveType::Float)
                    | (PrimitiveType::String, PrimitiveType::String)
                    | (PrimitiveType::Boolean, PrimitiveType::Boolean)
                    | (PrimitiveType::Date, PrimitiveType::Date)
            );
            if !compatible {
                self.error(span, format!("cannot compare `{lt}` with `{rt}`"));
            }
        }
    }

    fn aggregation_result_type(&self, expr: &Expr, expr_ty: &Type) -> Type {
        use ocql_ql_ast::AggKind;
        if let ExprKind::Aggregation { kind, .. } = &expr.kind {
            match kind {
                AggKind::Count | AggKind::StrictCount | AggKind::Rank => {
                    Type::Primitive(PrimitiveType::Int)
                }
                AggKind::Sum | AggKind::StrictSum => {
                    if expr_ty.is_numeric() {
                        expr_ty.clone()
                    } else {
                        Type::Primitive(PrimitiveType::Int)
                    }
                }
                AggKind::Avg => Type::Primitive(PrimitiveType::Float),
                AggKind::Min | AggKind::Max | AggKind::Unique | AggKind::Any => expr_ty.clone(),
                AggKind::Concat | AggKind::StrictConcat => {
                    Type::Primitive(PrimitiveType::String)
                }
            }
        } else {
            Type::Error
        }
    }
}
