//! MIR → LIR lowering.
//!
//! Transforms a flat MIR program (named predicates with conjunctive/disjunctive
//! bodies) into a stratified LIR program of relational algebra operators.
//!
//! ## Lowering stages:
//!
//! 1. **Monomorphization**: Each MIR predicate becomes a concrete relation.
//! 2. **Clause lowering**: Each conjunction becomes a relational plan:
//!    - Positive scans → WCO join atoms
//!    - Guards → Filter nodes
//!    - Assignments → Extend nodes
//!    - Negated scans → AntiJoin nodes
//!    - Aggregates → Aggregate nodes
//! 3. **Disjunction lowering**: Multiple clauses → Union.
//! 4. **Stratification**: Build dependency graph, compute strata via SCC.

use std::collections::{HashMap, HashSet};

use ocql_mir::nodes::*;
use crate::nodes::*;

// ============================================================
// Error
// ============================================================

#[derive(Debug)]
pub enum LowerError {
    NegationCycle(Vec<String>),
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LowerError::NegationCycle(preds) => {
                write!(f, "negation cycle: {}", preds.join(", "))
            }
        }
    }
}

impl std::error::Error for LowerError {}

// ============================================================
// Public API
// ============================================================

/// Lower a MIR program to a stratified LIR program.
pub fn lower_mir(mir: &MirProgram) -> Result<LirProgram, LowerError> {
    let mut ctx = LowerCtx::new();

    // Phase 1: Lower all predicates to LIR rules
    let mut all_rules: Vec<LirRule> = Vec::new();
    for pred in &mir.predicates {
        let rules = ctx.lower_predicate(pred);
        all_rules.extend(rules);
    }

    // Phase 2: Stratify
    let strata = stratify_rules(&all_rules, mir)?;

    Ok(LirProgram { strata })
}

// ============================================================
// Lowering context
// ============================================================

struct LowerCtx {
    /// Fresh variable counter for anonymous/temporary variables
    fresh_counter: u32,
}

impl LowerCtx {
    fn new() -> Self {
        Self { fresh_counter: 0 }
    }

    fn fresh_var(&mut self) -> String {
        let name = format!("_lir{}", self.fresh_counter);
        self.fresh_counter += 1;
        name
    }

    /// Lower a single MIR predicate to one or more LIR rules.
    fn lower_predicate(&mut self, pred: &MirPredicate) -> Vec<LirRule> {
        let target = pred.name.clone();
        let target_columns: Vec<String> = pred.params.iter()
            .map(|p| p.name.clone())
            .collect();

        match &pred.body {
            MirBody::None => {
                // Abstract predicate — no rules
                vec![]
            }
            MirBody::Conjunction(atoms) => {
                let body = self.lower_conjunction(atoms, &target_columns);
                vec![LirRule {
                    target,
                    target_columns,
                    body,
                }]
            }
            MirBody::Disjunction(clauses) => {
                if clauses.len() == 1 {
                    let body = self.lower_conjunction(&clauses[0], &target_columns);
                    vec![LirRule {
                        target,
                        target_columns,
                        body,
                    }]
                } else {
                    // Multiple clauses → one rule per clause (union at the relation level)
                    clauses.iter().map(|clause| {
                        let body = self.lower_conjunction(clause, &target_columns);
                        LirRule {
                            target: target.clone(),
                            target_columns: target_columns.clone(),
                            body,
                        }
                    }).collect()
                }
            }
        }
    }

