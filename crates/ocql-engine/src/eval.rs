//! Semi-naive Datalog evaluator.
//!
//! Evaluates a stratified Datalog program against a database to a fixed point.
//! Uses semi-naive evaluation for recursive strata: in each iteration, at least
//! one body atom must bind from the delta (newly derived) tuples.

use std::collections::HashMap;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use ocql_database::{Database, RelationSchema, ColumnDef, Tuple, Value};
use ocql_schema::ColumnType;
use smallvec::SmallVec;

use crate::rule::{Atom, BodyElement, CompOp, Guard, Program, Rule, Term, AggFunction, ArithOp};
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

/// Variable bindings during rule evaluation (used for aggregate sub-rules).
type Bindings = HashMap<String, Value>;

// === Compiled evaluation types ===
//
// Pre-compiled rule representation using slot indices instead of variable name strings.
// This eliminates HashMap cloning and string hashing from the inner evaluation loops.
// A rule with variables {x, y, z} maps them to slots {0, 1, 2}, and bindings become
// Vec<Option<Value>> instead of HashMap<String, Value>.

/// Compiled term: variables reference slots by index instead of by name.
#[derive(Clone, Debug)]
enum CTerm {
    Slot(usize),
    Const(Value),
}

/// Compiled atom with slot-based terms.
#[derive(Clone, Debug)]
struct CAtom {
    predicate: String,
    terms: Vec<CTerm>,
}

/// Compiled body element.
#[derive(Clone, Debug)]
enum CBody {
    Positive(CAtom),
    Negated(CAtom),
    Guard { left: CTerm, op: CompOp, right: CTerm },
    Assign { slot: usize, left: CTerm, op: ArithOp, right: CTerm },
    Aggregate {
        slot: usize,
        function: AggFunction,
        sub_rule: Box<Rule>,
        group_by: Vec<String>,
        agg_var: String,
        var_map: HashMap<String, usize>,
    },
}

/// Flat buffer of slot rows — eliminates per-row heap allocation.
///
/// Instead of `Vec<Vec<Option<Value>>>` (one heap alloc per row),
/// stores all rows contiguously in a single `Vec<Option<Value>>`.
/// Since `Option<Value>` is `Copy`, `extend_from_slice` is a single memcpy.
struct SlotRows {
    data: Vec<Option<Value>>,
    stride: usize,
    count: usize,
}

impl SlotRows {
    fn new(stride: usize) -> Self {
        Self { data: Vec::new(), stride, count: 0 }
    }

    fn len(&self) -> usize {
        self.count
    }

    fn is_empty(&self) -> bool {
        self.count == 0
    }

    fn row(&self, i: usize) -> &[Option<Value>] {
        &self.data[i * self.stride..(i + 1) * self.stride]
    }

    fn clear(&mut self) {
        self.data.clear();
        self.count = 0;
    }

    /// Append a row (single memcpy for Copy types).
    fn push_row(&mut self, row: &[Option<Value>]) {
        self.data.extend_from_slice(row);
        self.count += 1;
    }

    /// Append a row with one slot modified.
    fn push_row_with(&mut self, row: &[Option<Value>], slot: usize, val: Value) {
        let start = self.data.len();
        self.data.extend_from_slice(row);
        self.data[start + slot] = Some(val);
        self.count += 1;
    }
}


// === Scallop-style DynRelation for semi-naive evaluation ===
//
// Each recursive predicate gets a DynRelation that manages the
// stable/recent/to_add lifecycle:
// - stable: accumulated facts (stored in Database)
// - recent: newly derived facts from the last iteration (= delta)
// - to_add: facts being collected in the current iteration
//
// Rc<RefCell<...>> allows multiple rules to hold shared references
// to the same relation. This pattern directly follows Scallop's
// DynamicRelation design and enables future parallelism by replacing
// Rc with Arc and RefCell with RwLock/Mutex.

/// Dynamic relation for semi-naive evaluation.
#[derive(Clone)]
struct DynRelation {
    /// Facts derived in the previous iteration (= delta for semi-naive).
    recent: Rc<RefCell<Vec<Tuple>>>,
    /// Facts being accumulated in the current iteration.
    to_add: Rc<RefCell<Vec<Tuple>>>,
}

