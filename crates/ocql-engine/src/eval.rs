//! Semi-naive Datalog evaluator.
//!
//! Evaluates a stratified Datalog program against a database to a fixed point.
//! Uses semi-naive evaluation for recursive strata: in each iteration, at least
//! one body atom must bind from the delta (newly derived) tuples.

use std::collections::HashMap;

use ocql_database::{Database, RelationSchema, ColumnDef, Tuple, Value};
use ocql_schema::ColumnType;
use smallvec::SmallVec;

use crate::rule::{Atom, BodyElement, CompOp, Guard, Program, Rule, Term, AggFunction};
use crate::stratify::{Stratum, stratify};

/// Evaluation error.
#[derive(Debug)]
pub enum EvalError {
    Stratification(crate::stratify::StratificationError),
    UnknownRelation(String),
    UnboundVariable(String),
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::Stratification(e) => write!(f, "stratification error: {}", e),
            EvalError::UnknownRelation(r) => write!(f, "unknown relation: {}", r),
            EvalError::UnboundVariable(v) => write!(f, "unbound variable: {}", v),
        }
    }
}

impl std::error::Error for EvalError {}

/// Variable bindings during rule evaluation.
type Bindings = HashMap<String, Value>;

/// Evaluate a Datalog program against a database.
///
/// Creates output relations for all IDB predicates and evaluates rules
/// to a fixed point. Returns the names of all IDB relations created.
pub fn evaluate(program: &Program, db: &mut Database) -> Result<Vec<String>, EvalError> {
    let strata = stratify(program).map_err(EvalError::Stratification)?;

    // Ensure output relations exist for all IDB predicates
    let idb_names = program.head_predicates();
    for rule in &program.rules {
        ensure_relation_for_head(db, rule);
    }

    for stratum in &strata {
        if stratum.is_recursive {
            evaluate_recursive(stratum, db)?;
        } else {
            evaluate_nonrecursive(stratum, db)?;
        }
    }

    Ok(idb_names)
}

/// Ensure a relation exists in the database for a rule's head predicate.
/// Infers the schema from the head atom's arity.
fn ensure_relation_for_head(db: &mut Database, rule: &Rule) {
    let name = &rule.head.predicate;
    if db.relation(name).is_some() {
        return;
    }

    let columns: Vec<ColumnDef> = rule.head.terms.iter().enumerate()
        .map(|(i, term)| {
            let col_name = match term {
                Term::Var(v) => v.clone(),
                Term::Const(_) | Term::StrLit(_) => format!("c{}", i),
            };
            ColumnDef {
                name: col_name,
                col_type: ColumnType::Int, // generic; actual types come from data
            }
        })
        .collect();

    let schema = RelationSchema {
        name: name.clone(),
        columns,
    };
    db.add_relation(name, schema);
}

/// Evaluate non-recursive rules (single pass).
fn evaluate_nonrecursive(stratum: &Stratum, db: &mut Database) -> Result<(), EvalError> {
    for rule in &stratum.rules {
        let new_tuples = evaluate_rule(rule, db)?;
        let name = &rule.head.predicate;
        for tuple in new_tuples {
            db.insert(name, tuple).map_err(|_| EvalError::UnknownRelation(name.clone()))?;
        }
    }
    Ok(())
}

/// Evaluate recursive rules to a fixed point using semi-naive evaluation.
fn evaluate_recursive(stratum: &Stratum, db: &mut Database) -> Result<(), EvalError> {
    // Initial evaluation: run all rules once to seed the delta
    let mut deltas: HashMap<String, Vec<Tuple>> = HashMap::new();

    for rule in &stratum.rules {
        let new_tuples = evaluate_rule(rule, db)?;
        let name = &rule.head.predicate;
        for tuple in new_tuples {
            if db.insert(name, tuple.clone())
                .map_err(|_| EvalError::UnknownRelation(name.clone()))? {
                deltas.entry(name.clone()).or_default().push(tuple);
            }
        }
    }

    // Semi-naive loop
    loop {
        if deltas.values().all(|d| d.is_empty()) {
            break;
        }

        let mut new_deltas: HashMap<String, Vec<Tuple>> = HashMap::new();

        for rule in &stratum.rules {
            // Find which body atoms reference recursive predicates
            let recursive_atoms: Vec<usize> = rule.body.iter().enumerate()
                .filter_map(|(i, elem)| {
                    if let BodyElement::Positive(atom) = elem {
                        if stratum.predicates.contains(&atom.predicate)
                            && deltas.contains_key(&atom.predicate) {
                            return Some(i);
                        }
                    }
                    None
                })
                .collect();

            // For semi-naive: evaluate with each recursive atom using delta
            for &delta_idx in &recursive_atoms {
                let new_tuples = evaluate_rule_with_delta(rule, db, &deltas, delta_idx)?;
                let name = &rule.head.predicate;
                for tuple in new_tuples {
                    if db.insert(name, tuple.clone())
                        .map_err(|_| EvalError::UnknownRelation(name.clone()))? {
                        new_deltas.entry(name.clone()).or_default().push(tuple);
                    }
                }
            }
        }

        deltas = new_deltas;
    }

    Ok(())
}