    /// Lower a conjunction of MIR atoms to a LIR plan.
    ///
    /// Strategy:
    /// 1. Collect all positive scans → build a WCO join
    /// 2. Layer filters (guards) on top
    /// 3. Layer extends (assignments) on top
    /// 4. Layer anti-joins (negated scans) on top
    /// 5. Layer aggregations on top
    /// 6. Final projection to target columns
    fn lower_conjunction(
        &mut self,
        atoms: &[MirAtom],
        target_columns: &[String],
    ) -> LirPlan {
        // Classify atoms
        let mut positive_scans: Vec<&MirScan> = Vec::new();
        let mut neg_scans: Vec<&MirScan> = Vec::new();
        let mut guards: Vec<&MirGuard> = Vec::new();
        let mut assigns: Vec<&MirAssign> = Vec::new();
        let mut aggregates: Vec<&MirAggregate> = Vec::new();
        let mut type_checks: Vec<&MirTypeCheck> = Vec::new();

        for atom in atoms {
            match atom {
                MirAtom::Scan(s) => positive_scans.push(s),
                MirAtom::NegScan(s) => neg_scans.push(s),
                MirAtom::Guard(g) => guards.push(g),
                MirAtom::Assign(a) => assigns.push(a),
                MirAtom::Aggregate(a) => aggregates.push(a),
                MirAtom::TypeCheck(tc) => type_checks.push(tc),
            }
        }

        // TypeChecks are positive scans on characteristic predicates
        let type_check_scans: Vec<MirScan> = type_checks.iter().map(|tc| {
            MirScan::new(&tc.type_predicate, vec![MirTerm::var(&tc.var)])
        }).collect();
        for s in &type_check_scans {
            positive_scans.push(s);
        }

        // Build the base plan from positive scans
        let mut plan = self.build_join_plan(&positive_scans);

        // Layer guards as filters
        let filters = self.lower_guards(&guards);
        if !filters.is_empty() {
            for f in filters {
                plan = LirPlan::Filter {
                    input: Box::new(plan),
                    condition: f,
                };
            }
        }

        // Layer assignments as extends
        for assign in &assigns {
            plan = LirPlan::Extend {
                input: Box::new(plan),
                column: assign.result_var.clone(),
                expr: lower_arith_expr(&assign.expr),
            };
        }

        // Layer negated scans as anti-joins
        for neg in &neg_scans {
            plan = self.build_anti_join(plan, neg);
        }

        // Layer aggregations
        for agg in &aggregates {
            plan = self.build_aggregate(plan, agg);
        }

        // Final projection: keep only the target columns
        // (but only if there are extra columns to drop)
        let available = collect_plan_columns(&plan);
        let needed: HashSet<&str> = target_columns.iter().map(|s| s.as_str()).collect();
        if available.iter().any(|c| !needed.contains(c.as_str())) {
            plan = LirPlan::Project {
                input: Box::new(plan),
                columns: target_columns.to_vec(),
            };
        }

        plan
    }

    /// Build a join plan from positive scans.
    ///
    /// If there's exactly one scan, it becomes a simple Scan node.
    /// If there are multiple scans sharing variables, they become a WCO join.
    /// If there are no scans, return a single-row Constant.
    fn build_join_plan(&mut self, scans: &[&MirScan]) -> LirPlan {
        if scans.is_empty() {
            // No scans → single empty-tuple constant (for guards/assigns only)
            return LirPlan::Constant {
                columns: vec![],
                rows: vec![vec![]],
            };
        }

        // Convert scans to LIR atoms with bindings
        let lir_atoms: Vec<LirAtom> = scans.iter()
            .map(|scan| self.lower_scan(scan))
            .collect();

        if lir_atoms.len() == 1 {
            // Single scan — just a Scan node
            let atom = &lir_atoms[0];
            return LirPlan::Scan {
                relation: atom.relation.clone(),
                bindings: atom.bindings.clone(),
            };
        }

        // Multiple scans → WCO join
        // Compute variable ordering: shared variables first, then unique
        let variable_order = compute_variable_order(&lir_atoms);
        let project = collect_all_variables(&lir_atoms);

        LirPlan::WcoJoin {
            atoms: lir_atoms,
            variable_order,
            project,
        }
    }

    /// Lower a MIR scan to a LIR atom.
    fn lower_scan(&mut self, scan: &MirScan) -> LirAtom {
        let bindings = scan.args.iter().map(|term| {
            match term {
                MirTerm::Var(name) => LirBinding::Var(name.clone()),
                MirTerm::Const(c) => LirBinding::Const(lower_const(c)),
                MirTerm::Wildcard => {
                    // Wildcard → unique anonymous variable
                    LirBinding::Var(self.fresh_var())
                }
            }
        }).collect();

        LirAtom {
            relation: scan.predicate.clone(),
            bindings,
        }
    }

    /// Lower guards to LIR filter conditions.
    fn lower_guards(&self, guards: &[&MirGuard]) -> Vec<LirFilter> {
        guards.iter().map(|g| {
            LirFilter::Comparison {
                left: lower_operand(&g.left),
                op: lower_comp_op(g.op),
                right: lower_operand(&g.right),
            }
        }).collect()
    }

    /// Build an anti-join for a negated scan.
    fn build_anti_join(&mut self, positive: LirPlan, neg: &MirScan) -> LirPlan {
        // The negative side is a scan of the negated relation
        let neg_atom = self.lower_scan(neg);
        let neg_plan = LirPlan::Scan {
            relation: neg_atom.relation.clone(),
            bindings: neg_atom.bindings.clone(),
        };

        // Key columns: variables shared between the positive context and the negative scan
        let pos_vars = collect_plan_columns(&positive);
        let neg_vars: Vec<String> = neg_atom.bindings.iter()
            .filter_map(|b| b.as_var().map(|s| s.to_string()))
            .collect();
        let key_columns: Vec<String> = neg_vars.into_iter()
            .filter(|v| pos_vars.contains(v))
            .collect();

        LirPlan::AntiJoin {
            positive: Box::new(positive),
            negative: Box::new(neg_plan),
            key_columns,
        }
    }

