//! Lower QL AST to Datalog rules.
//!
//! Transforms QL predicates, classes, and select queries into flat Datalog
//! rules that `ocql-engine` can evaluate.

use ocql_engine::rule::{
    Atom, ArithExpr, ArithOp, BodyElement, CompOp, Guard, Program, Rule, Term,
};
use ocql_ql_ast::expr::{Expr, ExprKind, VarDecl};
use ocql_ql_ast::formula::{Formula, FormulaKind};
use ocql_ql_ast::module::{ClassDecl, ClassMember, ModuleMember, SourceFile};
use ocql_ql_ast::predicate::Predicate;
use ocql_ql_ast::query::Select;
use ocql_ql_ast::ty::TypeExprKind;
use ocql_ql_ast::{BinOp, Literal};

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

/// Context for lowering, tracks fresh variable and predicate counters.
pub struct LowerCtx {
    fresh_var: u32,
    fresh_pred: u32,
    rules: Vec<Rule>,
}

impl LowerCtx {
    pub fn new() -> Self {
        Self {
            fresh_var: 0,
            fresh_pred: 0,
            rules: Vec::new(),
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

    fn emit(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    pub fn into_program(self) -> Program {
        Program::new(self.rules)
    }
}

/// Lower an entire QL source file to a Datalog program.
pub fn lower_source_file(file: &SourceFile) -> Result<Program, LowerError> {
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
        ModuleMember::Import(_) => Ok(()), // imports don't produce rules
        ModuleMember::Module(_) => Ok(()), // TODO: lower nested modules
        ModuleMember::Newtype(_) => Ok(()),
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
    let mut params: Vec<String> = Vec::new();

    // If this is a member predicate, `this` is an implicit first parameter
    if class_name.is_some() {
        params.push("this".to_string());
    }

    for p in &pred.head.params {
        params.push(p.name.name.clone());
    }

    // If the predicate has a result type, add `result` as the last parameter
    if pred.head.result_type.is_some() {
        params.push("result".to_string());
    }

    let head = Atom::new(
        &pred_name,
        params.iter().map(|p| Term::Var(p.clone())).collect(),
    );

    if let Some(body_formula) = &pred.body {
        // Lower the body formula into rules
        let mut body_elements = Vec::new();

        // For member predicates, add characteristic predicate constraint on `this`
        if let Some(cls) = class_name {
            let char_pred = format!("{}#char", cls);
            body_elements.push(BodyElement::Positive(Atom::new(
                &char_pred,
                vec![Term::Var("this".to_string())],
            )));
        }

        lower_formula(ctx, body_formula, &mut body_elements, &pred_name)?;
        ctx.emit(Rule::new(head, body_elements));
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
                // ClassName() { body } → ClassName#char(this) :- lowered_body.
                let head = Atom::new(
                    &format!("{}#char", class_name),
                    vec![Term::Var("this".to_string())],
                );
                let mut body_elements = Vec::new();

                // Add supertype constraint if extending a class (not a primitive)
                for sup in &class.supertypes {
                    if let TypeExprKind::ClassName(name) = &sup.kind {
                        let super_char = format!("{}#char", name.name);
                        body_elements.push(BodyElement::Positive(Atom::new(
                            &super_char,
                            vec![Term::Var("this".to_string())],
                        )));
                    }
                    // Primitives (int, float, string) don't have char predicates
                }

                lower_formula(ctx, body, &mut body_elements, &format!("{}#char", class_name))?;
                ctx.emit(Rule::new(head, body_elements));
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
// Select query lowering
// ============================================================

fn lower_select(ctx: &mut LowerCtx, select: &Select) -> Result<(), LowerError> {
    // from T1 x1, T2 x2 where P select e1, e2
    // → select_result(e1_val, e2_val) :- type_constraint(x1), type_constraint(x2), P_lowered, ...

    let result_pred = ctx.fresh_pred("select_result");

    let mut body_elements = Vec::new();

    // Lower type constraints from `from` clause
    for var_decl in &select.from {
        lower_type_constraint(ctx, var_decl, &mut body_elements)?;
    }

    // Lower where clause
    if let Some(where_formula) = &select.where_clause {
        lower_formula(ctx, where_formula, &mut body_elements, &result_pred)?;
    }

    // Lower select expressions to get result terms
    let mut result_terms = Vec::new();
    for sel_expr in &select.select_exprs {
        let (term, extra) = lower_expr(ctx, &sel_expr.expr, &result_pred)?;
        body_elements.extend(extra);
        result_terms.push(term);
    }

    let head = Atom::new(&result_pred, result_terms);
    ctx.emit(Rule::new(head, body_elements));

    Ok(())
}

/// Lower a type constraint from a `from` clause variable declaration.
fn lower_type_constraint(
    _ctx: &mut LowerCtx,
    var_decl: &VarDecl,
    body: &mut Vec<BodyElement>,
) -> Result<(), LowerError> {
    let var_name = &var_decl.name.name;
    match &var_decl.ty.kind {
        TypeExprKind::Primitive(_) => {
            // Primitive types (int, float, string, boolean) don't need constraints —
            // they represent universal domains, bounded by the where clause.
            Ok(())
        }
        TypeExprKind::ClassName(name) => {
            // Class type: add characteristic predicate constraint
            let char_pred = format!("{}#char", name.name);
            body.push(BodyElement::Positive(Atom::new(
                &char_pred,
                vec![Term::Var(var_name.clone())],
            )));
            Ok(())
        }
        TypeExprKind::Database(name) => {
            // Database type (@foo): no constraint needed, entity IDs
            // are constrained by the predicates that produce them
            let _ = name;
            Ok(())
        }
        _ => Ok(()),
    }
}

// ============================================================
// Formula lowering
// ============================================================

fn lower_formula(
    ctx: &mut LowerCtx,
    formula: &Formula,
    body: &mut Vec<BodyElement>,
    parent_pred: &str,
) -> Result<(), LowerError> {
    match &formula.kind {
        FormulaKind::Conjunction { lhs, rhs } => {
            lower_formula(ctx, lhs, body, parent_pred)?;
            lower_formula(ctx, rhs, body, parent_pred)?;
        }

        FormulaKind::Disjunction { lhs, rhs } => {
            // A or B in a rule body → need auxiliary predicate
            // aux(vars...) :- A.
            // aux(vars...) :- B.
            // Then body gets: aux(vars...)
            //
            // For simplicity, we emit disjunction as separate rules for the parent.
            // This works when the disjunction is the top-level body.
            // For nested disjunctions, we create auxiliary predicates.
            let aux_pred = ctx.fresh_pred("disj");
            let free_vars = collect_formula_vars(formula);
            let terms: Vec<Term> = free_vars.iter().map(|v| Term::Var(v.clone())).collect();

            // Emit: aux(...) :- lhs.
            let mut lhs_body = Vec::new();
            lower_formula(ctx, lhs, &mut lhs_body, &aux_pred)?;
            ctx.emit(Rule::new(
                Atom::new(&aux_pred, terms.clone()),
                lhs_body,
            ));

            // Emit: aux(...) :- rhs.
            let mut rhs_body = Vec::new();
            lower_formula(ctx, rhs, &mut rhs_body, &aux_pred)?;
            ctx.emit(Rule::new(
                Atom::new(&aux_pred, terms.clone()),
                rhs_body,
            ));

            // Add aux to current body
            body.push(BodyElement::Positive(Atom::new(&aux_pred, terms)));
        }

        FormulaKind::Negation { inner } => {
            // Optimization: if inner is a simple predicate call, negate directly
            match &inner.kind {
                FormulaKind::PredicateCall { name, args } => {
                    let mut terms = Vec::new();
                    for arg in args {
                        let (term, extra) = lower_expr(ctx, arg, parent_pred)?;
                        body.extend(extra);
                        terms.push(term);
                    }
                    body.push(BodyElement::Negated(Atom::new(&name.name, terms)));
                }
                FormulaKind::ExprFormula(expr) => {
                    if let ExprKind::Call { name, args, .. } = &expr.kind {
                        let mut terms = Vec::new();
                        for arg in args {
                            let (term, extra) = lower_expr(ctx, arg, parent_pred)?;
                            body.extend(extra);
                            terms.push(term);
                        }
                        body.push(BodyElement::Negated(Atom::new(&name.name, terms)));
                    } else {
                        lower_negation_aux(ctx, inner, body, parent_pred)?;
                    }
                }
                _ => {
                    lower_negation_aux(ctx, inner, body, parent_pred)?;
                }
            }
        }

        FormulaKind::Comparison { lhs, op, rhs } => {
            lower_comparison(ctx, lhs, *op, rhs, body, parent_pred)?;
        }

        FormulaKind::PredicateCall { name, args } => {
            let mut terms = Vec::new();
            for arg in args {
                let (term, extra) = lower_expr(ctx, arg, parent_pred)?;
                body.extend(extra);
                terms.push(term);
            }
            body.push(BodyElement::Positive(Atom::new(&name.name, terms)));
        }

        FormulaKind::MemberCall { receiver, name, args, closure: _ } => {
            // x.pred(args) → pred(x, args...) or ClassName#pred(x, args...)
            let (recv_term, recv_extra) = lower_expr(ctx, receiver, parent_pred)?;
            body.extend(recv_extra);

            let mut terms = vec![recv_term];
            for arg in args {
                let (term, extra) = lower_expr(ctx, arg, parent_pred)?;
                body.extend(extra);
                terms.push(term);
            }
            // For now, use unqualified name; class dispatch would qualify it
            body.push(BodyElement::Positive(Atom::new(&name.name, terms)));
        }

        FormulaKind::QualifiedCall { qualifier, name, args } => {
            let pred_name = format!("{}#{}", qualifier.name, name.name);
            let mut terms = Vec::new();
            for arg in args {
                let (term, extra) = lower_expr(ctx, arg, parent_pred)?;
                body.extend(extra);
                terms.push(term);
            }
            body.push(BodyElement::Positive(Atom::new(&pred_name, terms)));
        }

        FormulaKind::Exists { vars, guard, body: exists_body } => {
            // exists(T x | guard | body) → just lower guard + body into current context
            // The existential variables are implicitly scoped
            for var_decl in vars {
                lower_type_constraint(ctx, var_decl, body)?;
            }
            if let Some(g) = guard {
                lower_formula(ctx, g, body, parent_pred)?;
            }
            lower_formula(ctx, exists_body, body, parent_pred)?;
        }

        FormulaKind::ExistsExpr { expr } => {
            // exists(expr) → just lower the expression (it must produce a value)
            let (_term, extra) = lower_expr(ctx, expr, parent_pred)?;
            body.extend(extra);
        }

        FormulaKind::Forall { vars, guard, body: forall_body } => {
            // forall(T x | guard | body) ≡ not exists(T x | guard and not body)
            let aux_pred = ctx.fresh_pred("forall_viol");
            let outer_vars = collect_formula_vars(formula);
            let terms: Vec<Term> = outer_vars.iter().map(|v| Term::Var(v.clone())).collect();

            // aux(outer_vars) :- guard, not body_lowered
            let mut aux_body = Vec::new();
            for var_decl in vars {
                lower_type_constraint(ctx, var_decl, &mut aux_body)?;
            }
            lower_formula(ctx, guard, &mut aux_body, &aux_pred)?;

            // Negate the forall body
            let neg_body_pred = ctx.fresh_pred("forall_body");
            let body_vars = collect_formula_vars(forall_body);
            let body_terms: Vec<Term> = body_vars.iter().map(|v| Term::Var(v.clone())).collect();

            let mut inner_body = Vec::new();
            lower_formula(ctx, forall_body, &mut inner_body, &neg_body_pred)?;
            ctx.emit(Rule::new(
                Atom::new(&neg_body_pred, body_terms.clone()),
                inner_body,
            ));
            aux_body.push(BodyElement::Negated(Atom::new(&neg_body_pred, body_terms)));

            ctx.emit(Rule::new(
                Atom::new(&aux_pred, terms.clone()),
                aux_body,
            ));

            body.push(BodyElement::Negated(Atom::new(&aux_pred, terms)));
        }

        FormulaKind::Forex { vars, guard, body: forex_body } => {
            // forex ≡ forall + exists(guard)
            // Lower as forall first
            let forall = Formula {
                kind: FormulaKind::Forall {
                    vars: vars.clone(),
                    guard: guard.clone(),
                    body: forex_body.clone(),
                },
                span: formula.span,
            };
            lower_formula(ctx, &forall, body, parent_pred)?;

            // Then add exists(guard) — at least one thing satisfies the guard
            let exists = Formula {
                kind: FormulaKind::Exists {
                    vars: vars.clone(),
                    guard: None,
                    body: guard.clone(),
                },
                span: formula.span,
            };
            lower_formula(ctx, &exists, body, parent_pred)?;
        }

        FormulaKind::Implies { lhs, rhs } => {
            // A implies B ≡ not A or B ≡ not (A and not B)
            let desugared = Formula {
                kind: FormulaKind::Disjunction {
                    lhs: Box::new(Formula {
                        kind: FormulaKind::Negation { inner: lhs.clone() },
                        span: formula.span,
                    }),
                    rhs: rhs.clone(),
                },
                span: formula.span,
            };
            lower_formula(ctx, &desugared, body, parent_pred)?;
        }

        FormulaKind::IfThenElse { cond, then, else_ } => {
            // if C then T else E →
            //   ite_N(free_vars) :- C, T.
            //   ite_N(free_vars) :- cond_neg(cond_vars), E.
            //   cond_neg(cond_vars) :- not cond_pos(cond_vars).
            //   cond_pos(cond_vars) :- C.
            // But negation needs positive grounding. So instead:
            //   ite_N(vars) :- C, T.                     [then-branch]
            //   ite_N(vars) :- not cond_N(cond_vars), E. [else-branch needs grounding]
            //   cond_N(cond_vars) :- C.
            // For the else branch, we need to also include body elements from
            // the outer context to ground the variables. The simplest approach:
            // add all already-accumulated body elements as grounding.
            let aux_pred = ctx.fresh_pred("ite");
            let free_vars = collect_formula_vars(formula);
            let terms: Vec<Term> = free_vars.iter().map(|v| Term::Var(v.clone())).collect();

            // Then-branch: aux(vars) :- cond, then_body.
            let mut then_body = Vec::new();
            lower_formula(ctx, cond, &mut then_body, &aux_pred)?;
            lower_formula(ctx, then, &mut then_body, &aux_pred)?;
            ctx.emit(Rule::new(
                Atom::new(&aux_pred, terms.clone()),
                then_body,
            ));

            // Else-branch: aux(vars) :- not cond_aux(cond_vars), else_body.
            // Create cond_aux for the negation
            let cond_aux = ctx.fresh_pred("cond");
            let cond_vars = collect_formula_vars(cond);
            let cond_terms: Vec<Term> = cond_vars.iter().map(|v| Term::Var(v.clone())).collect();

            let mut cond_body = Vec::new();
            lower_formula(ctx, cond, &mut cond_body, &cond_aux)?;
            ctx.emit(Rule::new(
                Atom::new(&cond_aux, cond_terms.clone()),
                cond_body,
            ));

            // The else branch needs grounding for free vars not in else_.
            // Include a copy of the accumulated outer body elements for grounding.
            let mut else_body_elements: Vec<BodyElement> = body.clone();
            else_body_elements.push(BodyElement::Negated(Atom::new(&cond_aux, cond_terms)));
            lower_formula(ctx, else_, &mut else_body_elements, &aux_pred)?;
            ctx.emit(Rule::new(
                Atom::new(&aux_pred, terms.clone()),
                else_body_elements,
            ));

            body.push(BodyElement::Positive(Atom::new(&aux_pred, terms)));
        }

        FormulaKind::InRange { expr, range } => {
            // x in [low..high] → x >= low, x <= high
            let (expr_term, expr_extra) = lower_expr(ctx, expr, parent_pred)?;
            body.extend(expr_extra);

            if let ExprKind::Range { low, high } = &range.kind {
                let (low_term, low_extra) = lower_expr(ctx, low, parent_pred)?;
                let (high_term, high_extra) = lower_expr(ctx, high, parent_pred)?;
                body.extend(low_extra);
                body.extend(high_extra);

                body.push(BodyElement::Guard(Guard {
                    left: expr_term.clone(),
                    op: CompOp::Ge,
                    right: low_term,
                }));
                body.push(BodyElement::Guard(Guard {
                    left: expr_term,
                    op: CompOp::Le,
                    right: high_term,
                }));
            } else {
                // expr in non-range → treat as equality
                let (range_term, range_extra) = lower_expr(ctx, range, parent_pred)?;
                body.extend(range_extra);
                body.push(BodyElement::Guard(Guard {
                    left: expr_term,
                    op: CompOp::Eq,
                    right: range_term,
                }));
            }
        }

        FormulaKind::InstanceOf { expr, ty } => {
            // x instanceof T → T#char(x)
            let (expr_term, expr_extra) = lower_expr(ctx, expr, parent_pred)?;
            body.extend(expr_extra);

            match &ty.kind {
                TypeExprKind::ClassName(name) => {
                    let char_pred = format!("{}#char", name.name);
                    body.push(BodyElement::Positive(Atom::new(
                        &char_pred,
                        vec![expr_term],
                    )));
                }
                _ => {} // primitive instanceof is a no-op
            }
        }

        FormulaKind::Any => {
            // always true — no body element needed
        }

        FormulaKind::None => {
            // always false — emit impossible guard
            body.push(BodyElement::Guard(Guard {
                left: Term::Const(ocql_database::Value::Int(0)),
                op: CompOp::Ne,
                right: Term::Const(ocql_database::Value::Int(0)),
            }));
        }

        FormulaKind::Paren { inner } => {
            lower_formula(ctx, inner, body, parent_pred)?;
        }

        FormulaKind::ExprFormula(expr) => {
            // Unwrap bridge nodes: ExprFormula(FormulaExpr(inner)) → lower inner directly
            if let ExprKind::FormulaExpr(inner_formula) = &expr.kind {
                lower_formula(ctx, inner_formula, body, parent_pred)?;
            } else if let ExprKind::Paren(inner) = &expr.kind {
                if let ExprKind::FormulaExpr(inner_formula) = &inner.kind {
                    lower_formula(ctx, inner_formula, body, parent_pred)?;
                } else {
                    lower_expr_as_formula(ctx, expr, body, parent_pred)?;
                }
            } else {
                lower_expr_as_formula(ctx, expr, body, parent_pred)?;
            }
        }
    }

    Ok(())
}

/// Lower a negation via an auxiliary predicate (general case).
fn lower_negation_aux(
    ctx: &mut LowerCtx,
    inner: &Formula,
    body: &mut Vec<BodyElement>,
    _parent_pred: &str,
) -> Result<(), LowerError> {
    let aux_pred = ctx.fresh_pred("neg");
    let free_vars = collect_formula_vars(inner);
    let terms: Vec<Term> = free_vars.iter().map(|v| Term::Var(v.clone())).collect();

    let mut inner_body = Vec::new();
    lower_formula(ctx, inner, &mut inner_body, &aux_pred)?;
    ctx.emit(Rule::new(
        Atom::new(&aux_pred, terms.clone()),
        inner_body,
    ));

    body.push(BodyElement::Negated(Atom::new(&aux_pred, terms)));
    Ok(())
}

/// Lower a comparison formula.
///
/// Handles the special case where `var = complex_expr` should become an
/// assignment rather than a guard. This is needed because the engine can't
/// evaluate guards with unbound variables (e.g., `result = x + x` where
/// `result` is only bound through the equality).
fn lower_comparison(
    ctx: &mut LowerCtx,
    lhs: &Expr,
    op: ocql_ql_ast::CompOp,
    rhs: &Expr,
    body: &mut Vec<BodyElement>,
    parent_pred: &str,
) -> Result<(), LowerError> {
    if op == ocql_ql_ast::CompOp::Eq {
        // Check if this is `simple_var = complex_expr` pattern
        if let Some(var_name) = simple_var_name(lhs) {
            if is_complex_expr(rhs) {
                // result = x + x  →  Assign { result, x + x }
                // result = f(x)   →  f(x, result) directly
                return lower_eq_assignment(ctx, &var_name, rhs, body, parent_pred);
            }
        }
        if let Some(var_name) = simple_var_name(rhs) {
            if is_complex_expr(lhs) {
                return lower_eq_assignment(ctx, &var_name, lhs, body, parent_pred);
            }
        }
    }

    // Default: evaluate both sides and emit a guard
    let (left_term, left_extra) = lower_expr(ctx, lhs, parent_pred)?;
    let (right_term, right_extra) = lower_expr(ctx, rhs, parent_pred)?;
    body.extend(left_extra);
    body.extend(right_extra);

    let comp_op = lower_comp_op(op);
    body.push(BodyElement::Guard(Guard {
        left: left_term,
        op: comp_op,
        right: right_term,
    }));
    Ok(())
}

/// Check if an expression is a simple variable reference.
fn simple_var_name(expr: &Expr) -> Option<String> {
    match &expr.kind {
        ExprKind::Variable(name) => Some(name.name.clone()),
        ExprKind::Result => Some("result".to_string()),
        ExprKind::This => Some("this".to_string()),
        _ => None,
    }
}

/// Check if an expression is "complex" (involves arithmetic or calls).
fn is_complex_expr(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::BinaryOp { .. } | ExprKind::UnaryOp { .. } => true,
        ExprKind::Call { .. } | ExprKind::MemberCall { .. } | ExprKind::QualifiedCall { .. } => true,
        ExprKind::Paren(inner) => is_complex_expr(inner),
        _ => false,
    }
}

/// Lower `var = complex_expr` as a direct assignment or call binding.
fn lower_eq_assignment(
    ctx: &mut LowerCtx,
    var_name: &str,
    expr: &Expr,
    body: &mut Vec<BodyElement>,
    parent_pred: &str,
) -> Result<(), LowerError> {
    match &expr.kind {
        ExprKind::BinaryOp { lhs, op, rhs } => {
            // var = a + b → Assign { var, a op b }
            let (left_term, left_extra) = lower_expr(ctx, lhs, parent_pred)?;
            let (right_term, right_extra) = lower_expr(ctx, rhs, parent_pred)?;
            body.extend(left_extra);
            body.extend(right_extra);

            body.push(BodyElement::Assign {
                result_var: var_name.to_string(),
                expr: ArithExpr {
                    left: left_term,
                    op: lower_binop(*op),
                    right: right_term,
                },
            });
            Ok(())
        }
        ExprKind::UnaryOp { op, operand } => {
            let (inner_term, extra) = lower_expr(ctx, operand, parent_pred)?;
            body.extend(extra);
            match op {
                ocql_ql_ast::UnaryOp::Neg => {
                    body.push(BodyElement::Assign {
                        result_var: var_name.to_string(),
                        expr: ArithExpr {
                            left: Term::Const(ocql_database::Value::Int(0)),
                            op: ArithOp::Sub,
                            right: inner_term,
                        },
                    });
                }
                ocql_ql_ast::UnaryOp::Plus => {
                    // +x is just x
                    body.push(BodyElement::Guard(Guard {
                        left: Term::Var(var_name.to_string()),
                        op: CompOp::Eq,
                        right: inner_term,
                    }));
                }
            }
            Ok(())
        }
        ExprKind::Call { name, args, closure: _ } => {
            // var = f(args) → f(args..., var)
            let mut terms = Vec::new();
            for arg in args {
                let (term, extra) = lower_expr(ctx, arg, parent_pred)?;
                body.extend(extra);
                terms.push(term);
            }
            terms.push(Term::Var(var_name.to_string()));
            body.push(BodyElement::Positive(Atom::new(&name.name, terms)));
            Ok(())
        }
        ExprKind::MemberCall { receiver, name, args, closure: _ } => {
            let (recv_term, recv_extra) = lower_expr(ctx, receiver, parent_pred)?;
            body.extend(recv_extra);
            let mut terms = vec![recv_term];
            for arg in args {
                let (term, extra) = lower_expr(ctx, arg, parent_pred)?;
                body.extend(extra);
                terms.push(term);
            }
            terms.push(Term::Var(var_name.to_string()));
            body.push(BodyElement::Positive(Atom::new(&name.name, terms)));
            Ok(())
        }
        ExprKind::QualifiedCall { qualifier, name, args } => {
            let pred_name = format!("{}#{}", qualifier.name, name.name);
            let mut terms = Vec::new();
            for arg in args {
                let (term, extra) = lower_expr(ctx, arg, parent_pred)?;
                body.extend(extra);
                terms.push(term);
            }
            terms.push(Term::Var(var_name.to_string()));
            body.push(BodyElement::Positive(Atom::new(&pred_name, terms)));
            Ok(())
        }
        ExprKind::Paren(inner) => lower_eq_assignment(ctx, var_name, inner, body, parent_pred),
        _ => {
            // Fallback: evaluate expr and emit guard
            let (term, extra) = lower_expr(ctx, expr, parent_pred)?;
            body.extend(extra);
            body.push(BodyElement::Guard(Guard {
                left: Term::Var(var_name.to_string()),
                op: CompOp::Eq,
                right: term,
            }));
            Ok(())
        }
    }
}

/// Lower an expression that appears in formula context (e.g., a bare call).
fn lower_expr_as_formula(
    ctx: &mut LowerCtx,
    expr: &Expr,
    body: &mut Vec<BodyElement>,
    parent_pred: &str,
) -> Result<(), LowerError> {
    match &expr.kind {
        ExprKind::Call { name, args, closure: _ } => {
            let mut terms = Vec::new();
            for arg in args {
                let (term, extra) = lower_expr(ctx, arg, parent_pred)?;
                body.extend(extra);
                terms.push(term);
            }
            body.push(BodyElement::Positive(Atom::new(&name.name, terms)));
            Ok(())
        }
        ExprKind::MemberCall { receiver, name, args, closure: _ } => {
            let (recv_term, recv_extra) = lower_expr(ctx, receiver, parent_pred)?;
            body.extend(recv_extra);
            let mut terms = vec![recv_term];
            for arg in args {
                let (term, extra) = lower_expr(ctx, arg, parent_pred)?;
                body.extend(extra);
                terms.push(term);
            }
            body.push(BodyElement::Positive(Atom::new(&name.name, terms)));
            Ok(())
        }
        ExprKind::QualifiedCall { qualifier, name, args } => {
            let pred_name = format!("{}#{}", qualifier.name, name.name);
            let mut terms = Vec::new();
            for arg in args {
                let (term, extra) = lower_expr(ctx, arg, parent_pred)?;
                body.extend(extra);
                terms.push(term);
            }
            body.push(BodyElement::Positive(Atom::new(&pred_name, terms)));
            Ok(())
        }
        _ => {
            // Other expressions in formula context: just evaluate them
            let (_term, extra) = lower_expr(ctx, expr, parent_pred)?;
            body.extend(extra);
            Ok(())
        }
    }
}

// ============================================================
// Expression lowering
// ============================================================

/// Lower an expression, returning (term, extra_body_elements).
/// The term is the Datalog variable/constant representing the expression's value.
/// Extra body elements may be needed (e.g., for calls that produce results).
fn lower_expr(
    ctx: &mut LowerCtx,
    expr: &Expr,
    parent_pred: &str,
) -> Result<(Term, Vec<BodyElement>), LowerError> {
    match &expr.kind {
        ExprKind::Literal(lit) => {
            let term = lower_literal(lit);
            Ok((term, vec![]))
        }

        ExprKind::Variable(name) => {
            Ok((Term::Var(name.name.clone()), vec![]))
        }

        ExprKind::This => {
            Ok((Term::Var("this".to_string()), vec![]))
        }

        ExprKind::Result => {
            Ok((Term::Var("result".to_string()), vec![]))
        }

        ExprKind::DontCare => {
            // Each don't-care is an independent fresh variable
            let v = ctx.fresh_var();
            Ok((Term::Var(v), vec![]))
        }

        ExprKind::BinaryOp { lhs, op, rhs } => {
            let (left_term, mut extra) = lower_expr(ctx, lhs, parent_pred)?;
            let (right_term, right_extra) = lower_expr(ctx, rhs, parent_pred)?;
            extra.extend(right_extra);

            let result_var = ctx.fresh_var();
            let arith_op = lower_binop(*op);
            extra.push(BodyElement::Assign {
                result_var: result_var.clone(),
                expr: ArithExpr {
                    left: left_term,
                    op: arith_op,
                    right: right_term,
                },
            });

            Ok((Term::Var(result_var), extra))
        }

        ExprKind::UnaryOp { op, operand } => {
            let (inner_term, mut extra) = lower_expr(ctx, operand, parent_pred)?;
            match op {
                ocql_ql_ast::UnaryOp::Neg => {
                    let result_var = ctx.fresh_var();
                    extra.push(BodyElement::Assign {
                        result_var: result_var.clone(),
                        expr: ArithExpr {
                            left: Term::Const(ocql_database::Value::Int(0)),
                            op: ArithOp::Sub,
                            right: inner_term,
                        },
                    });
                    Ok((Term::Var(result_var), extra))
                }
                ocql_ql_ast::UnaryOp::Plus => {
                    Ok((inner_term, extra))
                }
            }
        }

        ExprKind::Call { name, args, closure: _ } => {
            // Call with result: pred(args) = result_var
            // → pred(args..., result_var)
            let mut terms = Vec::new();
            let mut extra = Vec::new();
            for arg in args {
                let (term, arg_extra) = lower_expr(ctx, arg, parent_pred)?;
                extra.extend(arg_extra);
                terms.push(term);
            }

            let result_var = ctx.fresh_var();
            terms.push(Term::Var(result_var.clone()));
            extra.push(BodyElement::Positive(Atom::new(&name.name, terms)));

            Ok((Term::Var(result_var), extra))
        }

        ExprKind::MemberCall { receiver, name, args, closure: _ } => {
            let (recv_term, mut extra) = lower_expr(ctx, receiver, parent_pred)?;

            let mut terms = vec![recv_term];
            for arg in args {
                let (term, arg_extra) = lower_expr(ctx, arg, parent_pred)?;
                extra.extend(arg_extra);
                terms.push(term);
            }

            let result_var = ctx.fresh_var();
            terms.push(Term::Var(result_var.clone()));
            extra.push(BodyElement::Positive(Atom::new(&name.name, terms)));

            Ok((Term::Var(result_var), extra))
        }

        ExprKind::QualifiedCall { qualifier, name, args } => {
            let pred_name = format!("{}#{}", qualifier.name, name.name);
            let mut terms = Vec::new();
            let mut extra = Vec::new();
            for arg in args {
                let (term, arg_extra) = lower_expr(ctx, arg, parent_pred)?;
                extra.extend(arg_extra);
                terms.push(term);
            }
            let result_var = ctx.fresh_var();
            terms.push(Term::Var(result_var.clone()));
            extra.push(BodyElement::Positive(Atom::new(&pred_name, terms)));
            Ok((Term::Var(result_var), extra))
        }

        ExprKind::Paren(inner) => lower_expr(ctx, inner, parent_pred),

        ExprKind::Range { low, high } => {
            // [low..high] as an expression: generates a range variable
            // This is tricky — the engine doesn't natively support range generation.
            // For now, if both are constants, we note it.
            // The range constraint is typically applied via InRange in formula context.
            let (low_term, mut extra) = lower_expr(ctx, low, parent_pred)?;
            let (high_term, high_extra) = lower_expr(ctx, high, parent_pred)?;
            extra.extend(high_extra);

            let range_var = ctx.fresh_var();
            // Add range constraints: range_var >= low, range_var <= high
            extra.push(BodyElement::Guard(Guard {
                left: Term::Var(range_var.clone()),
                op: CompOp::Ge,
                right: low_term,
            }));
            extra.push(BodyElement::Guard(Guard {
                left: Term::Var(range_var.clone()),
                op: CompOp::Le,
                right: high_term,
            }));

            Ok((Term::Var(range_var), extra))
        }

        ExprKind::FormulaExpr(formula) => {
            // A formula in expression context — lower as formula side-effect
            let mut extra = Vec::new();
            lower_formula(ctx, formula, &mut extra, parent_pred)?;
            // The "value" is meaningless here; this is used for its side effects
            Ok((Term::Const(ocql_database::Value::Bool(true)), extra))
        }

        ExprKind::TypeCall { qualifier: _, name, args } => {
            // TypeCall: used as constructor. T(args) → T#char(args..., result)
            let mut terms = Vec::new();
            let mut extra = Vec::new();
            for arg in args {
                let (term, arg_extra) = lower_expr(ctx, arg, parent_pred)?;
                extra.extend(arg_extra);
                terms.push(term);
            }
            let result_var = ctx.fresh_var();
            terms.push(Term::Var(result_var.clone()));
            let pred_name = format!("{}#char", name.name);
            extra.push(BodyElement::Positive(Atom::new(&pred_name, terms)));
            Ok((Term::Var(result_var), extra))
        }

        _ => {
            // SetLiteral, Aggregation, Cast, Super, etc. — not yet supported
            Err(LowerError::Unsupported(format!("{:?}", expr.kind)))
        }
    }
}

// ============================================================
// Helpers
// ============================================================

fn lower_literal(lit: &Literal) -> Term {
    match lit {
        Literal::Int(n) => Term::Const(ocql_database::Value::Int(*n)),
        Literal::Float(f) => Term::Const(ocql_database::Value::Float(
            ordered_float::OrderedFloat(*f),
        )),
        Literal::String(s) => Term::StrLit(s.clone()),
        Literal::Bool(b) => Term::Const(ocql_database::Value::Bool(*b)),
    }
}

fn lower_comp_op(op: ocql_ql_ast::CompOp) -> CompOp {
    match op {
        ocql_ql_ast::CompOp::Eq => CompOp::Eq,
        ocql_ql_ast::CompOp::Ne => CompOp::Ne,
        ocql_ql_ast::CompOp::Lt => CompOp::Lt,
        ocql_ql_ast::CompOp::Gt => CompOp::Gt,
        ocql_ql_ast::CompOp::Le => CompOp::Le,
        ocql_ql_ast::CompOp::Ge => CompOp::Ge,
    }
}

fn lower_binop(op: BinOp) -> ArithOp {
    match op {
        BinOp::Add => ArithOp::Add,
        BinOp::Sub => ArithOp::Sub,
        BinOp::Mul => ArithOp::Mul,
        BinOp::Div => ArithOp::Div,
        BinOp::Mod => ArithOp::Mod,
    }
}

/// Collect free variable names from a formula (for building auxiliary predicates).
fn collect_formula_vars(formula: &Formula) -> Vec<String> {
    let mut vars = Vec::new();
    collect_formula_vars_inner(formula, &mut vars);
    vars.sort();
    vars.dedup();
    vars
}

fn collect_formula_vars_inner(formula: &Formula, vars: &mut Vec<String>) {
    match &formula.kind {
        FormulaKind::Conjunction { lhs, rhs }
        | FormulaKind::Disjunction { lhs, rhs }
        | FormulaKind::Implies { lhs, rhs } => {
            collect_formula_vars_inner(lhs, vars);
            collect_formula_vars_inner(rhs, vars);
        }
        FormulaKind::Negation { inner } | FormulaKind::Paren { inner } => {
            collect_formula_vars_inner(inner, vars);
        }
        FormulaKind::Comparison { lhs, rhs, .. } => {
            collect_expr_vars(lhs, vars);
            collect_expr_vars(rhs, vars);
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
        FormulaKind::Exists { vars: decls, guard, body } => {
            let bound: std::collections::HashSet<&str> =
                decls.iter().map(|d| d.name.name.as_str()).collect();
            // Collect from guard and body, excluding bound vars
            let mut inner = Vec::new();
            if let Some(g) = guard {
                collect_formula_vars_inner(g, &mut inner);
            }
            collect_formula_vars_inner(body, &mut inner);
            for v in inner {
                if !bound.contains(v.as_str()) {
                    vars.push(v);
                }
            }
        }
        FormulaKind::ExistsExpr { expr } => {
            collect_expr_vars(expr, vars);
        }
        FormulaKind::Forall { vars: decls, guard, body }
        | FormulaKind::Forex { vars: decls, guard, body } => {
            let bound: std::collections::HashSet<&str> =
                decls.iter().map(|d| d.name.name.as_str()).collect();
            let mut inner = Vec::new();
            collect_formula_vars_inner(guard, &mut inner);
            collect_formula_vars_inner(body, &mut inner);
            for v in inner {
                if !bound.contains(v.as_str()) {
                    vars.push(v);
                }
            }
        }
        FormulaKind::IfThenElse { cond, then, else_ } => {
            collect_formula_vars_inner(cond, vars);
            collect_formula_vars_inner(then, vars);
            collect_formula_vars_inner(else_, vars);
        }
        FormulaKind::InstanceOf { expr, .. } => {
            collect_expr_vars(expr, vars);
        }
        FormulaKind::InRange { expr, range } => {
            collect_expr_vars(expr, vars);
            collect_expr_vars(range, vars);
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
        ExprKind::This => vars.push("this".to_string()),
        ExprKind::Result => vars.push("result".to_string()),
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
        ExprKind::QualifiedCall { args, .. } => {
            for arg in args {
                collect_expr_vars(arg, vars);
            }
        }
        ExprKind::Paren(inner) => collect_expr_vars(inner, vars),
        ExprKind::Range { low, high } => {
            collect_expr_vars(low, vars);
            collect_expr_vars(high, vars);
        }
        ExprKind::FormulaExpr(f) => collect_formula_vars_inner(f, vars),
        ExprKind::PostfixCast { expr, .. } | ExprKind::PrefixCast { expr, .. } => {
            collect_expr_vars(expr, vars);
        }
        _ => {}
    }
}
