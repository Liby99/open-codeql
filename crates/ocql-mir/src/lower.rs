//! Lower QL AST to MIR.
//!
//! Transforms QL predicates, classes, and select queries into flat MIR
//! predicates that can then be emitted to engine rules.
//!
//! ## Lowering stages implemented here:
//!
//! 1. **Predicate lowering** — QL predicates → MIR predicates
//! 2. **Class elimination** — Classes → characteristic + member predicates
//! 3. **Formula lowering** — Conjunction, disjunction, negation, exists, etc.
//! 4. **Expression lowering** — Literals, arithmetic, calls, etc.
//! 5. **Select lowering** — from/where/select → result predicate

use ocql_ql_ast::expr::{Expr, ExprKind, VarDecl};
use ocql_ql_ast::formula::{Formula, FormulaKind};
use ocql_ql_ast::module::{ClassDecl, ClassMember, ModuleMember, SourceFile};
use ocql_ql_ast::predicate::Predicate;
use ocql_ql_ast::query::Select;
use ocql_ql_ast::ty::TypeExprKind;
use ocql_ql_ast::{BinOp, Literal};

use crate::nodes::*;

// ============================================================
// Error type
// ============================================================

/// Errors that can occur during MIR lowering.
#[derive(Debug)]
pub enum LowerError {
    /// A QL construct that is not yet supported.
    Unsupported(String),
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LowerError::Unsupported(msg) => write!(f, "unsupported: {}", msg),
        }
    }
}

impl std::error::Error for LowerError {}

// ============================================================
// Context
// ============================================================

/// Context for lowering, tracks fresh variable/predicate counters.
pub struct LowerCtx {
    fresh_var: u32,
    fresh_pred: u32,
    /// Accumulated predicates.
    predicates: Vec<MirPredicate>,
}

impl LowerCtx {
    pub fn new() -> Self {
        Self {
            fresh_var: 0,
            fresh_pred: 0,
            predicates: Vec::new(),
        }
    }

    fn fresh_var(&mut self) -> String {
        let v = format!("_t{}", self.fresh_var);
        self.fresh_var += 1;
        v
    }

    fn fresh_pred(&mut self, prefix: &str) -> String {
        let p = format!("{}_{}", prefix, self.fresh_pred);
        self.fresh_pred += 1;
        p
    }

    fn emit_predicate(&mut self, pred: MirPredicate) {
        self.predicates.push(pred);
    }

    pub fn into_program(self) -> MirProgram {
        MirProgram {
            predicates: self.predicates,
        }
    }
}

// ============================================================
// Public API
// ============================================================

/// Lower an entire QL source file to a MIR program.
pub fn lower_source_file(file: &SourceFile) -> Result<MirProgram, LowerError> {
    let mut ctx = LowerCtx::new();

    for member in &file.members {
        lower_member(&mut ctx, member)?;
    }

    Ok(ctx.into_program())
}

fn lower_member(ctx: &mut LowerCtx, member: &ModuleMember) -> Result<(), LowerError> {
    match member {
        ModuleMember::Predicate(pred) => lower_predicate(ctx, pred, None),
        ModuleMember::Class(class) => lower_class(ctx, class),
        ModuleMember::Select(select) => lower_select(ctx, select),
        ModuleMember::Import(_) => Ok(()),
        ModuleMember::Module(_) => Ok(()), // TODO: nested modules
        ModuleMember::Newtype(_) => Ok(()), // TODO: newtype lowering
        ModuleMember::ModuleAlias(_) | ModuleMember::TypeAlias(_) | ModuleMember::PredicateAlias(_) => Ok(()),
        ModuleMember::Signature(_) => Ok(()),
    }
}

// ============================================================
// Predicate lowering
// ============================================================

fn lower_predicate(
    ctx: &mut LowerCtx,
    pred: &Predicate,
    class_name: Option<&str>,
) -> Result<(), LowerError> {
    let pred_name = match class_name {
        Some(cls) => format!("{}#{}", cls, pred.head.name.name),
        None => pred.head.name.name.clone(),
    };

    // Build parameter list
    let mut params: Vec<MirParam> = Vec::new();

    // Member predicates have implicit `this` as first parameter
    if class_name.is_some() {
        params.push(MirParam::new("this", MirType::Any));
    }

    for p in &pred.head.params {
        let ty = lower_type_expr_kind(&p.ty.kind);
        params.push(MirParam::new(&p.name.name, ty));
    }

    // If the predicate has a result type, add `result` as the last parameter
    if let Some(ref rt) = pred.head.result_type {
        let ty = lower_type_expr_kind(&rt.kind);
        params.push(MirParam::new("result", ty));
    }

    if let Some(body_formula) = &pred.body {
        let mut atoms = Vec::new();

        // For member predicates, add characteristic predicate constraint on `this`
        if let Some(cls) = class_name {
            atoms.push(MirAtom::Scan(MirScan::new(
                &format!("{}#char", cls),
                vec![MirTerm::var("this")],
            )));
        }

        lower_formula(ctx, body_formula, &mut atoms, &pred_name)?;

        ctx.emit_predicate(MirPredicate::new(&pred_name, params, atoms));
    } else {
        // No body — abstract or external predicate
        ctx.emit_predicate(MirPredicate::abstract_pred(&pred_name, params));
    }

    Ok(())
}

// ============================================================
// Class lowering
// ============================================================

fn lower_class(ctx: &mut LowerCtx, class: &ClassDecl) -> Result<(), LowerError> {
    let class_name = &class.name.name;

    for member in &class.members {
        match member {
            ClassMember::CharacteristicPredicate { body, .. } => {
                let mut atoms = Vec::new();

                // Add supertype constraint
                for sup in &class.supertypes {
                    if let TypeExprKind::ClassName(name) = &sup.kind {
                        atoms.push(MirAtom::Scan(MirScan::new(
                            &format!("{}#char", name.name),
                            vec![MirTerm::var("this")],
                        )));
                    }
                    // Primitives don't have char predicates
                }

                lower_formula(ctx, body, &mut atoms, &format!("{}#char", class_name))?;

                ctx.emit_predicate(MirPredicate::new(
                    &format!("{}#char", class_name),
                    vec![MirParam::new("this", MirType::Any)],
                    atoms,
                ));
            }
            ClassMember::MemberPredicate(pred) => {
                lower_predicate(ctx, pred, Some(class_name))?;
            }
            ClassMember::Field { .. } => {
                // Fields become relations — handled at a higher level
            }
        }
    }

    Ok(())
}