    /// Build an aggregation node.
    fn build_aggregate(&self, input: LirPlan, agg: &MirAggregate) -> LirPlan {
        // The aggregate's sub-predicate should have been lowered as a separate
        // predicate. We add a scan of it, then aggregate.
        let sub_columns: Vec<String> = agg.group_by.iter()
            .chain(std::iter::once(&agg.agg_var))
            .cloned()
            .collect();
        let sub_bindings: Vec<LirBinding> = sub_columns.iter()
            .map(|c| LirBinding::Var(c.clone()))
            .collect();

        let sub_scan = LirPlan::Scan {
            relation: agg.sub_predicate.clone(),
            bindings: sub_bindings,
        };

        // We need to join the current input with the aggregation result.
        // The aggregate reads from the sub-predicate scan, groups by the
        // group_by columns, and produces a result column.
        let agg_plan = LirPlan::Aggregate {
            input: Box::new(sub_scan),
            group_by: agg.group_by.clone(),
            function: lower_agg_function(agg.function),
            agg_column: agg.agg_var.clone(),
            result_column: agg.result_var.clone(),
        };

        // If the input has columns that the aggregation groups by, we need
        // to join the input with the aggregation result on those columns.
        let input_vars = collect_plan_columns(&input);
        let shared: Vec<String> = agg.group_by.iter()
            .filter(|g| input_vars.contains(*g))
            .cloned()
            .collect();

        if shared.is_empty() && input_vars.is_empty() {
            // No input context — just the aggregation
            agg_plan
        } else if shared.is_empty() {
            // Cross product (rare, but possible)
            // Represent as WCO join with no shared variables
            let input_atom = plan_to_virtual_atom(&input, &mut 0);
            let agg_atom = plan_to_virtual_atom(&agg_plan, &mut 0);
            if let (Some(ia), Some(aa)) = (input_atom, agg_atom) {
                let mut all_vars = collect_plan_columns(&input);
                all_vars.push(agg.result_var.clone());
                LirPlan::WcoJoin {
                    atoms: vec![ia, aa],
                    variable_order: all_vars.clone(),
                    project: all_vars,
                }
            } else {
                // Fall back: just return the aggregation
                agg_plan
            }
        } else {
            // Join input with aggregation on shared group-by columns.
            // For now, wrap in a WCO join.
            let input_atom = plan_to_virtual_atom(&input, &mut 0);
            let agg_atom = plan_to_virtual_atom(&agg_plan, &mut 0);
            if let (Some(ia), Some(aa)) = (input_atom, agg_atom) {
                let mut all_vars = collect_plan_columns(&input);
                all_vars.push(agg.result_var.clone());
                all_vars.sort();
                all_vars.dedup();
                let variable_order = compute_variable_order(&[ia.clone(), aa.clone()]);
                LirPlan::WcoJoin {
                    atoms: vec![ia, aa],
                    variable_order,
                    project: all_vars,
                }
            } else {
                agg_plan
            }
        }
    }
}

// ============================================================
// Variable ordering for WCO join
// ============================================================

/// Compute a good variable ordering for WCO join.
///
/// Strategy: variables that appear in the most atoms come first (most
/// constrained). This is a simple heuristic; a cost-based optimizer
/// could do better.
fn compute_variable_order(atoms: &[LirAtom]) -> Vec<String> {
    let mut var_counts: HashMap<String, usize> = HashMap::new();
    for atom in atoms {
        // Count each variable once per atom (not per occurrence)
        let mut seen = HashSet::new();
        for binding in &atom.bindings {
            if let LirBinding::Var(name) = binding {
                if seen.insert(name.clone()) {
                    *var_counts.entry(name.clone()).or_insert(0) += 1;
                }
            }
        }
    }

    let mut vars: Vec<String> = var_counts.keys().cloned().collect();
    // Sort: most-shared variables first, then alphabetical for determinism
    vars.sort_by(|a, b| {
        var_counts[b].cmp(&var_counts[a])
            .then_with(|| a.cmp(b))
    });
    vars
}

/// Collect all variable names from a set of atoms.
fn collect_all_variables(atoms: &[LirAtom]) -> Vec<String> {
    let mut vars = Vec::new();
    let mut seen = HashSet::new();
    for atom in atoms {
        for binding in &atom.bindings {
            if let LirBinding::Var(name) = binding {
                if seen.insert(name.clone()) {
                    vars.push(name.clone());
                }
            }
        }
    }
    vars
}

// ============================================================
// Stratification
// ============================================================

