use ocql_common::Span;
use ocql_ql_ast::expr::{Expr, ExprKind};
use ocql_ql_ast::formula::{Formula, FormulaKind};
use ocql_ql_ast::module::{ClassDecl, ClassMember, ModuleMember, SourceFile};
use ocql_ql_ast::predicate::Predicate;
use ocql_ql_ast::query::Select;
use ocql_ql_ast::ty::{PrimitiveType, TypeExpr, TypeExprKind};
use ocql_ql_ast::{BinOp, CompOp, Literal};

use crate::def::{DefId, DefKind, FileId, LocalDefId};
use crate::diagnostics::{Diagnostic, Severity};
use crate::namespace::{ModuleNamespaces, PredicateInfo};
use crate::types::Type;
use crate::DefInfo;

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
// Declaration collection (Phase 3)
// ---------------------------------------------------------------------------

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
            ModuleMember::Select(_) => {} // selects don't declare names
            ModuleMember::Module(m) => {
                let _id = self.alloc_def(DefKind::Module, &m.name.name, m.span);
                // Milestone 3: recurse into module members
            }
            ModuleMember::Newtype(nt) => {
                let _id = self.alloc_def(DefKind::Newtype, &nt.name.name, nt.span);
                self.namespaces.types.insert(nt.name.name.clone(), _id);
            }
            ModuleMember::TypeAlias(ta) => {
                let id = self.alloc_def(DefKind::TypeAlias, &ta.name.name, ta.span);
                self.namespaces.types.insert(ta.name.name.clone(), id);
            }
            ModuleMember::ModuleAlias(ma) => {
                let _id = self.alloc_def(DefKind::ModuleAlias, &ma.name.name, ma.span);
            }
            ModuleMember::PredicateAlias(pa) => {
                let _id = self.alloc_def(DefKind::PredicateAlias, &pa.name.name, pa.span);
            }
            ModuleMember::Import(_) => {} // Milestone 3
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

        // Collect class members into namespace for member resolution
        for member in &class.members {
            match member {
                ClassMember::CharacteristicPredicate { name, span, .. } => {
                    self.alloc_def(DefKind::CharPredicate, &name.name, *span);
                }
                ClassMember::MemberPredicate(pred) => {
                    let pred_name = &pred.head.name.name;
                    let arity = pred.head.params.len();
                    let id =
                        self.alloc_def(DefKind::MemberPredicate, pred_name, pred.span);

                    // Register as member predicate on the class.
                    // For milestone 1 we use a simplified scheme: we register
                    // class member predicates in the global namespace with a
                    // class-qualified key. Full member resolution comes in M2.
                    let result_type = pred
                        .head
                        .result_type
                        .as_ref()
                        .map(|te| resolve_type_expr_simple(te));

                    // Store with class-qualified name for member lookup
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
fn resolve_type_expr_simple(te: &TypeExpr) -> Type {
    match &te.kind {
        TypeExprKind::Primitive(p) => Type::Primitive(*p),
        TypeExprKind::Database(name) => Type::DbEntity(name.clone()),
        TypeExprKind::ClassName(_) => Type::Error, // needs full resolution
        TypeExprKind::ModuleAccess(_, _) => Type::Error, // needs full resolution
    }
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
    collector: &'a DeclarationCollector,
    scopes: Vec<Scope>,
    next_local: u32,
    pub name_resolutions: Vec<(Span, ResolvedRef)>,
    pub expr_types: Vec<(Span, Type)>,
    pub diagnostics: Vec<Diagnostic>,
}

impl<'a> NameResolver<'a> {
    pub fn new(file_id: FileId, collector: &'a DeclarationCollector) -> Self {
        Self {
            file_id,
            collector,
            scopes: Vec::new(),
            next_local: collector.defs.len() as u32,
            name_resolutions: Vec::new(),
            expr_types: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    // -- scope management --

    fn push_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
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

    fn error(&mut self, span: Span, msg: impl Into<String>) {
        self.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            message: msg.into(),
            span,
            file: self.file_id,
            notes: vec![],
        });
    }

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
            _ => {} // Other members handled in later milestones
        }
    }

    // -- predicates --

    fn resolve_predicate(&mut self, pred: &Predicate) {
        self.push_scope();

        // Bind parameters
        for param in &pred.head.params {
            let ty = self.resolve_type_expr(&param.ty);
            self.define_var(&param.name.name, ty, param.span);
        }

        // Bind `result` if predicate has a result type
        if let Some(result_ty_expr) = &pred.head.result_type {
            let ty = self.resolve_type_expr(result_ty_expr);
            self.define_var("result", ty, pred.head.span);
        }

        // Resolve body
        if let Some(body) = &pred.body {
            self.resolve_formula(body);
        }

        self.pop_scope();
    }

    // -- classes (Milestone 2: basic support) --

    fn resolve_class(&mut self, class: &ClassDecl) {
        // Resolve supertypes
        for sup in &class.supertypes {
            self.resolve_type_expr(sup);
        }

        // Resolve class members
        for member in &class.members {
            match member {
                ClassMember::CharacteristicPredicate { body, .. } => {
                    self.push_scope();
                    // `this` is in scope with the class type
                    let class_type = if let Some(&class_def) =
                        self.collector.namespaces.types.get(&class.name.name)
                    {
                        Type::Class(class_def)
                    } else {
                        Type::Error
                    };
                    self.define_var("this", class_type, class.span);
                    self.resolve_formula(body);
                    self.pop_scope();
                }
                ClassMember::MemberPredicate(pred) => {
                    self.push_scope();
                    // `this` is in scope
                    let class_type = if let Some(&class_def) =
                        self.collector.namespaces.types.get(&class.name.name)
                    {
                        Type::Class(class_def)
                    } else {
                        Type::Error
                    };
                    self.define_var("this", class_type, class.span);

                    // Bind parameters
                    for param in &pred.head.params {
                        let ty = self.resolve_type_expr(&param.ty);
                        self.define_var(&param.name.name, ty, param.span);
                    }

                    // Bind `result`
                    if let Some(result_ty_expr) = &pred.head.result_type {
                        let ty = self.resolve_type_expr(result_ty_expr);
                        self.define_var("result", ty, pred.head.span);
                    }

                    if let Some(body) = &pred.body {
                        self.resolve_formula(body);
                    }
                    self.pop_scope();
                }
                ClassMember::Field { ty, .. } => {
                    self.resolve_type_expr(ty);
                }
            }
        }
    }

    // -- select --

    fn resolve_select(&mut self, select: &Select) {
        self.push_scope();

        // Bind `from` variables
        for var_decl in &select.from {
            let ty = self.resolve_type_expr(&var_decl.ty);
            let id = self.define_var(&var_decl.name.name, ty.clone(), var_decl.span);
            self.name_resolutions
                .push((var_decl.name.span, ResolvedRef::Def(id)));
        }

        // Resolve `where` clause
        if let Some(where_clause) = &select.where_clause {
            self.resolve_formula(where_clause);
        }

        // Resolve `select` expressions
        for sel_expr in &select.select_exprs {
            self.resolve_expr(&sel_expr.expr);
        }

        // Resolve `order by`
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
                // Look up in type namespace
                if let Some(&def_id) = self.collector.namespaces.types.get(&upper.name) {
                    self.name_resolutions
                        .push((upper.span, ResolvedRef::Def(def_id)));
                    Type::Class(def_id)
                } else {
                    self.error(upper.span, format!("undefined type `{}`", upper.name));
                    Type::Error
                }
            }
            TypeExprKind::ModuleAccess(module, ty) => {
                // Milestone 3: module-qualified type access
                self.warning(
                    te.span,
                    format!(
                        "module-qualified type `{}::{}` not yet supported",
                        module.name, ty.name
                    ),
                );
                Type::Error
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
            FormulaKind::IfThenElse {
                cond,
                then,
                else_,
            } => {
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
                // Milestone 2: full member predicate resolution
                for arg in args {
                    self.resolve_expr(arg);
                }
            }
            FormulaKind::QualifiedCall {
                qualifier,
                name,
                args,
            } => {
                // Milestone 3: module-qualified calls
                self.warning(
                    formula.span,
                    format!(
                        "qualified call `{}::{}` not yet fully resolved",
                        qualifier.name, name.name
                    ),
                );
                for arg in args {
                    self.resolve_expr(arg);
                }
            }
            FormulaKind::Any | FormulaKind::None => {}
            FormulaKind::Paren { inner } => {
                self.resolve_formula(inner);
            }
            FormulaKind::ExprFormula(expr) => {
                // Bridge node: an expression in formula context.
                // Usually a call that should be resolved as a predicate call.
                self.resolve_bridge_expr_formula(expr);
            }
        }
    }

    /// Resolve a bridge ExprFormula node. The parser produced an Expr where a
    /// Formula was expected — typically a predicate call.
    fn resolve_bridge_expr_formula(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Call {
                name, args, ..
            } => {
                // Check if this is a predicate call (no result) in formula context
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
                // Milestone 2: resolve member predicate
            }
            ExprKind::QualifiedCall {
                qualifier,
                name,
                args,
            } => {
                self.warning(
                    expr.span,
                    format!(
                        "qualified call `{}::{}` not yet fully resolved",
                        qualifier.name, name.name
                    ),
                );
                for arg in args {
                    self.resolve_expr(arg);
                }
            }
            ExprKind::Variable(lower) => {
                // A bare variable in formula context — resolve it
                if let Some((def_id, _ty)) = self.lookup_var(&lower.name) {
                    self.name_resolutions
                        .push((lower.span, ResolvedRef::Def(def_id)));
                } else {
                    // Could be a zero-arity predicate
                    self.resolve_predicate_call(&lower.name, lower.span, &[]);
                }
            }
            _ => {
                // General expression in formula context — just type-check it
                self.resolve_expr(expr);
            }
        }
    }

    fn resolve_predicate_call(&mut self, name: &str, span: Span, args: &[Expr]) {
        let arity = args.len();

        // Resolve arguments first
        for arg in args {
            self.resolve_expr(arg);
        }

        // Look up predicate by (name, arity)
        if let Some(info) = self
            .collector
            .namespaces
            .predicates
            .get(&(name.to_string(), arity))
        {
            self.name_resolutions
                .push((span, ResolvedRef::Def(info.def_id)));
        } else {
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
                if let Some((def_id, ty)) = self.lookup_var(&lower.name) {
                    self.name_resolutions
                        .push((lower.span, ResolvedRef::Def(def_id)));
                    ty
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

            ExprKind::DontCare => {
                // _ has no type constraint
                Type::Error
            }

            ExprKind::BinaryOp { lhs, op, rhs } => {
                let lt = self.resolve_expr(lhs);
                let rt = self.resolve_expr(rhs);
                self.check_binary_op(&lt, &rt, *op, expr.span)
            }

            ExprKind::UnaryOp { op: _, operand } => {
                let t = self.resolve_expr(operand);
                if !t.is_numeric() && t != Type::Error {
                    self.error(expr.span, format!("unary operator requires numeric type, got `{t}`"));
                }
                t
            }

            ExprKind::Call {
                name, args, ..
            } => {
                let arity = args.len();
                for arg in args {
                    self.resolve_expr(arg);
                }

                if let Some(info) = self
                    .collector
                    .namespaces
                    .predicates
                    .get(&(name.name.clone(), arity))
                {
                    self.name_resolutions
                        .push((name.span, ResolvedRef::Def(info.def_id)));
                    info.result_type.clone().unwrap_or_else(|| {
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
                    self.error(
                        name.span,
                        format!("undefined predicate `{}` with arity {arity}", name.name),
                    );
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
                // Milestone 2: resolve member predicate and return its type
                Type::Error
            }

            ExprKind::QualifiedCall {
                qualifier,
                name,
                args,
            } => {
                for arg in args {
                    self.resolve_expr(arg);
                }
                self.warning(
                    expr.span,
                    format!(
                        "qualified call `{}::{}` not yet fully resolved",
                        qualifier.name, name.name
                    ),
                );
                Type::Error
            }

            ExprKind::TypeCall { args, .. } => {
                for arg in args {
                    self.resolve_expr(arg);
                }
                // Milestone 2: resolve type call
                Type::Error
            }

            ExprKind::PostfixCast { expr: inner, ty } | ExprKind::PrefixCast { ty, expr: inner } => {
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
                kind: _,
                vars,
                guard,
                expr: agg_expr,
                separator,
                order_by,
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

                // Aggregation result type depends on the kind
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

            ExprKind::Super {
                super_type: _,
                name: _,
                args,
            } => {
                for arg in args {
                    self.resolve_expr(arg);
                }
                // Milestone 2: full super resolution
                Type::Error
            }

            ExprKind::Paren(inner) => self.resolve_expr(inner),

            ExprKind::FormulaExpr(formula) => {
                // A formula in expression context (e.g., inside aggregation guard).
                self.resolve_formula(formula);
                Type::Primitive(PrimitiveType::Boolean)
            }
        }
    }

    // -- type checking helpers --

    fn check_binary_op(&mut self, lt: &Type, rt: &Type, op: BinOp, span: Span) -> Type {
        match op {
            BinOp::Add => {
                // + works on numbers and strings
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
        // Basic compatibility check: both should be in the same universe.
        // For milestone 1, just check that primitives match category.
        if let (Type::Primitive(lp), Type::Primitive(rp)) = (lt, rt) {
            let compatible = match (lp, rp) {
                (PrimitiveType::Int, PrimitiveType::Float)
                | (PrimitiveType::Float, PrimitiveType::Int)
                | (PrimitiveType::Int, PrimitiveType::Int)
                | (PrimitiveType::Float, PrimitiveType::Float)
                | (PrimitiveType::String, PrimitiveType::String)
                | (PrimitiveType::Boolean, PrimitiveType::Boolean)
                | (PrimitiveType::Date, PrimitiveType::Date) => true,
                _ => false,
            };
            if !compatible {
                self.error(
                    span,
                    format!("cannot compare `{lt}` with `{rt}`"),
                );
            }
        }
        // Non-primitive type compatibility checked in later milestones
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