// ============================================================
// Select lowering
// ============================================================

fn lower_select(ctx: &mut LowerCtx, select: &Select) -> Result<(), LowerError> {
    let select_name = ctx.fresh_pred("select_result");

    // Build params from `from` clause variables
    let mut params: Vec<MirParam> = Vec::new();
    for var in &select.from {
        let ty = lower_type_expr_kind(&var.ty.kind);
        params.push(MirParam::new(&var.name.name, ty));
    }

    // Add params for each select expression (as _sel0, _sel1, ...)
    let mut select_vars = Vec::new();
    for (i, _) in select.select_exprs.iter().enumerate() {
        let var_name = format!("_sel{}", i);
        params.push(MirParam::new(&var_name, MirType::Any));
        select_vars.push(var_name);
    }

    let mut atoms = Vec::new();

    // Lower `where` clause
    if let Some(ref where_clause) = select.where_clause {
        lower_formula(ctx, where_clause, &mut atoms, &select_name)?;
    }

    // Lower select expressions
    for (i, sel_expr) in select.select_exprs.iter().enumerate() {
        let (term, extra) = lower_expr(ctx, &sel_expr.expr, &select_name)?;
        atoms.extend(extra);

        // Bind select variable to the expression result
        match term {
            MirTerm::Var(ref v) if *v == select_vars[i] => {
                // Already bound to the right variable — skip
            }
            _ => {
                atoms.push(MirAtom::Guard(MirGuard {
                    left: MirTerm::var(&select_vars[i]),
                    op: MirCompOp::Eq,
                    right: term,
                }));
            }
        }
    }

    ctx.emit_predicate(MirPredicate::new(&select_name, params, atoms));
    Ok(())
}

// ============================================================
// Formula lowering
// ============================================================

fn lower_formula(
    ctx: &mut LowerCtx,
    formula: &Formula,
    body: &mut Vec<MirAtom>,
    parent_pred: &str,
) -> Result<(), LowerError> {
    match &formula.kind {
        FormulaKind::Conjunction { lhs, rhs } => {
            lower_formula(ctx, lhs, body, parent_pred)?;
            lower_formula(ctx, rhs, body, parent_pred)?;
        }

        FormulaKind::Disjunction { lhs, rhs } => {
            lower_disjunction(ctx, lhs, rhs, body, parent_pred)?;
        }

        FormulaKind::Negation { inner } => {
            lower_negation(ctx, inner, body, parent_pred)?;
        }

        FormulaKind::Comparison { lhs, op, rhs } => {
            lower_comparison(ctx, lhs, *op, rhs, body, parent_pred)?;
        }

        FormulaKind::InstanceOf { expr, ty } => {
            let (term, extra) = lower_expr(ctx, expr, parent_pred)?;
            body.extend(extra);
            if let MirTerm::Var(var_name) = term {
                if let TypeExprKind::ClassName(name) = &ty.kind {
                    body.push(MirAtom::TypeCheck(MirTypeCheck {
                        var: var_name,
                        type_predicate: format!("{}#char", name.name),
                    }));
                }
            }
        }

        FormulaKind::Exists { vars, guard, body: exists_body } => {
            lower_exists(ctx, vars, guard.as_deref(), exists_body, body, parent_pred)?;
        }

        FormulaKind::ExistsExpr { expr } => {
            // exists(expr) — just evaluate the expression (it must have results)
            let (_term, extra) = lower_expr(ctx, expr, parent_pred)?;
            body.extend(extra);
        }

        FormulaKind::Forall { vars, guard, body: forall_body } => {
            // forall(vars | guard | body) ≡ not exists(vars | guard | not body)
            lower_forall(ctx, vars, guard, forall_body, body, parent_pred)?;
        }

        FormulaKind::Forex { vars, guard, body: forex_body } => {
            // forex(vars | guard | body) ≡ exists(vars | guard) and forall(vars | guard | body)
            lower_exists(ctx, vars, Some(guard), &Formula {
                kind: FormulaKind::Any,
                span: formula.span,
            }, body, parent_pred)?;
            lower_forall(ctx, vars, guard, forex_body, body, parent_pred)?;
        }

        FormulaKind::Implies { lhs, rhs } => {
            // A implies B ≡ not A or B
            let neg_lhs = Formula {
                kind: FormulaKind::Negation { inner: lhs.clone() },
                span: formula.span,
            };
            let disj = Formula {
                kind: FormulaKind::Disjunction {
                    lhs: Box::new(neg_lhs),
                    rhs: rhs.clone(),
                },
                span: formula.span,
            };
            lower_formula(ctx, &disj, body, parent_pred)?;
        }

        FormulaKind::IfThenElse { cond, then, else_ } => {
            lower_if_then_else(ctx, cond, then, else_, body, parent_pred)?;
        }

        FormulaKind::PredicateCall { name, args } => {
            let mut terms = Vec::new();
            for arg in args {
                let (term, extra) = lower_expr(ctx, arg, parent_pred)?;
                body.extend(extra);
                terms.push(term);
            }
            body.push(MirAtom::Scan(MirScan::new(&name.name, terms)));
        }

        FormulaKind::MemberCall { receiver, name, closure: _, args } => {
            let (recv_term, extra) = lower_expr(ctx, receiver, parent_pred)?;
            body.extend(extra);
            let mut terms = vec![recv_term];
            for arg in args {
                let (term, extra) = lower_expr(ctx, arg, parent_pred)?;
                body.extend(extra);
                terms.push(term);
            }
            // For member calls, we use the name directly (type resolution happens at HIR)
            body.push(MirAtom::Scan(MirScan::new(&name.name, terms)));
        }

        FormulaKind::QualifiedCall { qualifier, name, args } => {
            let pred_name = format!("{}#{}", qualifier.name, name.name);
            let mut terms = Vec::new();
            for arg in args {
                let (term, extra) = lower_expr(ctx, arg, parent_pred)?;
                body.extend(extra);
                terms.push(term);
            }
            body.push(MirAtom::Scan(MirScan::new(&pred_name, terms)));
        }

        FormulaKind::Any => {
            // any() — always true, no constraint
        }

        FormulaKind::None => {
            // none() — always false. We can't represent this in a conjunction easily;
            // mark by adding an impossible guard
            body.push(MirAtom::Guard(MirGuard {
                left: MirTerm::int(0),
                op: MirCompOp::Ne,
                right: MirTerm::int(0),
            }));
        }

        FormulaKind::Paren { inner } => {
            lower_formula(ctx, inner, body, parent_pred)?;
        }

        FormulaKind::ExprFormula(expr) => {
            // Bridge node: expression used as formula
            match &expr.kind {
                ExprKind::FormulaExpr(inner_formula) => {
                    // Double bridge: unwrap
                    lower_formula(ctx, inner_formula, body, parent_pred)?;
                }
                ExprKind::Call { name, closure: _, args } => {
                    // Bare call in formula context: no result variable
                    let mut terms = Vec::new();
                    for arg in args {
                        let (term, extra) = lower_expr(ctx, arg, parent_pred)?;
                        body.extend(extra);
                        terms.push(term);
                    }
                    body.push(MirAtom::Scan(MirScan::new(&name.name, terms)));
                }
                ExprKind::MemberCall { receiver, name, closure: _, args } => {
                    // Member call in formula context: no result variable
                    let (recv_term, extra) = lower_expr(ctx, receiver, parent_pred)?;
                    body.extend(extra);
                    let mut terms = vec![recv_term];
                    for arg in args {
                        let (term, extra) = lower_expr(ctx, arg, parent_pred)?;
                        body.extend(extra);
                        terms.push(term);
                    }
                    body.push(MirAtom::Scan(MirScan::new(&name.name, terms)));
                }
                ExprKind::QualifiedCall { qualifier, name, args } => {
                    // Qualified call in formula context: no result variable
                    let pred_name = format!("{}#{}", qualifier.name, name.name);
                    let mut terms = Vec::new();
                    for arg in args {
                        let (term, extra) = lower_expr(ctx, arg, parent_pred)?;
                        body.extend(extra);
                        terms.push(term);
                    }
                    body.push(MirAtom::Scan(MirScan::new(&pred_name, terms)));
                }
                _ => {
                    // General expression in formula context
                    let (_term, extra) = lower_expr(ctx, expr, parent_pred)?;
                    body.extend(extra);
                }
            }
        }

        FormulaKind::InRange { expr, range } => {
            let (term, extra) = lower_expr(ctx, expr, parent_pred)?;
            body.extend(extra);
            // Range expression generates >= low and <= high
            if let ExprKind::Range { low, high } = &range.kind {
                let (lo_term, extra) = lower_expr(ctx, low, parent_pred)?;
                body.extend(extra);
                let (hi_term, extra) = lower_expr(ctx, high, parent_pred)?;
                body.extend(extra);
                body.push(MirAtom::Guard(MirGuard {
                    left: term.clone(),
                    op: MirCompOp::Ge,
                    right: lo_term,
                }));
                body.push(MirAtom::Guard(MirGuard {
                    left: term,
                    op: MirCompOp::Le,
                    right: hi_term,
                }));
            }
        }
    }

    Ok(())
}