/// Stratify LIR rules using predicate dependency analysis.
///
/// Builds a dependency graph and uses Tarjan's SCC algorithm to
/// compute strata. Negation and aggregation create "negative" edges
/// that must not form cycles.
fn stratify_rules(
    rules: &[LirRule],
    mir: &MirProgram,
) -> Result<Vec<LirStratum>, LowerError> {
    // Collect all IDB predicate names
    let idb_preds: HashSet<String> = rules.iter()
        .map(|r| r.target.clone())
        .collect();

    // Build dependency graph from the MIR (since LIR plans are trees,
    // it's easier to analyze dependencies from the MIR structure)
    let mut edges: HashMap<String, Vec<(String, DepKind)>> = HashMap::new();
    for pred in &idb_preds {
        edges.entry(pred.clone()).or_default();
    }

    for pred in &mir.predicates {
        let head = &pred.name;
        if !idb_preds.contains(head) {
            continue;
        }
        let atoms_list = match &pred.body {
            MirBody::None => continue,
            MirBody::Conjunction(atoms) => vec![atoms.as_slice()],
            MirBody::Disjunction(clauses) => clauses.iter().map(|c| c.as_slice()).collect(),
        };

        for atoms in atoms_list {
            for atom in atoms {
                match atom {
                    MirAtom::Scan(s) => {
                        if idb_preds.contains(&s.predicate) {
                            edges.entry(head.clone()).or_default()
                                .push((s.predicate.clone(), DepKind::Positive));
                        }
                    }
                    MirAtom::NegScan(s) => {
                        if idb_preds.contains(&s.predicate) {
                            edges.entry(head.clone()).or_default()
                                .push((s.predicate.clone(), DepKind::Negative));
                        }
                    }
                    MirAtom::Aggregate(agg) => {
                        if idb_preds.contains(&agg.sub_predicate) {
                            edges.entry(head.clone()).or_default()
                                .push((agg.sub_predicate.clone(), DepKind::Negative));
                        }
                    }
                    MirAtom::TypeCheck(tc) => {
                        let char_pred = &tc.type_predicate;
                        if idb_preds.contains(char_pred) {
                            edges.entry(head.clone()).or_default()
                                .push((char_pred.clone(), DepKind::Positive));
                        }
                    }
                    MirAtom::Guard(_) | MirAtom::Assign(_) => {}
                }
            }
        }
    }

    // Tarjan's SCC
    let pred_list: Vec<String> = idb_preds.iter().cloned().collect();
    let sccs = tarjan_scc(&pred_list, &edges);

    // Check for negation cycles within SCCs
    for scc in &sccs {
        let scc_set: HashSet<&String> = scc.iter().collect();
        for pred in scc {
            if let Some(deps) = edges.get(pred) {
                for (dep, kind) in deps {
                    if scc_set.contains(dep) && *kind == DepKind::Negative {
                        return Err(LowerError::NegationCycle(scc.clone()));
                    }
                }
            }
        }
    }

    // Build strata from SCCs
    let mut strata = Vec::new();
    for scc in sccs {
        let pred_set: HashSet<&str> = scc.iter().map(|s| s.as_str()).collect();

        let recursive = if scc.len() > 1 {
            true
        } else {
            let pred = &scc[0];
            edges.get(pred).map_or(false, |deps| {
                deps.iter().any(|(dep, kind)| dep == pred && *kind == DepKind::Positive)
            })
        };

        let stratum_rules: Vec<LirRule> = rules.iter()
            .filter(|r| pred_set.contains(r.target.as_str()))
            .cloned()
            .collect();

        if !stratum_rules.is_empty() {
            strata.push(LirStratum {
                rules: stratum_rules,
                recursive,
            });
        }
    }

    Ok(strata)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DepKind {
    Positive,
    Negative,
}

// ============================================================
// Tarjan's SCC
// ============================================================

struct TarjanState {
    index_counter: usize,
    stack: Vec<String>,
    on_stack: HashSet<String>,
    index: HashMap<String, usize>,
    lowlink: HashMap<String, usize>,
    result: Vec<Vec<String>>,
}

fn tarjan_scc(
    nodes: &[String],
    edges: &HashMap<String, Vec<(String, DepKind)>>,
) -> Vec<Vec<String>> {
    let mut state = TarjanState {
        index_counter: 0,
        stack: Vec::new(),
        on_stack: HashSet::new(),
        index: HashMap::new(),
        lowlink: HashMap::new(),
        result: Vec::new(),
    };

    for node in nodes {
        if !state.index.contains_key(node) {
            strongconnect(node, edges, &mut state);
        }
    }

    state.result
}

fn strongconnect(
    v: &str,
    edges: &HashMap<String, Vec<(String, DepKind)>>,
    state: &mut TarjanState,
) {
    state.index.insert(v.to_string(), state.index_counter);
    state.lowlink.insert(v.to_string(), state.index_counter);
    state.index_counter += 1;
    state.stack.push(v.to_string());
    state.on_stack.insert(v.to_string());

    if let Some(neighbors) = edges.get(v) {
        for (w, _) in neighbors {
            if !state.index.contains_key(w.as_str()) {
                strongconnect(w, edges, state);
                let w_low = state.lowlink[w.as_str()];
                let v_low = state.lowlink[v];
                if w_low < v_low {
                    state.lowlink.insert(v.to_string(), w_low);
                }
            } else if state.on_stack.contains(w.as_str()) {
                let w_idx = state.index[w.as_str()];
                let v_low = state.lowlink[v];
                if w_idx < v_low {
                    state.lowlink.insert(v.to_string(), w_idx);
                }
            }
        }
    }

    if state.lowlink[v] == state.index[v] {
        let mut scc = Vec::new();
        loop {
            let w = state.stack.pop().unwrap();
            state.on_stack.remove(&w);
            scc.push(w.clone());
            if w == v {
                break;
            }
        }
        state.result.push(scc);
    }
}

// ============================================================
// Helpers
// ============================================================

fn lower_const(c: &MirConst) -> LirValue {
    match c {
        MirConst::Int(v) => LirValue::Int(*v),
        MirConst::Float(v) => LirValue::Float(*v),
        MirConst::String(s) => LirValue::String(s.clone()),
        MirConst::Bool(b) => LirValue::Bool(*b),
    }
}

fn lower_operand(term: &MirTerm) -> LirOperand {
    match term {
        MirTerm::Var(name) => LirOperand::Column(name.clone()),
        MirTerm::Const(c) => LirOperand::Literal(lower_const(c)),
        MirTerm::Wildcard => LirOperand::Column("_".to_string()),
    }
}

fn lower_comp_op(op: MirCompOp) -> LirCompOp {
    match op {
        MirCompOp::Eq => LirCompOp::Eq,
        MirCompOp::Ne => LirCompOp::Ne,
        MirCompOp::Lt => LirCompOp::Lt,
        MirCompOp::Le => LirCompOp::Le,
        MirCompOp::Gt => LirCompOp::Gt,
        MirCompOp::Ge => LirCompOp::Ge,
    }
}

fn lower_arith_expr(expr: &MirArithExpr) -> LirExpr {
    LirExpr {
        left: lower_operand(&expr.left),
        op: lower_arith_op(expr.op),
        right: lower_operand(&expr.right),
    }
}

fn lower_arith_op(op: MirArithOp) -> LirArithOp {
    match op {
        MirArithOp::Add => LirArithOp::Add,
        MirArithOp::Sub => LirArithOp::Sub,
        MirArithOp::Mul => LirArithOp::Mul,
        MirArithOp::Div => LirArithOp::Div,
        MirArithOp::Mod => LirArithOp::Mod,
    }
}

fn lower_agg_function(f: MirAggFunction) -> LirAggFunction {
    match f {
        MirAggFunction::Count => LirAggFunction::Count,
        MirAggFunction::Sum => LirAggFunction::Sum,
        MirAggFunction::Min => LirAggFunction::Min,
        MirAggFunction::Max => LirAggFunction::Max,
        MirAggFunction::Avg => LirAggFunction::Avg,
        MirAggFunction::Concat => LirAggFunction::Concat,
        MirAggFunction::Rank => LirAggFunction::Rank,
        MirAggFunction::StrictCount => LirAggFunction::StrictCount,
        MirAggFunction::StrictSum => LirAggFunction::StrictSum,
        MirAggFunction::StrictConcat => LirAggFunction::StrictConcat,
        MirAggFunction::Any => LirAggFunction::Any,
    }
}

/// Collect all column/variable names produced by a plan.
fn collect_plan_columns(plan: &LirPlan) -> Vec<String> {
    match plan {
        LirPlan::Scan { bindings, .. } => {
            bindings.iter()
                .filter_map(|b| b.as_var().map(|s| s.to_string()))
                .collect()
        }
        LirPlan::WcoJoin { project, .. } => project.clone(),
        LirPlan::Filter { input, .. } => collect_plan_columns(input),
        LirPlan::Project { columns, .. } => columns.clone(),
        LirPlan::Union { inputs } => {
            if let Some(first) = inputs.first() {
                collect_plan_columns(first)
            } else {
                vec![]
            }
        }
        LirPlan::AntiJoin { positive, .. } => collect_plan_columns(positive),
        LirPlan::Aggregate { group_by, result_column, .. } => {
            let mut cols = group_by.clone();
            cols.push(result_column.clone());
            cols
        }
        LirPlan::Extend { input, column, .. } => {
            let mut cols = collect_plan_columns(input);
            cols.push(column.clone());
            cols
        }
        LirPlan::Constant { columns, .. } => columns.clone(),
    }
}

/// Try to extract a virtual LirAtom from a plan (for building compound WCO joins).
/// Only works for simple Scan plans.
fn plan_to_virtual_atom(plan: &LirPlan, _counter: &mut u32) -> Option<LirAtom> {
    match plan {
        LirPlan::Scan { relation, bindings } => {
            Some(LirAtom {
                relation: relation.clone(),
                bindings: bindings.clone(),
            })
        }
        _ => None,
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    fn simple_pred(name: &str, params: Vec<(&str, MirType)>, atoms: Vec<MirAtom>) -> MirPredicate {
        let params = params.into_iter().map(|(n, t)| MirParam::new(n, t)).collect();
        MirPredicate::new(name, params, atoms)
    }

    #[test]
    fn lower_single_scan() {
        // predicate p(int x) { val(x) }
        let mir = MirProgram {
            predicates: vec![
                simple_pred("p", vec![("x", MirType::Int)], vec![
                    MirAtom::Scan(MirScan::new("val", vec![MirTerm::var("x")])),
                ]),
            ],
        };

        let lir = lower_mir(&mir).unwrap();
        assert_eq!(lir.strata.len(), 1);
        assert_eq!(lir.strata[0].rules.len(), 1);
        assert!(!lir.strata[0].recursive);

        let rule = &lir.strata[0].rules[0];
        assert_eq!(rule.target, "p");
        assert!(matches!(&rule.body, LirPlan::Scan { relation, .. } if relation == "val"));
    }

    #[test]
    fn lower_join_two_scans() {
        // predicate p(int x, int y) { a(x) and b(x, y) }
        let mir = MirProgram {
            predicates: vec![
                simple_pred("p", vec![("x", MirType::Int), ("y", MirType::Int)], vec![
                    MirAtom::Scan(MirScan::new("a", vec![MirTerm::var("x")])),
                    MirAtom::Scan(MirScan::new("b", vec![MirTerm::var("x"), MirTerm::var("y")])),
                ]),
            ],
        };

        let lir = lower_mir(&mir).unwrap();
        let rule = &lir.strata[0].rules[0];
        // Should be a WCO join of a(x) and b(x, y)
        match &rule.body {
            LirPlan::Project { input, columns } => {
                assert_eq!(columns, &["x", "y"]);
                match input.as_ref() {
                    LirPlan::WcoJoin { atoms, variable_order, .. } => {
                        assert_eq!(atoms.len(), 2);
                        assert_eq!(atoms[0].relation, "a");
                        assert_eq!(atoms[1].relation, "b");
                        // x should come first (shared variable)
                        assert_eq!(variable_order[0], "x");
                    }
                    other => panic!("expected WcoJoin, got {:?}", other),
                }
            }
            LirPlan::WcoJoin { atoms, variable_order, .. } => {
                assert_eq!(atoms.len(), 2);
                assert_eq!(variable_order[0], "x");
            }
            other => panic!("expected Project or WcoJoin, got {:?}", other),
        }
    }

    #[test]
    fn lower_with_guard() {
        // predicate p(int x) { val(x) and x > 0 }
        let mir = MirProgram {
            predicates: vec![
                simple_pred("p", vec![("x", MirType::Int)], vec![
                    MirAtom::Scan(MirScan::new("val", vec![MirTerm::var("x")])),
                    MirAtom::Guard(MirGuard {
                        left: MirTerm::var("x"),
                        op: MirCompOp::Gt,
                        right: MirTerm::int(0),
                    }),
                ]),
            ],
        };

        let lir = lower_mir(&mir).unwrap();
        let rule = &lir.strata[0].rules[0];
        // Should be: Filter(Scan(val), x > 0)
        match &rule.body {
            LirPlan::Filter { input, condition } => {
                assert!(matches!(input.as_ref(), LirPlan::Scan { relation, .. } if relation == "val"));
                assert!(matches!(condition, LirFilter::Comparison { op: LirCompOp::Gt, .. }));
            }
            other => panic!("expected Filter, got {:?}", other),
        }
    }

    #[test]
    fn lower_with_assignment() {
        // int doubled(int x) { val(x) and result = x * 2 }
        let mir = MirProgram {
            predicates: vec![
                simple_pred("doubled", vec![("x", MirType::Int), ("result", MirType::Int)], vec![
                    MirAtom::Scan(MirScan::new("val", vec![MirTerm::var("x")])),
                    MirAtom::Assign(MirAssign {
                        result_var: "result".to_string(),
                        expr: MirArithExpr {
                            left: MirTerm::var("x"),
                            op: MirArithOp::Mul,
                            right: MirTerm::int(2),
                        },
                    }),
                ]),
            ],
        };

        let lir = lower_mir(&mir).unwrap();
        let rule = &lir.strata[0].rules[0];
        // Should be: Extend(Scan(val), result = x * 2)
        match &rule.body {
            LirPlan::Extend { input, column, expr } => {
                assert_eq!(column, "result");
                assert!(matches!(input.as_ref(), LirPlan::Scan { .. }));
                assert!(matches!(expr.op, LirArithOp::Mul));
            }
            other => panic!("expected Extend, got {:?}", other),
        }
    }

    #[test]
    fn lower_negation_anti_join() {
        // predicate nonsink(int x) { node(x) and not sink(x) }
        let mir = MirProgram {
            predicates: vec![
                simple_pred("nonsink", vec![("x", MirType::Int)], vec![
                    MirAtom::Scan(MirScan::new("node", vec![MirTerm::var("x")])),
                    MirAtom::NegScan(MirScan::new("sink", vec![MirTerm::var("x")])),
                ]),
            ],
        };

        let lir = lower_mir(&mir).unwrap();
        let rule = &lir.strata[0].rules[0];
        // Should be: AntiJoin(Scan(node), Scan(sink), key=[x])
        match &rule.body {
            LirPlan::AntiJoin { positive, negative, key_columns } => {
                assert!(matches!(positive.as_ref(), LirPlan::Scan { relation, .. } if relation == "node"));
                assert!(matches!(negative.as_ref(), LirPlan::Scan { relation, .. } if relation == "sink"));
                assert_eq!(key_columns, &["x"]);
            }
            other => panic!("expected AntiJoin, got {:?}", other),
        }
    }

    #[test]
    fn lower_recursive_stratum() {
        // predicate path(int a, int b) {
        //     edge(a, b) or exists(int mid | edge(a, mid) and path(mid, b))
        // }
        let mir = MirProgram {
            predicates: vec![
                MirPredicate {
                    name: "path".to_string(),
                    params: vec![MirParam::new("a", MirType::Int), MirParam::new("b", MirType::Int)],
                    body: MirBody::Disjunction(vec![
                        vec![MirAtom::Scan(MirScan::new("edge", vec![MirTerm::var("a"), MirTerm::var("b")]))],
                        vec![
                            MirAtom::Scan(MirScan::new("edge", vec![MirTerm::var("a"), MirTerm::var("mid")])),
                            MirAtom::Scan(MirScan::new("path", vec![MirTerm::var("mid"), MirTerm::var("b")])),
                        ],
                    ]),
                    annotations: MirAnnotations::default(),
                    is_abstract: false,
                },
            ],
        };

        let lir = lower_mir(&mir).unwrap();
        assert_eq!(lir.strata.len(), 1);
        assert!(lir.strata[0].recursive);
        assert_eq!(lir.strata[0].rules.len(), 2);
    }

    #[test]
    fn lower_stratified_negation() {
        // predicate a(int x) { val(x) }
        // predicate b(int x) { val(x) and not a(x) }
        let mir = MirProgram {
            predicates: vec![
                simple_pred("a", vec![("x", MirType::Int)], vec![
                    MirAtom::Scan(MirScan::new("val", vec![MirTerm::var("x")])),
                ]),
                simple_pred("b", vec![("x", MirType::Int)], vec![
                    MirAtom::Scan(MirScan::new("val", vec![MirTerm::var("x")])),
                    MirAtom::NegScan(MirScan::new("a", vec![MirTerm::var("x")])),
                ]),
            ],
        };

        let lir = lower_mir(&mir).unwrap();
        // Should be 2 strata: a first, then b (b negates a)
        assert_eq!(lir.strata.len(), 2);
        // First stratum should contain 'a'
        assert!(lir.strata[0].rules.iter().any(|r| r.target == "a"));
        // Second stratum should contain 'b'
        assert!(lir.strata[1].rules.iter().any(|r| r.target == "b"));
    }

    #[test]
    fn lower_negation_cycle_error() {
        // predicate p(int x) { not q(x) }
        // predicate q(int x) { not p(x) }
        let mir = MirProgram {
            predicates: vec![
                simple_pred("p", vec![("x", MirType::Int)], vec![
                    MirAtom::NegScan(MirScan::new("q", vec![MirTerm::var("x")])),
                ]),
                simple_pred("q", vec![("x", MirType::Int)], vec![
                    MirAtom::NegScan(MirScan::new("p", vec![MirTerm::var("x")])),
                ]),
            ],
        };

        let result = lower_mir(&mir);
        assert!(result.is_err());
    }

    #[test]
    fn lower_disjunction() {
        // predicate ab(int x) { a(x) or b(x) }
        let mir = MirProgram {
            predicates: vec![
                MirPredicate {
                    name: "ab".to_string(),
                    params: vec![MirParam::new("x", MirType::Int)],
                    body: MirBody::Disjunction(vec![
                        vec![MirAtom::Scan(MirScan::new("a", vec![MirTerm::var("x")]))],
                        vec![MirAtom::Scan(MirScan::new("b", vec![MirTerm::var("x")]))],
                    ]),
                    annotations: MirAnnotations::default(),
                    is_abstract: false,
                },
            ],
        };

        let lir = lower_mir(&mir).unwrap();
        // Disjunction → 2 rules for the same target
        assert_eq!(lir.strata[0].rules.len(), 2);
        assert!(lir.strata[0].rules.iter().all(|r| r.target == "ab"));
    }

    #[test]
    fn lower_wildcard_scan() {
        // predicate first(int x) { rel(x, _, _) }
        let mir = MirProgram {
            predicates: vec![
                simple_pred("first", vec![("x", MirType::Int)], vec![
                    MirAtom::Scan(MirScan::new("rel", vec![
                        MirTerm::var("x"),
                        MirTerm::Wildcard,
                        MirTerm::Wildcard,
                    ])),
                ]),
            ],
        };

        let lir = lower_mir(&mir).unwrap();
        let rule = &lir.strata[0].rules[0];
        // Wildcards should become unique fresh variables
        match &rule.body {
            LirPlan::Project { input, .. } => {
                match input.as_ref() {
                    LirPlan::Scan { bindings, .. } => {
                        assert_eq!(bindings.len(), 3);
                        // First binding is x
                        assert!(matches!(&bindings[0], LirBinding::Var(n) if n == "x"));
                        // Second and third are distinct anonymous vars
                        let n1 = bindings[1].as_var().unwrap();
                        let n2 = bindings[2].as_var().unwrap();
                        assert!(n1.starts_with("_lir"));
                        assert!(n2.starts_with("_lir"));
                        assert_ne!(n1, n2);
                    }
                    _ => panic!("expected Scan"),
                }
            }
            LirPlan::Scan { bindings, .. } => {
                // If no projection needed
                assert_eq!(bindings.len(), 3);
            }
            other => panic!("expected Scan or Project, got {:?}", other),
        }
    }

    #[test]
    fn lower_constant_binding() {
        // predicate p(int x) { rel(x, 42) }
        let mir = MirProgram {
            predicates: vec![
                simple_pred("p", vec![("x", MirType::Int)], vec![
                    MirAtom::Scan(MirScan::new("rel", vec![
                        MirTerm::var("x"),
                        MirTerm::int(42),
                    ])),
                ]),
            ],
        };

        let lir = lower_mir(&mir).unwrap();
        let rule = &lir.strata[0].rules[0];
        // Navigate to the innermost Scan
        let scan = match &rule.body {
            LirPlan::Project { input, .. } => input.as_ref(),
            other => other,
        };
        if let LirPlan::Scan { bindings, .. } = scan {
            assert!(matches!(&bindings[0], LirBinding::Var(n) if n == "x"));
            assert!(matches!(&bindings[1], LirBinding::Const(LirValue::Int(42))));
        } else {
            panic!("expected Scan, got {:?}", scan);
        }
    }

    #[test]
    fn lower_abstract_predicate_no_rules() {
        let mir = MirProgram {
            predicates: vec![
                MirPredicate::abstract_pred("abs", vec![MirParam::new("x", MirType::Int)]),
            ],
        };

        let lir = lower_mir(&mir).unwrap();
        assert!(lir.strata.is_empty());
    }

    #[test]
    fn lower_type_check() {
        // predicate p(int x) { SmallNum#char(x) and x > 0 }
        // (TypeCheck is sugar for a scan on the char predicate)
        let mir = MirProgram {
            predicates: vec![
                simple_pred("p", vec![("x", MirType::Int)], vec![
                    MirAtom::TypeCheck(MirTypeCheck {
                        var: "x".to_string(),
                        type_predicate: "SmallNum#char".to_string(),
                    }),
                    MirAtom::Guard(MirGuard {
                        left: MirTerm::var("x"),
                        op: MirCompOp::Gt,
                        right: MirTerm::int(0),
                    }),
                ]),
            ],
        };

        let lir = lower_mir(&mir).unwrap();
        let rule = &lir.strata[0].rules[0];
        // Should be: Filter(Scan(SmallNum#char, [x]), x > 0)
        match &rule.body {
            LirPlan::Filter { input, .. } => {
                match input.as_ref() {
                    LirPlan::Scan { relation, .. } => {
                        assert_eq!(relation, "SmallNum#char");
                    }
                    other => panic!("expected Scan, got {:?}", other),
                }
            }
            other => panic!("expected Filter, got {:?}", other),
        }
    }

    #[test]
    fn program_metadata() {
        let mir = MirProgram {
            predicates: vec![
                simple_pred("a", vec![("x", MirType::Int)], vec![
                    MirAtom::Scan(MirScan::new("val", vec![MirTerm::var("x")])),
                ]),
                simple_pred("b", vec![("x", MirType::Int)], vec![
                    MirAtom::Scan(MirScan::new("val", vec![MirTerm::var("x")])),
                ]),
            ],
        };

        let lir = lower_mir(&mir).unwrap();
        let rels = lir.defined_relations();
        assert!(rels.contains(&"a"));
        assert!(rels.contains(&"b"));
        assert_eq!(lir.rule_count(), 2);
    }
}