/// Evaluate a rule, returning all tuples that satisfy the body.
fn evaluate_rule(rule: &Rule, db: &Database) -> Result<Vec<Tuple>, EvalError> {
    let bindings_list = evaluate_body(&rule.body, db, &HashMap::new())?;
    let mut results = Vec::new();
    for bindings in &bindings_list {
        if let Some(tuple) = project_head(&rule.head, bindings) {
            results.push(tuple);
        }
    }
    Ok(results)
}

/// Evaluate a rule with one body atom using delta tuples (semi-naive).
fn evaluate_rule_with_delta(
    rule: &Rule,
    db: &Database,
    deltas: &HashMap<String, Vec<Tuple>>,
    delta_idx: usize,
) -> Result<Vec<Tuple>, EvalError> {
    // Process body elements, using delta for the atom at delta_idx
    let mut current_bindings = vec![HashMap::new()];

    for (i, elem) in rule.body.iter().enumerate() {
        let mut next_bindings = Vec::new();

        for bindings in &current_bindings {
            match elem {
                BodyElement::Positive(atom) => {
                    if i == delta_idx {
                        // Use delta tuples instead of full relation
                        if let Some(delta_tuples) = deltas.get(&atom.predicate) {
                            for tuple in delta_tuples {
                                if let Some(new_bindings) = match_atom_tuple(atom, tuple, bindings) {
                                    next_bindings.push(new_bindings);
                                }
                            }
                        }
                    } else {
                        // Use full relation
                        let matches = match_atom(atom, db, bindings)?;
                        next_bindings.extend(matches);
                    }
                }
                BodyElement::Negated(atom) => {
                    if !has_any_match(atom, db, bindings)? {
                        next_bindings.push(bindings.clone());
                    }
                }
                BodyElement::Guard(guard) => {
                    if let Some(new_b) = eval_guard_binding(guard, bindings) {
                        next_bindings.push(new_b);
                    }
                }
                BodyElement::Assign { result_var, expr } => {
                    if let Some(val) = eval_arith(expr, bindings) {
                        let mut new_bindings = bindings.clone();
                        new_bindings.insert(result_var.clone(), val);
                        next_bindings.push(new_bindings);
                    }
                }
                BodyElement::Aggregate { result_var, function, sub_rule, group_by, agg_var } => {
                    let agg_result = eval_aggregate(
                        function, sub_rule, group_by, agg_var, db, bindings,
                    )?;
                    let mut new_bindings = bindings.clone();
                    new_bindings.insert(result_var.clone(), agg_result);
                    next_bindings.push(new_bindings);
                }
            }
        }

        current_bindings = next_bindings;
        if current_bindings.is_empty() {
            break;
        }
    }

    let mut results = Vec::new();
    for bindings in &current_bindings {
        if let Some(tuple) = project_head(&rule.head, bindings) {
            results.push(tuple);
        }
    }
    Ok(results)
}

/// Evaluate a body (list of body elements), threading bindings through.
fn evaluate_body(
    body: &[BodyElement],
    db: &Database,
    initial: &Bindings,
) -> Result<Vec<Bindings>, EvalError> {
    let mut current = vec![initial.clone()];

    for elem in body {
        let mut next = Vec::new();

        for bindings in &current {
            match elem {
                BodyElement::Positive(atom) => {
                    let matches = match_atom(atom, db, bindings)?;
                    next.extend(matches);
                }
                BodyElement::Negated(atom) => {
                    if !has_any_match(atom, db, bindings)? {
                        next.push(bindings.clone());
                    }
                }
                BodyElement::Guard(guard) => {
                    if let Some(new_b) = eval_guard_binding(guard, bindings) {
                        next.push(new_b);
                    }
                }
                BodyElement::Assign { result_var, expr } => {
                    if let Some(val) = eval_arith(expr, bindings) {
                        let mut new_bindings = bindings.clone();
                        new_bindings.insert(result_var.clone(), val);
                        next.push(new_bindings);
                    }
                }
                BodyElement::Aggregate { result_var, function, sub_rule, group_by, agg_var } => {
                    let agg_result = eval_aggregate(
                        function, sub_rule, group_by, agg_var, db, bindings,
                    )?;
                    let mut new_bindings = bindings.clone();
                    new_bindings.insert(result_var.clone(), agg_result);
                    next.push(new_bindings);
                }
            }
        }

        current = next;
        if current.is_empty() {
            break;
        }
    }

    Ok(current)
}