// ============================================================
// Disjunction lowering
// ============================================================

fn lower_disjunction(
    ctx: &mut LowerCtx,
    lhs: &Formula,
    rhs: &Formula,
    body: &mut Vec<MirAtom>,
    _parent_pred: &str,
) -> Result<(), LowerError> {
    // Collect free variables from both branches
    let mut all_vars = Vec::new();
    collect_formula_vars(lhs, &mut all_vars);
    collect_formula_vars(rhs, &mut all_vars);
    all_vars.sort();
    all_vars.dedup();

    // Create auxiliary predicate
    let aux_name = ctx.fresh_pred("_disj");
    let params: Vec<MirParam> = all_vars.iter().map(|v| MirParam::new(v, MirType::Any)).collect();
    let terms: Vec<MirTerm> = all_vars.iter().map(|v| MirTerm::var(v)).collect();

    // Lower both branches
    let mut lhs_atoms = Vec::new();
    lower_formula(ctx, lhs, &mut lhs_atoms, &aux_name)?;
    let mut rhs_atoms = Vec::new();
    lower_formula(ctx, rhs, &mut rhs_atoms, &aux_name)?;

    // Emit auxiliary predicate with disjunction
    ctx.emit_predicate(MirPredicate {
        name: aux_name.clone(),
        params,
        body: MirBody::Disjunction(vec![lhs_atoms, rhs_atoms]),
        annotations: MirAnnotations::default(),
        is_abstract: false,
    });

    // Add scan of auxiliary predicate to parent body
    body.push(MirAtom::Scan(MirScan::new(&aux_name, terms)));
    Ok(())
}

// ============================================================
// Negation lowering
// ============================================================

fn lower_negation(
    ctx: &mut LowerCtx,
    inner: &Formula,
    body: &mut Vec<MirAtom>,
    parent_pred: &str,
) -> Result<(), LowerError> {
    // Simple case: negation of a predicate call → NegScan
    match &inner.kind {
        FormulaKind::PredicateCall { name, args } => {
            let mut terms = Vec::new();
            for arg in args {
                let (term, extra) = lower_expr(ctx, arg, parent_pred)?;
                body.extend(extra);
                terms.push(term);
            }
            body.push(MirAtom::NegScan(MirScan::new(&name.name, terms)));
            return Ok(());
        }
        FormulaKind::ExprFormula(expr) => {
            if let ExprKind::FormulaExpr(inner_formula) = &expr.kind {
                return lower_negation(ctx, inner_formula, body, parent_pred);
            }
        }
        FormulaKind::Paren { inner: p } => {
            return lower_negation(ctx, p, body, parent_pred);
        }
        _ => {}
    }

    // General case: create auxiliary predicate, negate it
    let mut all_vars = Vec::new();
    collect_formula_vars(inner, &mut all_vars);
    all_vars.sort();
    all_vars.dedup();

    let aux_name = ctx.fresh_pred("_neg");
    let params: Vec<MirParam> = all_vars.iter().map(|v| MirParam::new(v, MirType::Any)).collect();
    let terms: Vec<MirTerm> = all_vars.iter().map(|v| MirTerm::var(v)).collect();

    let mut inner_atoms = Vec::new();
    lower_formula(ctx, inner, &mut inner_atoms, &aux_name)?;

    ctx.emit_predicate(MirPredicate::new(&aux_name, params, inner_atoms));
    body.push(MirAtom::NegScan(MirScan::new(&aux_name, terms)));
    Ok(())
}

// ============================================================
// Exists lowering
// ============================================================