impl DynRelation {
    fn new() -> Self {
        Self {
            recent: Rc::new(RefCell::new(Vec::new())),
            to_add: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Push a tuple into to_add (dedup happens on merge).
    fn insert_to_add(&self, tuple: Tuple) {
        self.to_add.borrow_mut().push(tuple);
    }

    /// Rotate: merge to_add into the database (stable), set recent = newly inserted.
    /// Returns the count of genuinely new tuples.
    fn changed(&self, db: &mut Database, name: &str) -> Result<usize, EvalError> {
        let to_add = std::mem::take(&mut *self.to_add.borrow_mut());
        let mut new_recent = Vec::new();
        for tuple in to_add {
            if db.insert(name, tuple.clone())
                .map_err(|_| EvalError::UnknownRelation(name.to_string()))? {
                new_recent.push(tuple);
            }
        }
        let count = new_recent.len();
        *self.recent.borrow_mut() = new_recent;
        Ok(count)
    }

    fn recent_is_empty(&self) -> bool {
        self.recent.borrow().is_empty()
    }
}

/// Evaluate a Datalog program against a database.
///
/// Creates output relations for all IDB predicates and evaluates rules
/// to a fixed point. Returns the names of all IDB relations created.
pub fn evaluate(program: &Program, db: &mut Database) -> Result<Vec<String>, EvalError> {
    let profile = std::env::var("OCQL_PROFILE").is_ok();
    let t0 = Instant::now();

    // Seed @type#char relations from the schema.
    seed_db_type_chars(db);

    let t_seed = t0.elapsed();
    let strata = stratify(program).map_err(EvalError::Stratification)?;
    let t_strat = t0.elapsed();

    // Ensure output relations exist for all IDB predicates
    let idb_names = program.head_predicates();
    for rule in &program.rules {
        ensure_relation_for_head(db, rule);
    }

    if profile {
        eprintln!("[profile] seed: {:?}, stratify: {:?}, strata: {}, rules: {}",
            t_seed, t_strat - t_seed, strata.len(), program.rules.len());
    }

    let mut total_rules_evaluated = 0u64;
    let mut total_tuples_produced = 0u64;
    let mut rules_skipped_empty = 0u64;

    for (si, stratum) in strata.iter().enumerate() {
        let ts = Instant::now();
        if stratum.is_recursive {
            let stats = evaluate_recursive_profiled(stratum, db, profile)?;
            total_rules_evaluated += stats.0;
            total_tuples_produced += stats.1;
            rules_skipped_empty += stats.2;
            if profile {
                let elapsed = ts.elapsed();
                if elapsed.as_millis() > 100 {
                    eprintln!("[profile] stratum {}/{} (recursive, {} preds, {} rules): {:?}, {} tuples, {} skipped",
                        si, strata.len(), stratum.predicates.len(), stratum.rules.len(),
                        elapsed, stats.1, stats.2);
                }
            }
        } else {
            let stats = evaluate_nonrecursive_profiled(stratum, db, profile)?;
            total_rules_evaluated += stats.0;
            total_tuples_produced += stats.1;
            rules_skipped_empty += stats.2;
            if profile {
                let elapsed = ts.elapsed();
                if elapsed.as_millis() > 50 {
                    let preds: Vec<&str> = stratum.rules.iter()
                        .map(|r| r.head.predicate.as_str()).collect();
                    let body_preds: Vec<String> = stratum.rules.iter()
                        .flat_map(|r| r.body.iter().filter_map(|e| {
                            if let BodyElement::Positive(a) = e { Some(a.predicate.clone()) } else { None }
                        }))
                        .collect();
                    eprintln!("[profile] stratum {}/{} (non-rec, {} rules): {:?}, {} tuples, {} skipped | head={:?} body={:?}",
                        si, strata.len(), stratum.rules.len(),
                        elapsed, stats.1, stats.2, preds, body_preds);
                }
            }
        }
    }

    if profile {
        eprintln!("[profile] TOTAL: {:?}, rules_eval: {}, tuples: {}, skipped_empty: {}",
            t0.elapsed(), total_rules_evaluated, total_tuples_produced, rules_skipped_empty);
        // Print sizes of key relations
        let names: Vec<String> = db.relation_names().map(|n| n.to_string()).collect();
        let mut rel_sizes: Vec<(String, usize)> = names.iter()
            .filter_map(|n| db.relation(n).map(|r| (n.clone(), r.len())))
            .filter(|(_, sz)| *sz > 0)
            .collect();
        rel_sizes.sort_by(|a, b| b.1.cmp(&a.1));
        eprintln!("[profile] Top relations by size:");
        for (name, sz) in rel_sizes.iter().take(20) {
            eprintln!("  {:>6} {}", sz, name);
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

/// Seed `@typename#char` relations from the database schema.
///
/// For each table with a `unique int id: @typename` column, collects all entity IDs
/// and inserts them into a `@typename#char` relation. Also handles union types
/// transitively (e.g., `@container = @file | @folder` means all `@file` and `@folder`
/// entities are also `@container` entities).
fn seed_db_type_chars(db: &mut Database) {
    use std::collections::HashSet;

    // Phase 1: For each table with `unique int id: @typename`, collect entity IDs
    let mut type_entities: HashMap<String, Vec<Value>> = HashMap::new();

    // Iterate schema tables to find defining tables
    let schema = db.schema.clone();
    for table in schema.tables() {
        // Find first column that is unique and has an entity db_type
        if let Some(col) = table.columns.first() {
            if col.is_unique {
                if let ocql_schema::DbType::Entity(ref entity_name) = col.db_type {
                    // This table defines instances of @entity_name
                    if let Some(rel) = db.relation(&table.name) {
                        let ids: Vec<Value> = rel.scan()
                            .map(|tuple| tuple[0].clone())
                            .collect();
                        type_entities.entry(entity_name.clone())
                            .or_default()
                            .extend(ids);
                    }
                }
            }
        }
    }

    // Phase 2: Propagate through union types
    // @container = @file | @folder  →  @container entities include @file + @folder entities
    let unions: Vec<_> = schema.unions().cloned().collect();
    // Iterate until fixpoint (unions can be transitive)
    let mut changed = true;
    while changed {
        changed = false;
        for union_type in &unions {
            let mut combined: Vec<Value> = type_entities.get(&union_type.name)
                .cloned()
                .unwrap_or_default();
            let before_len = combined.len();
            for variant in &union_type.variants {
                if let Some(variant_entities) = type_entities.get(variant) {
                    combined.extend(variant_entities.iter().cloned());
                }
            }
            // Dedup
            let deduped: HashSet<Value> = combined.into_iter().collect();
            let new_entities: Vec<Value> = deduped.into_iter().collect();
            if new_entities.len() != before_len {
                changed = true;
                type_entities.insert(union_type.name.clone(), new_entities);
            }
        }
    }

    // Phase 3: Insert into @typename#char relations
    for (type_name, entities) in &type_entities {
        let char_name = format!("{}#char", type_name);
        // Create the relation if it doesn't exist
        let rel_schema = RelationSchema {
            name: char_name.clone(),
            columns: vec![ColumnDef {
                name: "this".to_string(),
                col_type: ColumnType::Int,
            }],
        };
        if db.relation(&char_name).is_none() {
            db.add_relation(&char_name, rel_schema);
        }
        for entity in entities {
            let tuple: Tuple = SmallVec::from_vec(vec![entity.clone()]);
            let _ = db.insert(&char_name, tuple);
        }
    }
}

/// Check if a rule can possibly produce results by checking that all
/// positive body atoms reference non-empty relations (or are yet-to-be-computed IDB).
/// Returns false if any positive body atom references a definitely-empty relation.
fn rule_has_empty_body(rule: &Rule, db: &Database) -> bool {
    for elem in &rule.body {
        if let BodyElement::Positive(atom) = elem {
            if let Some(rel) = db.relation(&atom.predicate) {
                if rel.is_empty() {
                    return true;
                }
            } else {
                // Relation doesn't exist at all — definitely empty
                return true;
            }
        }
    }
    false
}

/// Stats: (rules_evaluated, tuples_produced, rules_skipped)
fn evaluate_nonrecursive_profiled(
    stratum: &Stratum,
    db: &mut Database,
    _profile: bool,
) -> Result<(u64, u64, u64), EvalError> {
    let mut rules_evaluated = 0u64;
    let mut tuples_produced = 0u64;
    let mut rules_skipped = 0u64;

    for rule in &stratum.rules {
        // Skip rules where any positive body atom is definitely empty
        if rule_has_empty_body(rule, db) {
            rules_skipped += 1;
            continue;
        }

        rules_evaluated += 1;
        let new_tuples = evaluate_rule(rule, db)?;
        let name = &rule.head.predicate;
        for tuple in new_tuples {
            if db.insert(name, tuple).map_err(|_| EvalError::UnknownRelation(name.clone()))? {
                tuples_produced += 1;
            }
        }
    }

    Ok((rules_evaluated, tuples_produced, rules_skipped))
}

/// Stats: (rules_evaluated, tuples_produced, rules_skipped)
fn evaluate_recursive_profiled(
    stratum: &Stratum,
    db: &mut Database,
    _profile: bool,
) -> Result<(u64, u64, u64), EvalError> {
    let mut rules_evaluated = 0u64;
    let mut tuples_produced = 0u64;
    let mut rules_skipped = 0u64;

    // Create a DynRelation for each recursive predicate in this stratum.
    let mut dyn_rels: HashMap<String, DynRelation> = HashMap::new();
    for pred in &stratum.predicates {
        dyn_rels.insert(pred.clone(), DynRelation::new());
    }

    // Initial evaluation: run all rules once to seed to_add
    for rule in &stratum.rules {
        if rule_has_empty_body(rule, db) {
            rules_skipped += 1;
            continue;
        }
        rules_evaluated += 1;
        let new_tuples = evaluate_rule(rule, db)?;
        let name = &rule.head.predicate;
        if let Some(dyn_rel) = dyn_rels.get(name) {
            for tuple in new_tuples {
                dyn_rel.insert_to_add(tuple);
            }
        } else {
            for tuple in new_tuples {
                if db.insert(name, tuple).map_err(|_| EvalError::UnknownRelation(name.clone()))? {
                    tuples_produced += 1;
                }
            }
        }
    }

    // Rotate: merge to_add into db (stable), populate recent (= delta)
    for (name, dyn_rel) in &dyn_rels {
        tuples_produced += dyn_rel.changed(db, name)? as u64;
    }

    // Compilation cache: (rule_index, delta_body_idx) → compiled data.
    // Compiled lazily on first use and reused across iterations.
    let mut compile_cache: HashMap<(usize, usize), (Vec<CBody>, CAtom, usize)> = HashMap::new();

    // Semi-naive loop
    loop {
        if dyn_rels.values().all(|dr| dr.recent_is_empty()) {
            break;
        }

        for (rule_idx, rule) in stratum.rules.iter().enumerate() {
            // Quick check: skip if any non-recursive positive body atom is empty
            let skip = rule.body.iter().any(|elem| {
                if let BodyElement::Positive(atom) = elem {
                    if !stratum.predicates.contains(&atom.predicate) {
                        if let Some(rel) = db.relation(&atom.predicate) {
                            return rel.is_empty();
                        } else {
                            return true;
                        }
                    }
                }
                false
            });
            if skip {
                rules_skipped += 1;
                continue;
            }

            // Find which body atoms reference recursive predicates with non-empty recent
            let recursive_atoms: Vec<usize> = rule.body.iter().enumerate()
                .filter_map(|(i, elem)| {
                    if let BodyElement::Positive(atom) = elem {
                        if let Some(dyn_rel) = dyn_rels.get(&atom.predicate) {
                            if !dyn_rel.recent_is_empty() {
                                return Some(i);
                            }
                        }
                    }
                    None
                })
                .collect();

            if recursive_atoms.is_empty() {
                rules_skipped += 1;
                continue;
            }

            // For semi-naive: evaluate with each recursive atom using its recent as delta
            for &delta_idx in &recursive_atoms {
                let pred = match &rule.body[delta_idx] {
                    BodyElement::Positive(atom) => &atom.predicate,
                    _ => unreachable!(),
                };
                let recent = dyn_rels[pred].recent.borrow();
                rules_evaluated += 1;

                // Get or compile the delta rule
                let cache_key = (rule_idx, delta_idx);
                let (compiled_body, compiled_head, num_slots) = compile_cache
                    .entry(cache_key)
                    .or_insert_with(|| {
                        let delta_elem = rule.body[delta_idx].clone();
                        let rest: Vec<BodyElement> = rule.body.iter().enumerate()
                            .filter(|(i, _)| *i != delta_idx)
                            .map(|(_, e)| e.clone())
                            .collect();
                        let optimized_rest = optimize_body_order(&rest, db);
                        let mut reordered = Vec::with_capacity(rule.body.len());
                        reordered.push(delta_elem);
                        reordered.extend(optimized_rest);
                        compile_body_and_head(&rule.head, &reordered)
                    });

                let initial = vec![None; *num_slots];
                let slot_rows = evaluate_body_c(compiled_body, db, &initial, Some(&recent))?;
                drop(recent);

                // Project and insert results
                let name = &rule.head.predicate;
                if let Some(head_dyn_rel) = dyn_rels.get(name) {
                    for ri in 0..slot_rows.len() {
                        if let Some(tuple) = project_head_c(compiled_head, slot_rows.row(ri)) {
                            head_dyn_rel.insert_to_add(tuple);
                        }
                    }
                } else {
                    for ri in 0..slot_rows.len() {
                        if let Some(tuple) = project_head_c(compiled_head, slot_rows.row(ri)) {
                            if db.insert(name, tuple).map_err(|_| EvalError::UnknownRelation(name.clone()))? {
                                tuples_produced += 1;
                            }
                        }
                    }
                }
            }
        }

        // Rotate all DynRelations
        for (name, dyn_rel) in &dyn_rels {
            tuples_produced += dyn_rel.changed(db, name)? as u64;
        }
    }

    Ok((rules_evaluated, tuples_produced, rules_skipped))
}

/// Evaluate a rule using compiled slot-based bindings.
fn evaluate_rule(rule: &Rule, db: &Database) -> Result<Vec<Tuple>, EvalError> {
    let optimized_body = optimize_body_order(&rule.body, db);
    let (compiled_body, compiled_head, num_slots) = compile_body_and_head(&rule.head, &optimized_body);
    let initial = vec![None; num_slots];
    let slot_rows = evaluate_body_c(&compiled_body, db, &initial, None)?;
    let mut results = Vec::new();
    for ri in 0..slot_rows.len() {
        if let Some(tuple) = project_head_c(&compiled_head, slot_rows.row(ri)) {
            results.push(tuple);
        }
    }
    Ok(results)
}

/// Reorder body elements for better join performance.
///
/// Strategy: use a greedy heuristic that at each step picks the element
/// that will produce the fewest intermediate results:
/// 1. Positive atoms with smaller relations come first (fewer tuples to scan).
/// 2. Among atoms of similar size, prefer those with more already-bound variables.
/// 3. Guards/Assigns are placed as early as possible (once their variables are bound).
/// 4. Negated atoms go after all their variables are bound.
/// 5. Aggregates go last.
fn optimize_body_order(body: &[BodyElement], db: &Database) -> Vec<BodyElement> {
    use std::collections::HashSet;

    if body.len() <= 1 {
        return body.to_vec();
    }

    // Collect variables that each element binds and requires
    fn atom_vars(atom: &Atom) -> HashSet<String> {
        atom.terms.iter().filter_map(|t| {
            if let Term::Var(v) = t { Some(v.clone()) } else { None }
        }).collect()
    }

    fn guard_vars(guard: &Guard) -> HashSet<String> {
        let mut vars = HashSet::new();
        if let Term::Var(v) = &guard.left { vars.insert(v.clone()); }
        if let Term::Var(v) = &guard.right { vars.insert(v.clone()); }
        vars
    }

    fn assign_required_vars(expr: &crate::rule::ArithExpr) -> HashSet<String> {
        let mut vars = HashSet::new();
        if let Term::Var(v) = &expr.left { vars.insert(v.clone()); }
        if let Term::Var(v) = &expr.right { vars.insert(v.clone()); }
        vars
    }

    // Partition body elements into categories
    let mut positive_atoms: Vec<(usize, &Atom)> = Vec::new();
    let mut negated_atoms: Vec<(usize, &Atom)> = Vec::new();
    let mut guards: Vec<(usize, &Guard)> = Vec::new();
    let mut assigns: Vec<(usize, &str, &crate::rule::ArithExpr)> = Vec::new();
    let mut aggregates: Vec<usize> = Vec::new();

    for (i, elem) in body.iter().enumerate() {
        match elem {
            BodyElement::Positive(atom) => positive_atoms.push((i, atom)),
            BodyElement::Negated(atom) => negated_atoms.push((i, atom)),
            BodyElement::Guard(guard) => guards.push((i, guard)),
            BodyElement::Assign { result_var, expr } => assigns.push((i, result_var.as_str(), expr)),
            BodyElement::Aggregate { .. } => aggregates.push(i),
        }
    }

    // Sort positive atoms by relation size (ascending) — smallest first
    positive_atoms.sort_by_key(|(_, atom)| {
        db.relation(&atom.predicate).map(|r| r.len()).unwrap_or(0)
    });

    // Greedy scheduling
    let mut result: Vec<BodyElement> = Vec::with_capacity(body.len());
    let mut bound_vars: HashSet<String> = HashSet::new();
    let mut used: HashSet<usize> = HashSet::new();

    // Schedule positive atoms first (in order of estimated selectivity)
    for &(idx, atom) in &positive_atoms {
        result.push(body[idx].clone());
        used.insert(idx);
        // This atom binds all its variables
        bound_vars.extend(atom_vars(atom));

        // After each positive atom, try to schedule any guards/assigns whose vars are now bound
        loop {
            let mut scheduled_any = false;

            for &(gi, guard) in &guards {
                if used.contains(&gi) { continue; }
                let needed = guard_vars(guard);
                if needed.is_subset(&bound_vars) {
                    result.push(body[gi].clone());
                    used.insert(gi);
                    scheduled_any = true;
                }
            }

            for &(ai, result_var, expr) in &assigns {
                if used.contains(&ai) { continue; }
                let needed = assign_required_vars(expr);
                if needed.is_subset(&bound_vars) {
                    result.push(body[ai].clone());
                    used.insert(ai);
                    bound_vars.insert(result_var.to_string());
                    scheduled_any = true;
                }
            }

            if !scheduled_any { break; }
        }
    }

    // Schedule negated atoms (need all vars bound)
    for &(idx, atom) in &negated_atoms {
        if used.contains(&idx) { continue; }
        let needed = atom_vars(atom);
        if needed.is_subset(&bound_vars) {
            result.push(body[idx].clone());
            used.insert(idx);
        }
    }

    // Schedule aggregates
    for &idx in &aggregates {
        if used.contains(&idx) { continue; }
        result.push(body[idx].clone());
        used.insert(idx);
    }

    // Any remaining elements (shouldn't happen, but safety)
    for (i, elem) in body.iter().enumerate() {
        if !used.contains(&i) {
            result.push(elem.clone());
        }
    }

    result
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
    let rel = match db.relation(&atom.predicate) {
        Some(r) => r,
        // Undefined relation: return no matches (standard Datalog semantics —
        // a reference to an undefined predicate produces zero tuples).
        None => return Ok(Vec::new()),
    };

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

// ============================================================
// Compiled evaluation functions
// ============================================================

/// Compile a Term into a CTerm, assigning slot indices to variables.
fn compile_term(term: &Term, var_map: &mut HashMap<String, usize>, next_slot: &mut usize) -> CTerm {
    match term {
        Term::Var(name) => {
            let slot = *var_map.entry(name.clone()).or_insert_with(|| {
                let s = *next_slot;
                *next_slot += 1;
                s
            });
            CTerm::Slot(slot)
        }
        Term::Const(v) => CTerm::Const(v.clone()),
        Term::StrLit(_) => panic!("unresolved string literal; call program.resolve_strings() first"),
    }
}

/// Compile a rule head and body into slot-based representation.
/// Returns (compiled_body, compiled_head, num_slots).
fn compile_body_and_head(head: &Atom, body: &[BodyElement]) -> (Vec<CBody>, CAtom, usize) {
    let mut var_map: HashMap<String, usize> = HashMap::new();
    let mut next_slot = 0usize;

    let compiled_body: Vec<CBody> = body.iter().map(|elem| {
        match elem {
            BodyElement::Positive(atom) => {
                let terms = atom.terms.iter()
                    .map(|t| compile_term(t, &mut var_map, &mut next_slot))
                    .collect();
                CBody::Positive(CAtom { predicate: atom.predicate.clone(), terms })
            }
            BodyElement::Negated(atom) => {
                let terms = atom.terms.iter()
                    .map(|t| compile_term(t, &mut var_map, &mut next_slot))
                    .collect();
                CBody::Negated(CAtom { predicate: atom.predicate.clone(), terms })
            }
            BodyElement::Guard(guard) => {
                let left = compile_term(&guard.left, &mut var_map, &mut next_slot);
                let right = compile_term(&guard.right, &mut var_map, &mut next_slot);
                CBody::Guard { left, op: guard.op, right }
            }
            BodyElement::Assign { result_var, expr } => {
                let left = compile_term(&expr.left, &mut var_map, &mut next_slot);
                let right = compile_term(&expr.right, &mut var_map, &mut next_slot);
                let slot = *var_map.entry(result_var.clone()).or_insert_with(|| {
                    let s = next_slot;
                    next_slot += 1;
                    s
                });
                CBody::Assign { slot, left, op: expr.op, right }
            }
            BodyElement::Aggregate { result_var, function, sub_rule, group_by, agg_var } => {
                let slot = *var_map.entry(result_var.clone()).or_insert_with(|| {
                    let s = next_slot;
                    next_slot += 1;
                    s
                });
                CBody::Aggregate {
                    slot,
                    function: *function,
                    sub_rule: sub_rule.clone(),
                    group_by: group_by.clone(),
                    agg_var: agg_var.clone(),
                    var_map: var_map.clone(),
                }
            }
        }
    }).collect();

    let head_terms = head.terms.iter()
        .map(|t| compile_term(t, &mut var_map, &mut next_slot))
        .collect();
    let compiled_head = CAtom { predicate: head.predicate.clone(), terms: head_terms };

    (compiled_body, compiled_head, next_slot)
}

/// Evaluate a compiled body, threading slot-based bindings through.
/// If `delta_tuples` is Some(tuples), the first body element (position 0) reads
/// from delta tuples instead of the full database relation.
///
/// Uses flat SlotRows buffers — all rows are stored contiguously in a single Vec.
/// Since Option<Value> is Copy, row copying is a single memcpy, eliminating
/// the per-row heap allocation that Vec<Vec<Option<Value>>> would require.
fn evaluate_body_c(
    body: &[CBody],
    db: &Database,
    initial: &[Option<Value>],
    delta_tuples: Option<&[Tuple]>,
) -> Result<SlotRows, EvalError> {
    let stride = initial.len();

    // Fast path for empty body (facts — rules with no body elements)
    if body.is_empty() {
        let mut results = SlotRows::new(stride);
        results.push_row(initial);
        return Ok(results);
    }

    // Fast path for single positive body element
    if body.len() == 1 {
        if let CBody::Positive(atom) = &body[0] {
            let mut results = SlotRows::new(stride);
            if let Some(dt) = delta_tuples {
                for tuple in dt {
                    match_tuple_fresh_flat(atom, tuple, stride, &mut results);
                }
            } else {
                match_atom_flat(atom, db, initial, &mut results)?;
            }
            return Ok(results);
        }
    }

    let mut current = SlotRows::new(stride);
    current.push_row(initial);
    let mut next = SlotRows::new(stride);

    for (i, elem) in body.iter().enumerate() {
        next.clear();
        let is_delta = i == 0 && delta_tuples.is_some();

        for row_idx in 0..current.len() {
            let row = current.row(row_idx);
            match elem {
                CBody::Positive(atom) => {
                    if is_delta {
                        for tuple in delta_tuples.unwrap() {
                            match_tuple_fresh_flat(atom, tuple, stride, &mut next);
                        }
                    } else {
                        match_atom_flat(atom, db, row, &mut next)?;
                    }
                }
                CBody::Negated(atom) => {
                    if !has_any_match_c(atom, db, row)? {
                        next.push_row(row);
                    }
                }
                CBody::Guard { left, op, right } => {
                    eval_guard_flat(left, *op, right, row, &mut next);
                }
                CBody::Assign { slot, left, op, right } => {
                    if let Some(val) = eval_arith_c(left, *op, right, row) {
                        next.push_row_with(row, *slot, val);
                    }
                }
                CBody::Aggregate { slot, function, sub_rule, group_by, agg_var, var_map } => {
                    let bindings = slots_to_bindings(row, var_map);
                    let agg_result = eval_aggregate(
                        function, sub_rule, group_by, agg_var, db, &bindings,
                    )?;
                    next.push_row_with(row, *slot, agg_result);
                }
            }
        }

        std::mem::swap(&mut current, &mut next);
        if current.is_empty() {
            break;
        }
    }

    Ok(current)
}

/// Match a compiled atom against a database relation, pushing results into `out`.
fn match_atom_flat(
    atom: &CAtom,
    db: &Database,
    row: &[Option<Value>],
    out: &mut SlotRows,
) -> Result<(), EvalError> {
    let rel = match db.relation(&atom.predicate) {
        Some(r) => r,
        None => return Ok(()),
    };

    // Determine which columns have known values
    let mut bound_cols: SmallVec<[usize; 4]> = SmallVec::new();
    let mut bound_vals: SmallVec<[Value; 4]> = SmallVec::new();
    for (i, term) in atom.terms.iter().enumerate() {
        match term {
            CTerm::Const(v) => {
                bound_cols.push(i);
                bound_vals.push(*v);
            }
            CTerm::Slot(s) => {
                if let Some(val) = row[*s] {
                    bound_cols.push(i);
                    bound_vals.push(val);
                }
            }
        }
    }

    if bound_cols.is_empty() {
        for tuple in rel.scan() {
            match_tuple_flat(atom, tuple, row, out);
        }
    } else {
        rel.lookup_each(&bound_cols, &bound_vals, |tuple| {
            match_tuple_flat(atom, tuple, row, out);
        });
    }

    Ok(())
}

/// Match a compiled atom against a tuple when all slots are initially empty (delta scan path).
/// Pushes result directly into flat buffer.
fn match_tuple_fresh_flat(atom: &CAtom, tuple: &Tuple, stride: usize, out: &mut SlotRows) -> bool {
    if atom.terms.len() != tuple.len() {
        return false;
    }

    // Check constants only (no bound slots to check since all are None)
    for (term, value) in atom.terms.iter().zip(tuple.iter()) {
        if let CTerm::Const(c) = term {
            if *c != *value {
                return false;
            }
        }
    }

    // Append a fresh row and fill slots
    let start = out.data.len();
    out.data.resize(start + stride, None);
    for (term, value) in atom.terms.iter().zip(tuple.iter()) {
        if let CTerm::Slot(s) = term {
            out.data[start + *s] = Some(*value);
        }
    }
    out.count += 1;
    true
}

/// Try to match a compiled atom against a single tuple, pushing result into flat buffer.
/// Phase 1: check bound slots/constants (no allocation).
/// Phase 2: copy row via extend_from_slice (single memcpy) and fill unbound slots.
fn match_tuple_flat(atom: &CAtom, tuple: &Tuple, row: &[Option<Value>], out: &mut SlotRows) -> bool {
    if atom.terms.len() != tuple.len() {
        return false;
    }

    // Phase 1: Check all bound variables and constants
    for (term, value) in atom.terms.iter().zip(tuple.iter()) {
        match term {
            CTerm::Slot(s) => {
                if let Some(existing) = row[*s] {
                    if existing != *value {
                        return false;
                    }
                }
            }
            CTerm::Const(c) => {
                if *c != *value {
                    return false;
                }
            }
        }
    }

    // Phase 2: Copy row and fill unbound (one memcpy + a few indexed writes)
    let start = out.data.len();
    out.data.extend_from_slice(row);
    for (term, value) in atom.terms.iter().zip(tuple.iter()) {
        if let CTerm::Slot(s) = term {
            if out.data[start + *s].is_none() {
                out.data[start + *s] = Some(*value);
            }
        }
    }
    out.count += 1;
    true
}

/// Check if a compiled atom has any match in the database (for negation).
fn has_any_match_c(atom: &CAtom, db: &Database, row: &[Option<Value>]) -> Result<bool, EvalError> {
    let rel = match db.relation(&atom.predicate) {
        Some(r) => r,
        None => return Ok(false),
    };

    let mut bound_cols: SmallVec<[usize; 4]> = SmallVec::new();
    let mut bound_vals: SmallVec<[Value; 4]> = SmallVec::new();
    let mut all_bound = true;

    for (i, term) in atom.terms.iter().enumerate() {
        match term {
            CTerm::Const(v) => {
                bound_cols.push(i);
                bound_vals.push(*v);
            }
            CTerm::Slot(s) => {
                if let Some(val) = row[*s] {
                    bound_cols.push(i);
                    bound_vals.push(val);
                } else {
                    all_bound = false;
                }
            }
        }
    }

    if bound_cols.is_empty() {
        return Ok(!rel.is_empty());
    }

    if all_bound {
        return Ok(rel.lookup_any(&bound_cols, &bound_vals));
    }

    let mut found = false;
    rel.lookup_each(&bound_cols, &bound_vals, |tuple| {
        if !found {
            if check_tuple_match(atom, tuple, row) {
                found = true;
            }
        }
    });
    Ok(found)
}

/// Check if a tuple matches an atom given existing bindings (no allocation needed).
fn check_tuple_match(atom: &CAtom, tuple: &Tuple, row: &[Option<Value>]) -> bool {
    if atom.terms.len() != tuple.len() {
        return false;
    }
    for (term, value) in atom.terms.iter().zip(tuple.iter()) {
        match term {
            CTerm::Slot(s) => {
                if let Some(existing) = row[*s] {
                    if existing != *value {
                        return false;
                    }
                }
            }
            CTerm::Const(c) => {
                if *c != *value {
                    return false;
                }
            }
        }
    }
    true
}

/// Evaluate a compiled guard, pushing result into output buffer.
/// For pure filters (both sides bound), copies the row via extend_from_slice.
/// For binding guards (Eq with one unbound side), copies and sets the slot.
fn eval_guard_flat(left: &CTerm, op: CompOp, right: &CTerm, row: &[Option<Value>], out: &mut SlotRows) {
    let l = resolve_cterm(left, row);
    let r = resolve_cterm(right, row);

    match (l, r) {
        (Some(l), Some(r)) => {
            let pass = match op {
                CompOp::Eq => l == r,
                CompOp::Ne => l != r,
                CompOp::Lt => l < r,
                CompOp::Le => l <= r,
                CompOp::Gt => l > r,
                CompOp::Ge => l >= r,
            };
            if pass { out.push_row(row); }
        }
        (None, Some(r)) if op == CompOp::Eq => {
            if let CTerm::Slot(s) = left {
                out.push_row_with(row, *s, r);
            }
        }
        (Some(l), None) if op == CompOp::Eq => {
            if let CTerm::Slot(s) = right {
                out.push_row_with(row, *s, l);
            }
        }
        _ => {}
    }
}

/// Evaluate a compiled arithmetic expression.
fn eval_arith_c(left: &CTerm, op: ArithOp, right: &CTerm, row: &[Option<Value>]) -> Option<Value> {
    let l = resolve_cterm(left, row)?;
    let r = resolve_cterm(right, row)?;

    match (&l, &r) {
        (Value::Int(a), Value::Int(b)) => {
            let result = match op {
                ArithOp::Add => a.checked_add(*b)?,
                ArithOp::Sub => a.checked_sub(*b)?,
                ArithOp::Mul => a.checked_mul(*b)?,
                ArithOp::Div => { if *b == 0 { return None; } a.checked_div(*b)? }
                ArithOp::Mod => { if *b == 0 { return None; } a.checked_rem(*b)? }
            };
            Some(Value::Int(result))
        }
        (Value::Float(a), Value::Float(b)) => {
            let result = match op {
                ArithOp::Add => a.into_inner() + b.into_inner(),
                ArithOp::Sub => a.into_inner() - b.into_inner(),
                ArithOp::Mul => a.into_inner() * b.into_inner(),
                ArithOp::Div => { if b.into_inner() == 0.0 { return None; } a.into_inner() / b.into_inner() }
                ArithOp::Mod => { if b.into_inner() == 0.0 { return None; } a.into_inner() % b.into_inner() }
            };
            Some(Value::Float(ordered_float::OrderedFloat(result)))
        }
        (Value::Int(a), Value::Float(b)) => {
            let a = *a as f64;
            let b = b.into_inner();
            let result = match op {
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
            let result = match op {
                ArithOp::Add => a + b,
                ArithOp::Sub => a - b,
                ArithOp::Mul => a * b,
                ArithOp::Div => { if b == 0.0 { return None; } a / b }
                ArithOp::Mod => { if b == 0.0 { return None; } a % b }
            };
            Some(Value::Float(ordered_float::OrderedFloat(result)))
        }
        (Value::String(_), Value::String(_)) if op == ArithOp::Add => None,
        _ => None,
    }
}

/// Resolve a compiled term to a value using slot bindings.
fn resolve_cterm(term: &CTerm, row: &[Option<Value>]) -> Option<Value> {
    match term {
        CTerm::Slot(s) => row[*s],
        CTerm::Const(v) => Some(*v),
    }
}

/// Project compiled head atom terms to a tuple using slot bindings.
fn project_head_c(head: &CAtom, row: &[Option<Value>]) -> Option<Tuple> {
    let mut tuple: Tuple = SmallVec::new();
    for term in &head.terms {
        match resolve_cterm(term, row) {
            Some(v) => tuple.push(v),
            None => return None,
        }
    }
    Some(tuple)
}

/// Convert slot-based bindings back to HashMap (for aggregate sub-rule evaluation).
fn slots_to_bindings(row: &[Option<Value>], var_map: &HashMap<String, usize>) -> Bindings {
    let mut bindings = HashMap::new();
    for (name, &slot) in var_map {
        if let Some(val) = row[slot] {
            bindings.insert(name.clone(), val);
        }
    }
    bindings
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