/// Match an atom against a database relation, extending bindings.
///
/// Uses index-accelerated lookup when some columns are already bound
/// (from constants in the atom or variables already in bindings).
/// Falls back to a full scan when no columns are bound.
fn match_atom(
    atom: &Atom,
    db: &Database,
    bindings: &Bindings,
) -> Result<Vec<Bindings>, EvalError> {
    let rel = db.relation(&atom.predicate)
        .ok_or_else(|| EvalError::UnknownRelation(atom.predicate.clone()))?;

    // Determine which columns have known values (bound vars or constants)
    let mut bound_cols: SmallVec<[usize; 4]> = SmallVec::new();
    let mut bound_vals: SmallVec<[Value; 4]> = SmallVec::new();
    for (i, term) in atom.terms.iter().enumerate() {
        match term {
            Term::Const(v) => {
                bound_cols.push(i);
                bound_vals.push(v.clone());
            }
            Term::Var(name) => {
                if let Some(val) = bindings.get(name) {
                    bound_cols.push(i);
                    bound_vals.push(val.clone());
                }
            }
            Term::StrLit(_) => {
                panic!("unresolved string literal; call program.resolve_strings() first")
            }
        }
    }

    let mut results = Vec::new();

    if bound_cols.is_empty() {
        // No bound columns — full scan (can't narrow with an index)
        for tuple in rel.scan() {
            if let Some(new_bindings) = match_atom_tuple(atom, tuple, bindings) {
                results.push(new_bindings);
            }
        }
    } else {
        // Use indexed lookup to only examine matching tuples
        rel.lookup_each(&bound_cols, &bound_vals, |tuple| {
            if let Some(new_bindings) = match_atom_tuple(atom, tuple, bindings) {
                results.push(new_bindings);
            }
        });
    }

    Ok(results)
}

/// Try to match an atom against a single tuple, extending bindings.
/// Returns None if the match fails.
fn match_atom_tuple(atom: &Atom, tuple: &Tuple, bindings: &Bindings) -> Option<Bindings> {
    if atom.terms.len() != tuple.len() {
        return None;
    }

    let mut new_bindings = bindings.clone();
    for (term, value) in atom.terms.iter().zip(tuple.iter()) {
        match term {
            Term::Var(name) => {
                if let Some(existing) = new_bindings.get(name) {
                    if existing != value {
                        return None; // Binding conflict
                    }
                } else {
                    new_bindings.insert(name.clone(), value.clone());
                }
            }
            Term::Const(c) => {
                if c != value {
                    return None; // Constant doesn't match
                }
            }
            Term::StrLit(_) => {
                panic!("unresolved string literal; call program.resolve_strings() first")
            }
        }
    }
    Some(new_bindings)
}

/// Check if an atom has any match in the database (for negation).
///
/// Uses indexed lookup when possible for O(1) existence checks.
fn has_any_match(atom: &Atom, db: &Database, bindings: &Bindings) -> Result<bool, EvalError> {
    let rel = match db.relation(&atom.predicate) {
        Some(r) => r,
        None => return Ok(false), // Non-existent relation = empty = no match
    };

    // Determine bound columns
    let mut bound_cols: SmallVec<[usize; 4]> = SmallVec::new();
    let mut bound_vals: SmallVec<[Value; 4]> = SmallVec::new();
    let mut all_bound = true;
    for (i, term) in atom.terms.iter().enumerate() {
        match term {
            Term::Const(v) => {
                bound_cols.push(i);
                bound_vals.push(v.clone());
            }
            Term::Var(name) => {
                if let Some(val) = bindings.get(name) {
                    bound_cols.push(i);
                    bound_vals.push(val.clone());
                } else {
                    all_bound = false;
                }
            }
            Term::StrLit(_) => {
                panic!("unresolved string literal; call program.resolve_strings() first")
            }
        }
    }

    if bound_cols.is_empty() {
        // No bound columns — just check if relation is non-empty
        return Ok(!rel.is_empty());
    }

    if all_bound {
        // All columns are bound — O(1) existence check via index
        return Ok(rel.lookup_any(&bound_cols, &bound_vals));
    }

    // Some columns bound — use index to narrow, then check remaining
    let mut found = false;
    rel.lookup_each(&bound_cols, &bound_vals, |tuple| {
        if !found {
            if match_atom_tuple(atom, tuple, bindings).is_some() {
                found = true;
            }
        }
    });
    Ok(found)
}