fn lower_exists(
    ctx: &mut LowerCtx,
    vars: &[VarDecl],
    guard: Option<&Formula>,
    exists_body: &Formula,
    body: &mut Vec<MirAtom>,
    _parent_pred: &str,
) -> Result<(), LowerError> {
    // Collect all free variables (outer + quantified)
    let mut outer_vars = Vec::new();
    if let Some(g) = guard {
        collect_formula_vars(g, &mut outer_vars);
    }
    collect_formula_vars(exists_body, &mut outer_vars);

    // Remove the quantified variables — they are existentially quantified
    let quant_names: Vec<&str> = vars.iter().map(|v| v.name.name.as_str()).collect();
    outer_vars.retain(|v| !quant_names.contains(&v.as_str()));
    outer_vars.sort();
    outer_vars.dedup();

    let aux_name = ctx.fresh_pred("_exists");
    let params: Vec<MirParam> = outer_vars.iter().map(|v| MirParam::new(v, MirType::Any)).collect();
    let terms: Vec<MirTerm> = outer_vars.iter().map(|v| MirTerm::var(v)).collect();

    let mut inner_atoms = Vec::new();
    if let Some(g) = guard {
        lower_formula(ctx, g, &mut inner_atoms, &aux_name)?;
    }
    lower_formula(ctx, exists_body, &mut inner_atoms, &aux_name)?;

    ctx.emit_predicate(MirPredicate::new(&aux_name, params, inner_atoms));
    body.push(MirAtom::Scan(MirScan::new(&aux_name, terms)));
    Ok(())
}

// ============================================================
// Forall lowering
// ============================================================

fn lower_forall(
    ctx: &mut LowerCtx,
    vars: &[VarDecl],
    guard: &Formula,
    forall_body: &Formula,
    body: &mut Vec<MirAtom>,
    _parent_pred: &str,
) -> Result<(), LowerError> {
    // forall(vars | guard | body) ≡ not exists(vars | guard | not body)
    let neg_body = Formula {
        kind: FormulaKind::Negation { inner: Box::new(forall_body.clone()) },
        span: forall_body.span,
    };

    // Collect outer variables (those not quantified)
    let mut all_vars = Vec::new();
    collect_formula_vars(guard, &mut all_vars);
    collect_formula_vars(forall_body, &mut all_vars);
    let quant_names: Vec<&str> = vars.iter().map(|v| v.name.name.as_str()).collect();
    all_vars.retain(|v| !quant_names.contains(&v.as_str()));
    all_vars.sort();
    all_vars.dedup();

    // Create auxiliary for the exists(guard | not body) part
    let aux_name = ctx.fresh_pred("_forall_neg");
    let params: Vec<MirParam> = all_vars.iter().map(|v| MirParam::new(v, MirType::Any)).collect();
    let terms: Vec<MirTerm> = all_vars.iter().map(|v| MirTerm::var(v)).collect();

    let mut inner_atoms = Vec::new();
    lower_formula(ctx, guard, &mut inner_atoms, &aux_name)?;
    lower_formula(ctx, &neg_body, &mut inner_atoms, &aux_name)?;

    ctx.emit_predicate(MirPredicate::new(&aux_name, params, inner_atoms));
    body.push(MirAtom::NegScan(MirScan::new(&aux_name, terms)));
    Ok(())
}

// ============================================================
// If-then-else lowering
// ============================================================

fn lower_if_then_else(
    ctx: &mut LowerCtx,
    cond: &Formula,
    then_f: &Formula,
    else_f: &Formula,
    body: &mut Vec<MirAtom>,
    _parent_pred: &str,
) -> Result<(), LowerError> {
    // if C then T else E ≡ (C and T) or (not C and E)
    let mut all_vars = Vec::new();
    collect_formula_vars(cond, &mut all_vars);
    collect_formula_vars(then_f, &mut all_vars);
    collect_formula_vars(else_f, &mut all_vars);
    all_vars.sort();
    all_vars.dedup();

    let aux_name = ctx.fresh_pred("_ite");
    let params: Vec<MirParam> = all_vars.iter().map(|v| MirParam::new(v, MirType::Any)).collect();
    let terms: Vec<MirTerm> = all_vars.iter().map(|v| MirTerm::var(v)).collect();

    // Then branch: C and T
    let mut then_atoms = Vec::new();
    // Include outer body context atoms for grounding
    then_atoms.extend(body.clone());
    lower_formula(ctx, cond, &mut then_atoms, &aux_name)?;
    lower_formula(ctx, then_f, &mut then_atoms, &aux_name)?;

    // Else branch: not C and E
    let mut else_atoms = Vec::new();
    // Include outer body context atoms for grounding
    else_atoms.extend(body.clone());
    // Negate condition
    lower_negation(ctx, cond, &mut else_atoms, &aux_name)?;
    lower_formula(ctx, else_f, &mut else_atoms, &aux_name)?;

    ctx.emit_predicate(MirPredicate {
        name: aux_name.clone(),
        params,
        body: MirBody::Disjunction(vec![then_atoms, else_atoms]),
        annotations: MirAnnotations::default(),
        is_abstract: false,
    });

    body.push(MirAtom::Scan(MirScan::new(&aux_name, terms)));
    Ok(())
}

// ============================================================
// Comparison lowering
// ============================================================

fn lower_comparison(
    ctx: &mut LowerCtx,
    lhs: &Expr,
    op: ocql_ql_ast::CompOp,
    rhs: &Expr,
    body: &mut Vec<MirAtom>,
    parent_pred: &str,
) -> Result<(), LowerError> {
    let mir_op = lower_comp_op(op);

    // Special case: x = complex_expr → assignment
    if mir_op == MirCompOp::Eq {
        if let ExprKind::Variable(ref name) = lhs.kind {
            if is_complex_expr(rhs) {
                return lower_eq_assignment(ctx, &name.name, rhs, body, parent_pred);
            }
        }
        if let ExprKind::Variable(ref name) = rhs.kind {
            if is_complex_expr(lhs) {
                return lower_eq_assignment(ctx, &name.name, lhs, body, parent_pred);
            }
        }
    }

    let (left, extra) = lower_expr(ctx, lhs, parent_pred)?;
    body.extend(extra);
    let (right, extra) = lower_expr(ctx, rhs, parent_pred)?;
    body.extend(extra);

    body.push(MirAtom::Guard(MirGuard { left, op: mir_op, right }));
    Ok(())
}

fn is_complex_expr(expr: &Expr) -> bool {
    matches!(
        expr.kind,
        ExprKind::BinaryOp { .. }
            | ExprKind::Call { .. }
            | ExprKind::MemberCall { .. }
            | ExprKind::QualifiedCall { .. }
            | ExprKind::Aggregation { .. }
    )
}

fn lower_eq_assignment(
    ctx: &mut LowerCtx,
    var_name: &str,
    expr: &Expr,
    body: &mut Vec<MirAtom>,
    parent_pred: &str,
) -> Result<(), LowerError> {
    match &expr.kind {
        ExprKind::BinaryOp { lhs, op, rhs } => {
            let (left, extra) = lower_expr(ctx, lhs, parent_pred)?;
            body.extend(extra);
            let (right, extra) = lower_expr(ctx, rhs, parent_pred)?;
            body.extend(extra);
            body.push(MirAtom::Assign(MirAssign {
                result_var: var_name.to_string(),
                expr: MirArithExpr {
                    left,
                    op: lower_binop(*op),
                    right,
                },
            }));
            Ok(())
        }
        ExprKind::Call { name, closure: None, args } => {
            // result = f(args) → f(args, result)
            let mut terms = Vec::new();
            for arg in args {
                let (term, extra) = lower_expr(ctx, arg, parent_pred)?;
                body.extend(extra);
                terms.push(term);
            }
            terms.push(MirTerm::var(var_name));
            body.push(MirAtom::Scan(MirScan::new(&name.name, terms)));
            Ok(())
        }
        ExprKind::MemberCall { receiver, name, closure: None, args } => {
            let (recv_term, extra) = lower_expr(ctx, receiver, parent_pred)?;
            body.extend(extra);
            let mut terms = vec![recv_term];
            for arg in args {
                let (term, extra) = lower_expr(ctx, arg, parent_pred)?;
                body.extend(extra);
                terms.push(term);
            }
            terms.push(MirTerm::var(var_name));
            body.push(MirAtom::Scan(MirScan::new(&name.name, terms)));
            Ok(())
        }
        _ => {
            // General case: evaluate expression, guard equality
            let (term, extra) = lower_expr(ctx, expr, parent_pred)?;
            body.extend(extra);
            body.push(MirAtom::Guard(MirGuard {
                left: MirTerm::var(var_name),
                op: MirCompOp::Eq,
                right: term,
            }));
            Ok(())
        }
    }
}

// ============================================================
// Expression lowering
// ============================================================