/// Evaluate a guard condition against current bindings.
/// Evaluate a guard, supporting variable binding for Eq guards.
///
/// When one side of an Eq guard is an unbound variable and the other side
/// is bound, binds the unbound variable to the value. This enables patterns
/// like `label = 100` where `label` hasn't been bound yet.
fn eval_guard_binding(guard: &Guard, bindings: &Bindings) -> Option<Bindings> {
    let left = resolve_term(&guard.left, bindings);
    let right = resolve_term(&guard.right, bindings);

    match (left, right) {
        (Some(l), Some(r)) => {
            let pass = match guard.op {
                CompOp::Eq => l == r,
                CompOp::Ne => l != r,
                CompOp::Lt => l < r,
                CompOp::Le => l <= r,
                CompOp::Gt => l > r,
                CompOp::Ge => l >= r,
            };
            if pass { Some(bindings.clone()) } else { None }
        }
        (None, Some(r)) if guard.op == CompOp::Eq => {
            // Bind left variable to right value
            if let Term::Var(name) = &guard.left {
                let mut new_bindings = bindings.clone();
                new_bindings.insert(name.clone(), r);
                Some(new_bindings)
            } else {
                None
            }
        }
        (Some(l), None) if guard.op == CompOp::Eq => {
            // Bind right variable to left value
            if let Term::Var(name) = &guard.right {
                let mut new_bindings = bindings.clone();
                new_bindings.insert(name.clone(), l);
                Some(new_bindings)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Evaluate an arithmetic expression, returning the computed value.
/// Returns None if operands are unbound or types are incompatible.
fn eval_arith(expr: &crate::rule::ArithExpr, bindings: &Bindings) -> Option<Value> {
    use crate::rule::ArithOp;

    let left = resolve_term(&expr.left, bindings)?;
    let right = resolve_term(&expr.right, bindings)?;

    match (&left, &right) {
        (Value::Int(a), Value::Int(b)) => {
            let result = match expr.op {
                ArithOp::Add => a.checked_add(*b)?,
                ArithOp::Sub => a.checked_sub(*b)?,
                ArithOp::Mul => a.checked_mul(*b)?,
                ArithOp::Div => {
                    if *b == 0 { return None; }
                    a.checked_div(*b)?
                }
                ArithOp::Mod => {
                    if *b == 0 { return None; }
                    a.checked_rem(*b)?
                }
            };
            Some(Value::Int(result))
        }
        (Value::Float(a), Value::Float(b)) => {
            let result = match expr.op {
                ArithOp::Add => a.into_inner() + b.into_inner(),
                ArithOp::Sub => a.into_inner() - b.into_inner(),
                ArithOp::Mul => a.into_inner() * b.into_inner(),
                ArithOp::Div => {
                    if b.into_inner() == 0.0 { return None; }
                    a.into_inner() / b.into_inner()
                }
                ArithOp::Mod => {
                    if b.into_inner() == 0.0 { return None; }
                    a.into_inner() % b.into_inner()
                }
            };
            Some(Value::Float(ordered_float::OrderedFloat(result)))
        }
        // Mixed int/float: promote int to float
        (Value::Int(a), Value::Float(b)) => {
            let a = *a as f64;
            let b = b.into_inner();
            let result = match expr.op {
                ArithOp::Add => a + b,
                ArithOp::Sub => a - b,
                ArithOp::Mul => a * b,
                ArithOp::Div => { if b == 0.0 { return None; } a / b }
                ArithOp::Mod => { if b == 0.0 { return None; } a % b }
            };
            Some(Value::Float(ordered_float::OrderedFloat(result)))
        }
        (Value::Float(a), Value::Int(b)) => {
            let a = a.into_inner();
            let b = *b as f64;
            let result = match expr.op {
                ArithOp::Add => a + b,
                ArithOp::Sub => a - b,
                ArithOp::Mul => a * b,
                ArithOp::Div => { if b == 0.0 { return None; } a / b }
                ArithOp::Mod => { if b == 0.0 { return None; } a % b }
            };
            Some(Value::Float(ordered_float::OrderedFloat(result)))
        }
        // String concatenation with Add
        (Value::String(_), Value::String(_)) if expr.op == ArithOp::Add => {
            // String concatenation not supported without StringInterner access
            None
        }
        _ => None, // Type mismatch
    }
}

/// Resolve a term to a value using current bindings.
fn resolve_term(term: &Term, bindings: &Bindings) -> Option<Value> {
    match term {
        Term::Var(name) => bindings.get(name).cloned(),
        Term::Const(v) => Some(v.clone()),
        Term::StrLit(_) => {
            panic!("unresolved string literal in evaluation; call program.resolve_strings() first")
        }
    }
}

/// Project head atom terms to a tuple using current bindings.
fn project_head(head: &Atom, bindings: &Bindings) -> Option<Tuple> {
    let mut tuple: Tuple = SmallVec::new();
    for term in &head.terms {
        match resolve_term(term, bindings) {
            Some(v) => tuple.push(v),
            None => return None, // Unbound head variable
        }
    }
    Some(tuple)
}

/// Evaluate an aggregate over a sub-rule.
fn eval_aggregate(
    function: &AggFunction,
    sub_rule: &Rule,
    _group_by: &[String],
    agg_var: &str,
    db: &Database,
    bindings: &Bindings,
) -> Result<Value, EvalError> {
    let sub_results = evaluate_body(&sub_rule.body, db, bindings)?;

    match function {
        AggFunction::Count => Ok(Value::Int(sub_results.len() as i64)),
        AggFunction::Sum => {
            let sum: i64 = sub_results.iter()
                .filter_map(|b| b.get(agg_var)?.as_int())
                .sum();
            Ok(Value::Int(sum))
        }
        AggFunction::Min => {
            let min = sub_results.iter()
                .filter_map(|b| b.get(agg_var).cloned())
                .min();
            Ok(min.unwrap_or(Value::Null))
        }
        AggFunction::Max => {
            let max = sub_results.iter()
                .filter_map(|b| b.get(agg_var).cloned())
                .max();
            Ok(max.unwrap_or(Value::Null))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule::{var, int, Atom, Rule, Program};
    use smallvec::smallvec;

    /// Helper: create a database with an "edge" relation and populate it.
    fn make_graph_db() -> Database {
        let mut db = Database::empty();
        db.add_relation("edge", RelationSchema {
            name: "edge".to_string(),
            columns: vec![
                ColumnDef { name: "src".to_string(), col_type: ColumnType::Int },
                ColumnDef { name: "dst".to_string(), col_type: ColumnType::Int },
            ],
        });
        // Graph: 1 -> 2 -> 3 -> 4
        db.insert("edge", smallvec![Value::Int(1), Value::Int(2)]).unwrap();
        db.insert("edge", smallvec![Value::Int(2), Value::Int(3)]).unwrap();
        db.insert("edge", smallvec![Value::Int(3), Value::Int(4)]).unwrap();
        db
    }

    #[test]
    fn test_simple_copy_rule() {
        // path(x, y) :- edge(x, y).
        let mut db = make_graph_db();
        let program = Program::new(vec![
            Rule::new(
                Atom::new("path", vec![var("x"), var("y")]),
                vec![BodyElement::Positive(Atom::new("edge", vec![var("x"), var("y")]))],
            ),
        ]);

        evaluate(&program, &mut db).unwrap();
        assert_eq!(db.relation("path").unwrap().len(), 3);
    }

    #[test]
    fn test_transitive_closure() {
        // path(x, y) :- edge(x, y).
        // path(x, y) :- path(x, z), edge(z, y).
        let mut db = make_graph_db();
        let program = Program::new(vec![
            Rule::new(
                Atom::new("path", vec![var("x"), var("y")]),
                vec![BodyElement::Positive(Atom::new("edge", vec![var("x"), var("y")]))],
            ),
            Rule::new(
                Atom::new("path", vec![var("x"), var("y")]),
                vec![
                    BodyElement::Positive(Atom::new("path", vec![var("x"), var("z")])),
                    BodyElement::Positive(Atom::new("edge", vec![var("z"), var("y")])),
                ],
            ),
        ]);

        evaluate(&program, &mut db).unwrap();

        // Graph: 1->2->3->4
        // Paths: (1,2), (2,3), (3,4), (1,3), (2,4), (1,4) = 6
        let path = db.relation("path").unwrap();
        assert_eq!(path.len(), 6);

        // Check specific paths
        let tuples: Vec<_> = path.scan().collect();
        assert!(tuples.contains(&&smallvec![Value::Int(1), Value::Int(4)]));
        assert!(tuples.contains(&&smallvec![Value::Int(1), Value::Int(3)]));
    }

    #[test]
    fn test_guard_filter() {
        // big_edge(x, y) :- edge(x, y), x > 1.
        let mut db = make_graph_db();
        let program = Program::new(vec![
            Rule::new(
                Atom::new("big_edge", vec![var("x"), var("y")]),
                vec![
                    BodyElement::Positive(Atom::new("edge", vec![var("x"), var("y")])),
                    BodyElement::Guard(Guard {
                        left: var("x"),
                        op: CompOp::Gt,
                        right: int(1),
                    }),
                ],
            ),
        ]);

        evaluate(&program, &mut db).unwrap();
        // Only edges starting from 2 and 3
        assert_eq!(db.relation("big_edge").unwrap().len(), 2);
    }

    #[test]
    fn test_constant_in_atom() {
        // starts_at_one(y) :- edge(1, y).
        let mut db = make_graph_db();
        let program = Program::new(vec![
            Rule::new(
                Atom::new("starts_at_one", vec![var("y")]),
                vec![BodyElement::Positive(Atom::new("edge", vec![int(1), var("y")]))],
            ),
        ]);

        evaluate(&program, &mut db).unwrap();
        assert_eq!(db.relation("starts_at_one").unwrap().len(), 1);
        let tuples: Vec<_> = db.relation("starts_at_one").unwrap().scan().collect();
        assert_eq!(tuples[0][0], Value::Int(2));
    }

    #[test]
    fn test_join_two_relations() {
        // result(x, name) :- edge(x, y), node_name(y, name).
        let mut db = make_graph_db();
        db.add_relation("node_name", RelationSchema {
            name: "node_name".to_string(),
            columns: vec![
                ColumnDef { name: "id".to_string(), col_type: ColumnType::Int },
                ColumnDef { name: "name".to_string(), col_type: ColumnType::Int },
            ],
        });
        db.insert("node_name", smallvec![Value::Int(2), Value::Int(200)]).unwrap();
        db.insert("node_name", smallvec![Value::Int(3), Value::Int(300)]).unwrap();
        db.insert("node_name", smallvec![Value::Int(4), Value::Int(400)]).unwrap();

        let program = Program::new(vec![
            Rule::new(
                Atom::new("result", vec![var("x"), var("name")]),
                vec![
                    BodyElement::Positive(Atom::new("edge", vec![var("x"), var("y")])),
                    BodyElement::Positive(Atom::new("node_name", vec![var("y"), var("name")])),
                ],
            ),
        ]);

        evaluate(&program, &mut db).unwrap();
        assert_eq!(db.relation("result").unwrap().len(), 3);
    }

    #[test]
    fn test_negation() {
        // no_outgoing(x) :- node(x), not edge(x, _y).
        let mut db = make_graph_db();
        db.add_relation("node", RelationSchema {
            name: "node".to_string(),
            columns: vec![ColumnDef { name: "id".to_string(), col_type: ColumnType::Int }],
        });
        db.insert("node", smallvec![Value::Int(1)]).unwrap();
        db.insert("node", smallvec![Value::Int(2)]).unwrap();
        db.insert("node", smallvec![Value::Int(3)]).unwrap();
        db.insert("node", smallvec![Value::Int(4)]).unwrap();

        let program = Program::new(vec![
            Rule::new(
                Atom::new("no_outgoing", vec![var("x")]),
                vec![
                    BodyElement::Positive(Atom::new("node", vec![var("x")])),
                    BodyElement::Negated(Atom::new("edge", vec![var("x"), var("_y")])),
                ],
            ),
        ]);

        evaluate(&program, &mut db).unwrap();
        // Only node 4 has no outgoing edges
        assert_eq!(db.relation("no_outgoing").unwrap().len(), 1);
        let tuples: Vec<_> = db.relation("no_outgoing").unwrap().scan().collect();
        assert_eq!(tuples[0][0], Value::Int(4));
    }

    #[test]
    fn test_cyclic_graph() {
        // Cyclic graph: 1 -> 2 -> 3 -> 1
        let mut db = Database::empty();
        db.add_relation("edge", RelationSchema {
            name: "edge".to_string(),
            columns: vec![
                ColumnDef { name: "src".to_string(), col_type: ColumnType::Int },
                ColumnDef { name: "dst".to_string(), col_type: ColumnType::Int },
            ],
        });
        db.insert("edge", smallvec![Value::Int(1), Value::Int(2)]).unwrap();
        db.insert("edge", smallvec![Value::Int(2), Value::Int(3)]).unwrap();
        db.insert("edge", smallvec![Value::Int(3), Value::Int(1)]).unwrap();

        let program = Program::new(vec![
            Rule::new(
                Atom::new("path", vec![var("x"), var("y")]),
                vec![BodyElement::Positive(Atom::new("edge", vec![var("x"), var("y")]))],
            ),
            Rule::new(
                Atom::new("path", vec![var("x"), var("y")]),
                vec![
                    BodyElement::Positive(Atom::new("path", vec![var("x"), var("z")])),
                    BodyElement::Positive(Atom::new("edge", vec![var("z"), var("y")])),
                ],
            ),
        ]);

        evaluate(&program, &mut db).unwrap();
        // 3 nodes, each can reach every other (including itself via cycle)
        // = 3 * 3 = 9 paths
        assert_eq!(db.relation("path").unwrap().len(), 9);
    }

    #[test]
    fn test_aggregate_count() {
        // out_degree(x, cnt) :- node(x), cnt = count(edge(x, _y)).
        let mut db = make_graph_db();
        db.add_relation("node", RelationSchema {
            name: "node".to_string(),
            columns: vec![ColumnDef { name: "id".to_string(), col_type: ColumnType::Int }],
        });
        for i in 1..=4 {
            db.insert("node", smallvec![Value::Int(i)]).unwrap();
        }

        let program = Program::new(vec![
            Rule::new(
                Atom::new("out_degree", vec![var("x"), var("cnt")]),
                vec![
                    BodyElement::Positive(Atom::new("node", vec![var("x")])),
                    BodyElement::Aggregate {
                        result_var: "cnt".to_string(),
                        function: AggFunction::Count,
                        sub_rule: Box::new(Rule::new(
                            Atom::new("_sub", vec![var("_y")]),
                            vec![BodyElement::Positive(Atom::new("edge", vec![var("x"), var("_y")]))],
                        )),
                        group_by: vec!["x".to_string()],
                        agg_var: "_y".to_string(),
                    },
                ],
            ),
        ]);

        evaluate(&program, &mut db).unwrap();
        let results: Vec<_> = db.relation("out_degree").unwrap().scan().collect();
        assert_eq!(results.len(), 4);

        // Node 1,2,3 each have 1 outgoing edge, node 4 has 0
        let node1 = results.iter().find(|t| t[0] == Value::Int(1)).unwrap();
        assert_eq!(node1[1], Value::Int(1));
        let node4 = results.iter().find(|t| t[0] == Value::Int(4)).unwrap();
        assert_eq!(node4[1], Value::Int(0));
    }

    #[test]
    fn test_multi_stratum() {
        // Stratum 1: path(x, y) :- edge(x, y).
        //            path(x, y) :- path(x, z), edge(z, y).
        // Stratum 2: unreachable(x) :- node(x), not path(1, x).
        let mut db = make_graph_db();
        // Add a disconnected node 5
        db.insert("edge", smallvec![Value::Int(5), Value::Int(5)]).unwrap(); // self-loop
        db.add_relation("node", RelationSchema {
            name: "node".to_string(),
            columns: vec![ColumnDef { name: "id".to_string(), col_type: ColumnType::Int }],
        });
        for i in 1..=5 {
            db.insert("node", smallvec![Value::Int(i)]).unwrap();
        }

        let program = Program::new(vec![
            Rule::new(
                Atom::new("path", vec![var("x"), var("y")]),
                vec![BodyElement::Positive(Atom::new("edge", vec![var("x"), var("y")]))],
            ),
            Rule::new(
                Atom::new("path", vec![var("x"), var("y")]),
                vec![
                    BodyElement::Positive(Atom::new("path", vec![var("x"), var("z")])),
                    BodyElement::Positive(Atom::new("edge", vec![var("z"), var("y")])),
                ],
            ),
            Rule::new(
                Atom::new("unreachable", vec![var("x")]),
                vec![
                    BodyElement::Positive(Atom::new("node", vec![var("x")])),
                    BodyElement::Negated(Atom::new("path", vec![int(1), var("x")])),
                ],
            ),
        ]);

        evaluate(&program, &mut db).unwrap();

        // From node 1, can reach 2, 3, 4. Cannot reach 5. Also 1 itself is not reachable from 1
        // (no self-loop on 1). So unreachable = {1, 5}
        let unreachable: Vec<_> = db.relation("unreachable").unwrap().scan().collect();
        assert_eq!(unreachable.len(), 2);
        assert!(unreachable.contains(&&smallvec![Value::Int(1)]));
        assert!(unreachable.contains(&&smallvec![Value::Int(5)]));
    }

    #[test]
    fn test_indexed_join_large() {
        // Stress test: join two large relations.
        // edge(src, dst) with 10K edges, node_name(id, name) with 1K nodes.
        // Query: result(x, name) :- edge(x, y), node_name(y, name).
        //
        // Without indexing: 10K * 1K = 10M comparisons.
        // With indexing on node_name[0]: 10K lookups, each O(1).
        let mut db = Database::empty();
        db.add_relation("edge", RelationSchema {
            name: "edge".to_string(),
            columns: vec![
                ColumnDef { name: "src".to_string(), col_type: ColumnType::Int },
                ColumnDef { name: "dst".to_string(), col_type: ColumnType::Int },
            ],
        });
        db.add_relation("node_name", RelationSchema {
            name: "node_name".to_string(),
            columns: vec![
                ColumnDef { name: "id".to_string(), col_type: ColumnType::Int },
                ColumnDef { name: "name".to_string(), col_type: ColumnType::Int },
            ],
        });

        // 1K nodes
        for i in 0..1000 {
            db.insert("node_name", smallvec![Value::Int(i), Value::Int(i * 100)]).unwrap();
        }
        // 10K edges (src in 0..100, dst in 0..1000)
        for i in 0..10_000 {
            db.insert("edge", smallvec![Value::Int(i % 100), Value::Int(i % 1000)]).unwrap();
        }

        let program = Program::new(vec![
            Rule::new(
                Atom::new("result", vec![var("x"), var("name")]),
                vec![
                    BodyElement::Positive(Atom::new("edge", vec![var("x"), var("y")])),
                    BodyElement::Positive(Atom::new("node_name", vec![var("y"), var("name")])),
                ],
            ),
        ]);

        let start = std::time::Instant::now();
        evaluate(&program, &mut db).unwrap();
        let elapsed = start.elapsed();

        let result_count = db.relation("result").unwrap().len();
        eprintln!(
            "Indexed join: {} edges x {} nodes → {} results in {:?}",
            db.relation("edge").unwrap().len(),
            db.relation("node_name").unwrap().len(),
            result_count,
            elapsed
        );
        assert!(result_count > 0);
        // With index acceleration, this should complete in well under 1 second
        assert!(elapsed.as_millis() < 5000, "join took too long: {:?}", elapsed);
    }

    #[test]
    fn test_indexed_transitive_closure_large() {
        // Transitive closure on a 500-node chain: 1→2→3→...→500
        // Total paths: 500*499/2 = 124,750
        // Semi-naive with indexing should handle this efficiently.
        let mut db = Database::empty();
        db.add_relation("edge", RelationSchema {
            name: "edge".to_string(),
            columns: vec![
                ColumnDef { name: "src".to_string(), col_type: ColumnType::Int },
                ColumnDef { name: "dst".to_string(), col_type: ColumnType::Int },
            ],
        });
        for i in 1..=499 {
            db.insert("edge", smallvec![Value::Int(i), Value::Int(i + 1)]).unwrap();
        }

        let program = Program::new(vec![
            Rule::new(
                Atom::new("path", vec![var("x"), var("y")]),
                vec![BodyElement::Positive(Atom::new("edge", vec![var("x"), var("y")]))],
            ),
            Rule::new(
                Atom::new("path", vec![var("x"), var("y")]),
                vec![
                    BodyElement::Positive(Atom::new("path", vec![var("x"), var("z")])),
                    BodyElement::Positive(Atom::new("edge", vec![var("z"), var("y")])),
                ],
            ),
        ]);

        let start = std::time::Instant::now();
        evaluate(&program, &mut db).unwrap();
        let elapsed = start.elapsed();

        let path_count = db.relation("path").unwrap().len();
        eprintln!(
            "Transitive closure: 499 edges → {} paths in {:?}",
            path_count, elapsed
        );
        assert_eq!(path_count, 499 * 500 / 2); // n*(n+1)/2 - n = n*(n-1)/2 ... actually 499*500/2 = 124,750
        assert!(elapsed.as_secs() < 30, "transitive closure took too long: {:?}", elapsed);
    }

    #[test]
    fn test_arithmetic_add() {
        let mut db = make_graph_db();
        // sum(x, y, z) :- edge(x, y), z = x + y.
        let program = Program::new(vec![
            Rule::new(
                Atom::new("sum", vec![var("x"), var("y"), var("z")]),
                vec![
                    BodyElement::Positive(Atom::new("edge", vec![var("x"), var("y")])),
                    BodyElement::Assign {
                        result_var: "z".to_string(),
                        expr: crate::rule::ArithExpr {
                            left: var("x"),
                            op: crate::rule::ArithOp::Add,
                            right: var("y"),
                        },
                    },
                ],
            ),
        ]);
        evaluate(&program, &mut db).unwrap();

        let results: Vec<_> = db.scan("sum").unwrap().collect();
        assert!(!results.is_empty());
        // edge(1,2) → sum(1,2,3)
        assert!(results.iter().any(|t| t[0] == Value::Int(1) && t[1] == Value::Int(2) && t[2] == Value::Int(3)));
        // edge(2,3) → sum(2,3,5)
        assert!(results.iter().any(|t| t[0] == Value::Int(2) && t[1] == Value::Int(3) && t[2] == Value::Int(5)));
    }

    #[test]
    fn test_arithmetic_sub_mul() {
        let mut db = Database::empty();
        db.add_relation("vals", RelationSchema {
            name: "vals".to_string(),
            columns: vec![
                ColumnDef { name: "x".to_string(), col_type: ColumnType::Int },
            ],
        });
        db.insert("vals", smallvec![Value::Int(10)]).unwrap();
        db.insert("vals", smallvec![Value::Int(20)]).unwrap();

        let program = Program::new(vec![
            // doubled(x, d) :- vals(x), d = x * 2.
            Rule::new(
                Atom::new("doubled", vec![var("x"), var("d")]),
                vec![
                    BodyElement::Positive(Atom::new("vals", vec![var("x")])),
                    BodyElement::Assign {
                        result_var: "d".to_string(),
                        expr: crate::rule::ArithExpr {
                            left: var("x"),
                            op: crate::rule::ArithOp::Mul,
                            right: int(2),
                        },
                    },
                ],
            ),
            // decremented(x, d) :- vals(x), d = x - 1.
            Rule::new(
                Atom::new("decremented", vec![var("x"), var("d")]),
                vec![
                    BodyElement::Positive(Atom::new("vals", vec![var("x")])),
                    BodyElement::Assign {
                        result_var: "d".to_string(),
                        expr: crate::rule::ArithExpr {
                            left: var("x"),
                            op: crate::rule::ArithOp::Sub,
                            right: int(1),
                        },
                    },
                ],
            ),
        ]);
        evaluate(&program, &mut db).unwrap();

        let doubled: Vec<_> = db.scan("doubled").unwrap().collect();
        assert!(doubled.iter().any(|t| t[0] == Value::Int(10) && t[1] == Value::Int(20)));
        assert!(doubled.iter().any(|t| t[0] == Value::Int(20) && t[1] == Value::Int(40)));

        let dec: Vec<_> = db.scan("decremented").unwrap().collect();
        assert!(dec.iter().any(|t| t[0] == Value::Int(10) && t[1] == Value::Int(9)));
        assert!(dec.iter().any(|t| t[0] == Value::Int(20) && t[1] == Value::Int(19)));
    }

    #[test]
    fn test_arithmetic_div_by_zero() {
        let mut db = Database::empty();
        db.add_relation("vals", RelationSchema {
            name: "vals".to_string(),
            columns: vec![
                ColumnDef { name: "x".to_string(), col_type: ColumnType::Int },
            ],
        });
        db.insert("vals", smallvec![Value::Int(10)]).unwrap();

        // result(x, d) :- vals(x), d = x / 0.  (should produce no results)
        let program = Program::new(vec![
            Rule::new(
                Atom::new("result", vec![var("x"), var("d")]),
                vec![
                    BodyElement::Positive(Atom::new("vals", vec![var("x")])),
                    BodyElement::Assign {
                        result_var: "d".to_string(),
                        expr: crate::rule::ArithExpr {
                            left: var("x"),
                            op: crate::rule::ArithOp::Div,
                            right: int(0),
                        },
                    },
                ],
            ),
        ]);
        evaluate(&program, &mut db).unwrap();
        assert_eq!(db.relation("result").unwrap().len(), 0);
    }
}