/// Lower an expression. Returns (term, extra_atoms) where extra_atoms are
/// body elements needed to compute the term (e.g., for predicate calls).
fn lower_expr(
    ctx: &mut LowerCtx,
    expr: &Expr,
    parent_pred: &str,
) -> Result<(MirTerm, Vec<MirAtom>), LowerError> {
    match &expr.kind {
        ExprKind::Literal(lit) => {
            Ok((lower_literal(lit), vec![]))
        }

        ExprKind::Variable(name) => {
            Ok((MirTerm::var(&name.name), vec![]))
        }

        ExprKind::This => {
            Ok((MirTerm::var("this"), vec![]))
        }

        ExprKind::Result => {
            Ok((MirTerm::var("result"), vec![]))
        }

        ExprKind::DontCare => {
            Ok((MirTerm::Wildcard, vec![]))
        }

        ExprKind::BinaryOp { lhs, op, rhs } => {
            let (left, mut extra) = lower_expr(ctx, lhs, parent_pred)?;
            let (right, rextra) = lower_expr(ctx, rhs, parent_pred)?;
            extra.extend(rextra);

            let result_var = ctx.fresh_var();
            extra.push(MirAtom::Assign(MirAssign {
                result_var: result_var.clone(),
                expr: MirArithExpr {
                    left,
                    op: lower_binop(*op),
                    right,
                },
            }));
            Ok((MirTerm::var(&result_var), extra))
        }

        ExprKind::UnaryOp { op, operand } => {
            let (inner, mut extra) = lower_expr(ctx, operand, parent_pred)?;
            match op {
                ocql_ql_ast::UnaryOp::Neg => {
                    let result_var = ctx.fresh_var();
                    extra.push(MirAtom::Assign(MirAssign {
                        result_var: result_var.clone(),
                        expr: MirArithExpr {
                            left: MirTerm::int(0),
                            op: MirArithOp::Sub,
                            right: inner,
                        },
                    }));
                    Ok((MirTerm::var(&result_var), extra))
                }
                ocql_ql_ast::UnaryOp::Plus => Ok((inner, extra)),
            }
        }

        ExprKind::Call { name, closure: _, args } => {
            let mut terms = Vec::new();
            let mut extra = Vec::new();
            for arg in args {
                let (term, e) = lower_expr(ctx, arg, parent_pred)?;
                extra.extend(e);
                terms.push(term);
            }
            // Add result variable (predicate call returns a value)
            let result_var = ctx.fresh_var();
            terms.push(MirTerm::var(&result_var));
            extra.push(MirAtom::Scan(MirScan::new(&name.name, terms)));
            Ok((MirTerm::var(&result_var), extra))
        }

        ExprKind::MemberCall { receiver, name, closure: _, args } => {
            let (recv_term, mut extra) = lower_expr(ctx, receiver, parent_pred)?;
            let mut terms = vec![recv_term];
            for arg in args {
                let (term, e) = lower_expr(ctx, arg, parent_pred)?;
                extra.extend(e);
                terms.push(term);
            }
            let result_var = ctx.fresh_var();
            terms.push(MirTerm::var(&result_var));
            extra.push(MirAtom::Scan(MirScan::new(&name.name, terms)));
            Ok((MirTerm::var(&result_var), extra))
        }

        ExprKind::QualifiedCall { qualifier, name, args } => {
            let pred_name = format!("{}#{}", qualifier.name, name.name);
            let mut terms = Vec::new();
            let mut extra = Vec::new();
            for arg in args {
                let (term, e) = lower_expr(ctx, arg, parent_pred)?;
                extra.extend(e);
                terms.push(term);
            }
            let result_var = ctx.fresh_var();
            terms.push(MirTerm::var(&result_var));
            extra.push(MirAtom::Scan(MirScan::new(&pred_name, terms)));
            Ok((MirTerm::var(&result_var), extra))
        }

        ExprKind::TypeCall { qualifier, name, args } => {
            let pred_name = match qualifier {
                Some(q) => format!("{}#{}", q.name, name.name),
                None => name.name.clone(),
            };
            let mut terms = Vec::new();
            let mut extra = Vec::new();
            for arg in args {
                let (term, e) = lower_expr(ctx, arg, parent_pred)?;
                extra.extend(e);
                terms.push(term);
            }
            let result_var = ctx.fresh_var();
            terms.push(MirTerm::var(&result_var));
            extra.push(MirAtom::Scan(MirScan::new(&pred_name, terms)));
            Ok((MirTerm::var(&result_var), extra))
        }

        ExprKind::PostfixCast { expr: inner, ty } | ExprKind::PrefixCast { ty, expr: inner } => {
            // Cast: evaluate inner, add type check
            let (term, mut extra) = lower_expr(ctx, inner, parent_pred)?;
            if let MirTerm::Var(ref var_name) = term {
                if let TypeExprKind::ClassName(name) = &ty.kind {
                    extra.push(MirAtom::TypeCheck(MirTypeCheck {
                        var: var_name.clone(),
                        type_predicate: format!("{}#char", name.name),
                    }));
                }
            }
            Ok((term, extra))
        }

        ExprKind::Range { low, high } => {
            // [lo..hi] in expression context — create a fresh var with range constraint
            let (lo_term, mut extra) = lower_expr(ctx, low, parent_pred)?;
            let (hi_term, hi_extra) = lower_expr(ctx, high, parent_pred)?;
            extra.extend(hi_extra);

            let result_var = ctx.fresh_var();
            extra.push(MirAtom::Guard(MirGuard {
                left: MirTerm::var(&result_var),
                op: MirCompOp::Ge,
                right: lo_term,
            }));
            extra.push(MirAtom::Guard(MirGuard {
                left: MirTerm::var(&result_var),
                op: MirCompOp::Le,
                right: hi_term,
            }));
            Ok((MirTerm::var(&result_var), extra))
        }

        ExprKind::SetLiteral { elements } => {
            // {a, b, c} — create a fresh var, disjunction of equalities
            // For now, use first element (simplified)
            if elements.is_empty() {
                Ok((MirTerm::Wildcard, vec![]))
            } else {
                let (first, extra) = lower_expr(ctx, &elements[0], parent_pred)?;
                Ok((first, extra))
            }
        }

        ExprKind::Paren(inner) => lower_expr(ctx, inner, parent_pred),

        ExprKind::FormulaExpr(formula) => {
            // Formula in expression context — should be unwrapped
            let mut extra = Vec::new();
            lower_formula(ctx, formula, &mut extra, parent_pred)?;
            Ok((MirTerm::Wildcard, extra))
        }

        ExprKind::Aggregation { kind, vars, guard, expr, separator: _, order_by: _ } => {
            lower_aggregation(ctx, *kind, vars, guard.as_deref(), expr.as_deref(), parent_pred)
        }

        ExprKind::RankExpr { index, vars, guard, expr, order_by: _ } => {
            // rank[i](vars | guard | expr) — shorthand for rank aggregation
            // Lower as: auxiliary predicate for the body, then a Rank aggregate
            // The index expression is bound to the aggregate result
            let aux_name = ctx.fresh_pred("_rank");

            // Collect outer variables from the guard
            let mut outer_vars = Vec::new();
            if let Some(g) = guard {
                collect_formula_vars(g, &mut outer_vars);
            }
            collect_expr_vars(expr, &mut outer_vars);
            let quant_names: Vec<&str> = vars.iter().map(|v| v.name.name.as_str()).collect();
            outer_vars.retain(|v| !quant_names.contains(&v.as_str()));
            outer_vars.sort();
            outer_vars.dedup();

            let agg_var_name = ctx.fresh_var();

            // Build auxiliary predicate body
            let mut inner_atoms = Vec::new();
            if let Some(g) = guard {
                lower_formula(ctx, g, &mut inner_atoms, &aux_name)?;
            }
            let (term, extra_inner) = lower_expr(ctx, expr, &aux_name)?;
            inner_atoms.extend(extra_inner);
            inner_atoms.push(MirAtom::Guard(MirGuard {
                left: MirTerm::var(&agg_var_name),
                op: MirCompOp::Eq,
                right: term,
            }));

            let mut params: Vec<MirParam> = outer_vars.iter().map(|v| MirParam::new(v, MirType::Any)).collect();
            params.push(MirParam::new(&agg_var_name, MirType::Any));
            ctx.emit_predicate(MirPredicate::new(&aux_name, params, inner_atoms));

            // Create the aggregate atom
            let result_var = ctx.fresh_var();
            let mut extra = vec![MirAtom::Aggregate(MirAggregate {
                result_var: result_var.clone(),
                function: MirAggFunction::Rank,
                sub_predicate: aux_name,
                group_by: outer_vars,
                agg_var: agg_var_name,
            })];

            // Bind the index expression to the rank result
            let (idx_term, idx_extra) = lower_expr(ctx, index, parent_pred)?;
            extra.extend(idx_extra);
            extra.push(MirAtom::Guard(MirGuard {
                left: idx_term,
                op: MirCompOp::Eq,
                right: MirTerm::var(&result_var),
            }));

            Ok((MirTerm::var(&result_var), extra))
        }

        ExprKind::AnyExpr { vars, guard, expr } => {
            // any(vars | guard | expr) — shorthand for any aggregation
            // If expr is present, lower as Any aggregate over the expr
            // If expr is absent, lower as Any aggregate over the first var
            let aux_name = ctx.fresh_pred("_any");

            let mut outer_vars = Vec::new();
            if let Some(g) = guard {
                collect_formula_vars(g, &mut outer_vars);
            }
            if let Some(e) = expr {
                collect_expr_vars(e, &mut outer_vars);
            }
            let quant_names: Vec<&str> = vars.iter().map(|v| v.name.name.as_str()).collect();
            outer_vars.retain(|v| !quant_names.contains(&v.as_str()));
            outer_vars.sort();
            outer_vars.dedup();

            let agg_var_name = ctx.fresh_var();

            let mut inner_atoms = Vec::new();
            if let Some(g) = guard {
                lower_formula(ctx, g, &mut inner_atoms, &aux_name)?;
            }

            if let Some(e) = expr {
                let (term, extra_inner) = lower_expr(ctx, e, &aux_name)?;
                inner_atoms.extend(extra_inner);
                inner_atoms.push(MirAtom::Guard(MirGuard {
                    left: MirTerm::var(&agg_var_name),
                    op: MirCompOp::Eq,
                    right: term,
                }));
            } else if let Some(first_var) = vars.first() {
                // No expr: aggregate over the first quantified variable
                inner_atoms.push(MirAtom::Guard(MirGuard {
                    left: MirTerm::var(&agg_var_name),
                    op: MirCompOp::Eq,
                    right: MirTerm::var(&first_var.name.name),
                }));
            }

            let mut params: Vec<MirParam> = outer_vars.iter().map(|v| MirParam::new(v, MirType::Any)).collect();
            params.push(MirParam::new(&agg_var_name, MirType::Any));
            ctx.emit_predicate(MirPredicate::new(&aux_name, params, inner_atoms));

            let result_var = ctx.fresh_var();
            let extra = vec![MirAtom::Aggregate(MirAggregate {
                result_var: result_var.clone(),
                function: MirAggFunction::Any,
                sub_predicate: aux_name,
                group_by: outer_vars,
                agg_var: agg_var_name,
            })];

            Ok((MirTerm::var(&result_var), extra))
        }

        ExprKind::Super { super_type, name, args } => {
            // Type.super.method(args) — dispatch to parent type's method
            // Lower as qualified call: Type#method(this, args..., result)
            let pred_name = format!("{}#{}", super_type.name, name.name);
            let mut terms = vec![MirTerm::var("this")];
            let mut extra = Vec::new();
            for arg in args {
                let (term, e) = lower_expr(ctx, arg, parent_pred)?;
                extra.extend(e);
                terms.push(term);
            }
            let result_var = ctx.fresh_var();
            terms.push(MirTerm::var(&result_var));
            extra.push(MirAtom::Scan(MirScan::new(&pred_name, terms)));
            Ok((MirTerm::var(&result_var), extra))
        }
    }
}

// ============================================================
// Aggregation lowering
// ============================================================

fn lower_aggregation(
    ctx: &mut LowerCtx,
    kind: ocql_ql_ast::AggKind,
    vars: &[VarDecl],
    guard: Option<&Formula>,
    expr: Option<&Expr>,
    _parent_pred: &str,
) -> Result<(MirTerm, Vec<MirAtom>), LowerError> {
    let agg_fn = lower_agg_kind(kind);

    // Create auxiliary predicate for the aggregate body
    let aux_name = ctx.fresh_pred("_agg");

    // Collect outer variables (not quantified by the aggregate)
    let mut outer_vars = Vec::new();
    if let Some(g) = guard {
        collect_formula_vars(g, &mut outer_vars);
    }
    let quant_names: Vec<&str> = vars.iter().map(|v| v.name.name.as_str()).collect();
    outer_vars.retain(|v| !quant_names.contains(&v.as_str()));
    outer_vars.sort();
    outer_vars.dedup();

    // The aggregate variable is the expression result (or a quantified var for count)
    let agg_var_name = ctx.fresh_var();

    // Build the auxiliary predicate body
    let mut inner_atoms = Vec::new();
    if let Some(g) = guard {
        lower_formula(ctx, g, &mut inner_atoms, &aux_name)?;
    }

    if let Some(e) = expr {
        let (term, extra) = lower_expr(ctx, e, &aux_name)?;
        inner_atoms.extend(extra);
        // Bind the aggregate variable to the expression
        inner_atoms.push(MirAtom::Guard(MirGuard {
            left: MirTerm::var(&agg_var_name),
            op: MirCompOp::Eq,
            right: term,
        }));
    }

    // Build params: outer vars + aggregate var
    let mut params: Vec<MirParam> = outer_vars.iter().map(|v| MirParam::new(v, MirType::Any)).collect();
    params.push(MirParam::new(&agg_var_name, MirType::Any));

    ctx.emit_predicate(MirPredicate::new(&aux_name, params, inner_atoms));

    // Create the aggregate atom
    let result_var = ctx.fresh_var();
    let extra = vec![MirAtom::Aggregate(MirAggregate {
        result_var: result_var.clone(),
        function: agg_fn,
        sub_predicate: aux_name,
        group_by: outer_vars,
        agg_var: agg_var_name,
    })];

    Ok((MirTerm::var(&result_var), extra))
}

// ============================================================
// Helper: collect free variables from a formula
// ============================================================

pub fn collect_formula_vars(formula: &Formula, vars: &mut Vec<String>) {
    match &formula.kind {
        FormulaKind::Conjunction { lhs, rhs }
        | FormulaKind::Disjunction { lhs, rhs }
        | FormulaKind::Implies { lhs, rhs } => {
            collect_formula_vars(lhs, vars);
            collect_formula_vars(rhs, vars);
        }
        FormulaKind::Negation { inner } | FormulaKind::Paren { inner } => {
            collect_formula_vars(inner, vars);
        }
        FormulaKind::Comparison { lhs, rhs, .. } => {
            collect_expr_vars(lhs, vars);
            collect_expr_vars(rhs, vars);
        }
        FormulaKind::InstanceOf { expr, .. } => {
            collect_expr_vars(expr, vars);
        }
        FormulaKind::InRange { expr, range } => {
            collect_expr_vars(expr, vars);
            collect_expr_vars(range, vars);
        }
        FormulaKind::Exists { vars: quant_vars, guard, body } => {
            if let Some(g) = guard {
                collect_formula_vars(g, vars);
            }
            collect_formula_vars(body, vars);
            // Remove quantified vars
            let quant: Vec<&str> = quant_vars.iter().map(|v| v.name.name.as_str()).collect();
            vars.retain(|v| !quant.contains(&v.as_str()));
        }
        FormulaKind::ExistsExpr { expr } => {
            collect_expr_vars(expr, vars);
        }
        FormulaKind::Forall { vars: quant_vars, guard, body }
        | FormulaKind::Forex { vars: quant_vars, guard, body } => {
            collect_formula_vars(guard, vars);
            collect_formula_vars(body, vars);
            let quant: Vec<&str> = quant_vars.iter().map(|v| v.name.name.as_str()).collect();
            vars.retain(|v| !quant.contains(&v.as_str()));
        }
        FormulaKind::IfThenElse { cond, then, else_ } => {
            collect_formula_vars(cond, vars);
            collect_formula_vars(then, vars);
            collect_formula_vars(else_, vars);
        }
        FormulaKind::PredicateCall { args, .. } => {
            for arg in args {
                collect_expr_vars(arg, vars);
            }
        }
        FormulaKind::MemberCall { receiver, args, .. } => {
            collect_expr_vars(receiver, vars);
            for arg in args {
                collect_expr_vars(arg, vars);
            }
        }
        FormulaKind::QualifiedCall { args, .. } => {
            for arg in args {
                collect_expr_vars(arg, vars);
            }
        }
        FormulaKind::ExprFormula(expr) => {
            collect_expr_vars(expr, vars);
        }
        FormulaKind::Any | FormulaKind::None => {}
    }
}

fn collect_expr_vars(expr: &Expr, vars: &mut Vec<String>) {
    match &expr.kind {
        ExprKind::Variable(name) => {
            vars.push(name.name.clone());
        }
        ExprKind::This => {
            vars.push("this".to_string());
        }
        ExprKind::Result => {
            vars.push("result".to_string());
        }
        ExprKind::BinaryOp { lhs, rhs, .. } => {
            collect_expr_vars(lhs, vars);
            collect_expr_vars(rhs, vars);
        }
        ExprKind::UnaryOp { operand, .. } => {
            collect_expr_vars(operand, vars);
        }
        ExprKind::Call { args, .. } => {
            for arg in args {
                collect_expr_vars(arg, vars);
            }
        }
        ExprKind::MemberCall { receiver, args, .. } => {
            collect_expr_vars(receiver, vars);
            for arg in args {
                collect_expr_vars(arg, vars);
            }
        }
        ExprKind::QualifiedCall { args, .. } | ExprKind::TypeCall { args, .. } => {
            for arg in args {
                collect_expr_vars(arg, vars);
            }
        }
        ExprKind::PostfixCast { expr: inner, .. }
        | ExprKind::PrefixCast { expr: inner, .. }
        | ExprKind::Paren(inner) => {
            collect_expr_vars(inner, vars);
        }
        ExprKind::Range { low, high } => {
            collect_expr_vars(low, vars);
            collect_expr_vars(high, vars);
        }
        ExprKind::SetLiteral { elements } => {
            for e in elements {
                collect_expr_vars(e, vars);
            }
        }
        ExprKind::FormulaExpr(formula) => {
            collect_formula_vars(formula, vars);
        }
        ExprKind::Aggregation { guard, expr, vars: quant_vars, .. } => {
            if let Some(g) = guard {
                collect_formula_vars(g, vars);
            }
            if let Some(e) = expr {
                collect_expr_vars(e, vars);
            }
            let quant: Vec<&str> = quant_vars.iter().map(|v| v.name.name.as_str()).collect();
            vars.retain(|v| !quant.contains(&v.as_str()));
        }
        ExprKind::RankExpr { index, vars: quant_vars, guard, expr, .. } => {
            collect_expr_vars(index, vars);
            if let Some(g) = guard {
                collect_formula_vars(g, vars);
            }
            collect_expr_vars(expr, vars);
            let quant: Vec<&str> = quant_vars.iter().map(|v| v.name.name.as_str()).collect();
            vars.retain(|v| !quant.contains(&v.as_str()));
        }
        ExprKind::AnyExpr { vars: quant_vars, guard, expr } => {
            if let Some(g) = guard {
                collect_formula_vars(g, vars);
            }
            if let Some(e) = expr {
                collect_expr_vars(e, vars);
            }
            let quant: Vec<&str> = quant_vars.iter().map(|v| v.name.name.as_str()).collect();
            vars.retain(|v| !quant.contains(&v.as_str()));
        }
        ExprKind::Super { args, .. } => {
            for arg in args {
                collect_expr_vars(arg, vars);
            }
        }
        ExprKind::Literal(_) | ExprKind::DontCare => {}
    }
}

// ============================================================
// Type/operator conversion helpers
// ============================================================

fn lower_type_expr_kind(kind: &TypeExprKind) -> MirType {
    match kind {
        TypeExprKind::Primitive(p) => match p {
            ocql_ql_ast::ty::PrimitiveType::Boolean => MirType::Boolean,
            ocql_ql_ast::ty::PrimitiveType::Int => MirType::Int,
            ocql_ql_ast::ty::PrimitiveType::Float => MirType::Float,
            ocql_ql_ast::ty::PrimitiveType::String => MirType::String,
            ocql_ql_ast::ty::PrimitiveType::Date => MirType::Date,
        },
        TypeExprKind::Database(name) => MirType::Entity(name.clone()),
        TypeExprKind::ClassName(name) => MirType::Class(name.name.clone()),
        TypeExprKind::ModuleAccess(_, name) => MirType::Class(name.name.clone()),
    }
}

fn lower_literal(lit: &Literal) -> MirTerm {
    match lit {
        Literal::Int(v) => MirTerm::int(*v),
        Literal::Float(v) => MirTerm::float(*v),
        Literal::String(s) => MirTerm::string(s),
        Literal::Bool(b) => MirTerm::bool(*b),
    }
}

fn lower_comp_op(op: ocql_ql_ast::CompOp) -> MirCompOp {
    match op {
        ocql_ql_ast::CompOp::Eq => MirCompOp::Eq,
        ocql_ql_ast::CompOp::Ne => MirCompOp::Ne,
        ocql_ql_ast::CompOp::Lt => MirCompOp::Lt,
        ocql_ql_ast::CompOp::Gt => MirCompOp::Gt,
        ocql_ql_ast::CompOp::Le => MirCompOp::Le,
        ocql_ql_ast::CompOp::Ge => MirCompOp::Ge,
    }
}

fn lower_binop(op: BinOp) -> MirArithOp {
    match op {
        BinOp::Add => MirArithOp::Add,
        BinOp::Sub => MirArithOp::Sub,
        BinOp::Mul => MirArithOp::Mul,
        BinOp::Div => MirArithOp::Div,
        BinOp::Mod => MirArithOp::Mod,
    }
}

fn lower_agg_kind(kind: ocql_ql_ast::AggKind) -> MirAggFunction {
    match kind {
        ocql_ql_ast::AggKind::Count => MirAggFunction::Count,
        ocql_ql_ast::AggKind::Sum => MirAggFunction::Sum,
        ocql_ql_ast::AggKind::Min => MirAggFunction::Min,
        ocql_ql_ast::AggKind::Max => MirAggFunction::Max,
        ocql_ql_ast::AggKind::Avg => MirAggFunction::Avg,
        ocql_ql_ast::AggKind::Concat => MirAggFunction::Concat,
        ocql_ql_ast::AggKind::Rank => MirAggFunction::Rank,
        ocql_ql_ast::AggKind::Unique => MirAggFunction::Count, // approx
        ocql_ql_ast::AggKind::StrictCount => MirAggFunction::StrictCount,
        ocql_ql_ast::AggKind::StrictSum => MirAggFunction::StrictSum,
        ocql_ql_ast::AggKind::StrictConcat => MirAggFunction::StrictConcat,
        ocql_ql_ast::AggKind::Any => MirAggFunction::Any,
    }
}
