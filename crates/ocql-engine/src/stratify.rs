//! Negation stratification for Datalog programs.
//!
//! Groups rules into strata such that negated predicates are always
//! fully computed in a prior stratum. Uses Tarjan's SCC algorithm
//! on the predicate dependency graph with integer-indexed arrays
//! for performance on large programs (48K+ predicates).

use std::collections::{HashMap, HashSet};

use crate::rule::{BodyElement, Program, Rule};

/// A stratum is a group of rules that can be evaluated together.
#[derive(Debug)]
pub struct Stratum {
    pub rules: Vec<Rule>,
    pub predicates: HashSet<String>,
    pub is_recursive: bool,
}

/// Error returned when a program cannot be stratified.
#[derive(Debug)]
pub enum StratificationError {
    /// A negation cycle was found (predicate depends negatively on itself).
    NegationCycle(Vec<String>),
}

impl std::fmt::Display for StratificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StratificationError::NegationCycle(preds) => {
                write!(f, "negation cycle through predicates: {}", preds.join(", "))
            }
        }
    }
}

impl std::error::Error for StratificationError {}

/// Edge kind in the predicate dependency graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EdgeKind {
    Positive,
    Negative,
}

/// Compute negation stratification for a Datalog program.
///
/// Returns an ordered list of strata (evaluate in order) or an error
/// if the program is not stratifiable (negation cycle).
pub fn stratify(program: &Program) -> Result<Vec<Stratum>, StratificationError> {
    // 1. Collect all IDB predicates and assign numeric indices
    let idb_preds: HashSet<&str> = program.rules.iter()
        .map(|r| r.head.predicate.as_str())
        .collect();

    let mut pred_names: Vec<&str> = idb_preds.iter().copied().collect();
    pred_names.sort_unstable(); // deterministic ordering

    let pred_to_idx: HashMap<&str, usize> = pred_names.iter()
        .enumerate()
        .map(|(i, &p)| (p, i))
        .collect();
    let n = pred_names.len();

    // 2. Build integer-indexed dependency graph
    let mut edges: Vec<Vec<(usize, EdgeKind)>> = vec![Vec::new(); n];

    for rule in &program.rules {
        if let Some(&head_idx) = pred_to_idx.get(rule.head.predicate.as_str()) {
            for elem in &rule.body {
                match elem {
                    BodyElement::Positive(atom) => {
                        if let Some(&dep_idx) = pred_to_idx.get(atom.predicate.as_str()) {
                            edges[head_idx].push((dep_idx, EdgeKind::Positive));
                        }
                    }
                    BodyElement::Negated(atom) => {
                        if let Some(&dep_idx) = pred_to_idx.get(atom.predicate.as_str()) {
                            edges[head_idx].push((dep_idx, EdgeKind::Negative));
                        }
                    }
                    BodyElement::Aggregate { sub_rule, .. } => {
                        for sub_elem in &sub_rule.body {
                            if let BodyElement::Positive(atom) = sub_elem {
                                if let Some(&dep_idx) = pred_to_idx.get(atom.predicate.as_str()) {
                                    edges[head_idx].push((dep_idx, EdgeKind::Negative));
                                }
                            }
                        }
                    }
                    BodyElement::Guard(_) | BodyElement::Assign { .. } => {}
                }
            }
        }
    }

    // 3. Find SCCs using iterative Tarjan's algorithm (avoids stack overflow on deep chains)
    let sccs = tarjan_scc_iterative(n, &edges);

    // 4. Check for negation within SCCs
    //
    // Negative edges to compiler-generated auxiliary predicates (names starting
    // with `_`) are artifacts of double-negation lowering and are benign.
    for scc in &sccs {
        if scc.len() == 1 {
            let pred_idx = scc[0];
            for &(dep, kind) in &edges[pred_idx] {
                if dep == pred_idx && kind == EdgeKind::Negative {
                    return Err(StratificationError::NegationCycle(
                        scc.iter().map(|&i| pred_names[i].to_string()).collect()
                    ));
                }
            }
        } else {
            let scc_set: Vec<bool> = {
                let mut v = vec![false; n];
                for &idx in scc { v[idx] = true; }
                v
            };
            let mut has_problematic_neg = false;

            'outer: for &pred_idx in scc {
                for &(dep, kind) in &edges[pred_idx] {
                    if scc_set[dep] && kind == EdgeKind::Negative {
                        if !pred_names[dep].starts_with('_') {
                            has_problematic_neg = true;
                            break 'outer;
                        }
                    }
                }
            }

            if has_problematic_neg {
                return Err(StratificationError::NegationCycle(
                    scc.iter().map(|&i| pred_names[i].to_string()).collect()
                ));
            }
        }
    }

    // 5. Pre-group rules by head predicate for efficient stratum construction
    let mut rules_by_pred: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, rule) in program.rules.iter().enumerate() {
        rules_by_pred.entry(rule.head.predicate.as_str())
            .or_default()
            .push(i);
    }

    // 6. Build strata from SCCs (Tarjan outputs leaves-first = correct eval order)
    let mut strata = Vec::with_capacity(sccs.len());
    for scc in sccs {
        let is_recursive = if scc.len() > 1 {
            true
        } else {
            let pred_idx = scc[0];
            edges[pred_idx].iter().any(|&(dep, kind)| dep == pred_idx && kind == EdgeKind::Positive)
        };

        let pred_set: HashSet<String> = scc.iter()
            .map(|&i| pred_names[i].to_string())
            .collect();

        // Collect rules efficiently using pre-grouped index
        let rules: Vec<Rule> = scc.iter()
            .flat_map(|&pred_idx| {
                rules_by_pred.get(pred_names[pred_idx])
                    .into_iter()
                    .flat_map(|indices| indices.iter().map(|&i| program.rules[i].clone()))
            })
            .collect();

        strata.push(Stratum {
            rules,
            predicates: pred_set,
            is_recursive,
        });
    }

    Ok(strata)
}

// ============================================================
// Iterative Tarjan's SCC algorithm (integer-indexed)
// ============================================================

/// Frame for the iterative Tarjan DFS.
struct TarjanFrame {
    node: usize,
    edge_idx: usize,  // which neighbor we're processing next
}

fn tarjan_scc_iterative(n: usize, edges: &[Vec<(usize, EdgeKind)>]) -> Vec<Vec<usize>> {
    const UNVISITED: usize = usize::MAX;

    let mut index = vec![UNVISITED; n];
    let mut lowlink = vec![0usize; n];
    let mut on_stack = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut counter: usize = 0;
    let mut result: Vec<Vec<usize>> = Vec::new();

    // DFS call stack (replaces recursion)
    let mut call_stack: Vec<TarjanFrame> = Vec::new();

    for start in 0..n {
        if index[start] != UNVISITED {
            continue;
        }

        // Start DFS from `start`
        index[start] = counter;
        lowlink[start] = counter;
        counter += 1;
        stack.push(start);
        on_stack[start] = true;
        call_stack.push(TarjanFrame { node: start, edge_idx: 0 });

        while let Some(frame) = call_stack.last_mut() {
            let v = frame.node;

            if frame.edge_idx < edges[v].len() {
                let (w, _) = edges[v][frame.edge_idx];
                frame.edge_idx += 1;

                if index[w] == UNVISITED {
                    // Push new frame (equivalent to recursive call)
                    index[w] = counter;
                    lowlink[w] = counter;
                    counter += 1;
                    stack.push(w);
                    on_stack[w] = true;
                    call_stack.push(TarjanFrame { node: w, edge_idx: 0 });
                } else if on_stack[w] {
                    lowlink[v] = lowlink[v].min(index[w]);
                }
            } else {
                // All neighbors processed — equivalent to returning from recursive call
                let v = frame.node;
                let v_lowlink = lowlink[v];

                // Pop this frame
                call_stack.pop();

                // Update parent's lowlink
                if let Some(parent) = call_stack.last() {
                    lowlink[parent.node] = lowlink[parent.node].min(v_lowlink);
                }

                // If v is a root node, pop the SCC
                if v_lowlink == index[v] {
                    let mut scc = Vec::new();
                    loop {
                        let w = stack.pop().unwrap();
                        on_stack[w] = false;
                        scc.push(w);
                        if w == v { break; }
                    }
                    result.push(scc);
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule::{Atom, var};

    fn pos(pred: &str, terms: Vec<Term>) -> BodyElement {
        BodyElement::Positive(Atom::new(pred, terms))
    }

    fn neg(pred: &str, terms: Vec<Term>) -> BodyElement {
        BodyElement::Negated(Atom::new(pred, terms))
    }

    use crate::rule::Term;

    #[test]
    fn test_non_recursive_stratification() {
        // edge(x, y) is EDB
        // path(x, y) :- edge(x, y).
        // no_path(x, y) :- node(x), node(y), not path(x, y).
        let program = Program::new(vec![
            Rule::new(
                Atom::new("path", vec![var("x"), var("y")]),
                vec![pos("edge", vec![var("x"), var("y")])],
            ),
            Rule::new(
                Atom::new("no_path", vec![var("x"), var("y")]),
                vec![
                    pos("node", vec![var("x")]),
                    pos("node", vec![var("y")]),
                    neg("path", vec![var("x"), var("y")]),
                ],
            ),
        ]);

        let strata = stratify(&program).unwrap();
        assert_eq!(strata.len(), 2);
        // path should be in stratum 0 (computed first)
        assert!(strata[0].predicates.contains("path"));
        assert!(!strata[0].is_recursive);
        // no_path in stratum 1
        assert!(strata[1].predicates.contains("no_path"));
        assert!(!strata[1].is_recursive);
    }

    #[test]
    fn test_recursive_stratification() {
        // edge(x, y) is EDB
        // path(x, y) :- edge(x, y).
        // path(x, y) :- path(x, z), edge(z, y).
        let program = Program::new(vec![
            Rule::new(
                Atom::new("path", vec![var("x"), var("y")]),
                vec![pos("edge", vec![var("x"), var("y")])],
            ),
            Rule::new(
                Atom::new("path", vec![var("x"), var("y")]),
                vec![
                    pos("path", vec![var("x"), var("z")]),
                    pos("edge", vec![var("z"), var("y")]),
                ],
            ),
        ]);

        let strata = stratify(&program).unwrap();
        assert_eq!(strata.len(), 1);
        assert!(strata[0].is_recursive);
        assert!(strata[0].predicates.contains("path"));
    }

    #[test]
    fn test_negation_cycle_error() {
        // p(x) :- not q(x).
        // q(x) :- not p(x).
        let program = Program::new(vec![
            Rule::new(
                Atom::new("p", vec![var("x")]),
                vec![neg("q", vec![var("x")])],
            ),
            Rule::new(
                Atom::new("q", vec![var("x")]),
                vec![neg("p", vec![var("x")])],
            ),
        ]);

        let result = stratify(&program);
        assert!(result.is_err());
    }

    #[test]
    fn test_self_negation_error() {
        // p(x) :- node(x), not p(x).
        let program = Program::new(vec![
            Rule::new(
                Atom::new("p", vec![var("x")]),
                vec![
                    pos("node", vec![var("x")]),
                    neg("p", vec![var("x")]),
                ],
            ),
        ]);

        let result = stratify(&program);
        assert!(result.is_err());
    }

    #[test]
    fn test_double_negation_forall_pattern_allowed() {
        // Simulates the forall lowering pattern:
        //   parent(x) :- base(x), not _forall_neg_1(x).
        //   _forall_neg_1(x) :- guard(x), not _neg_2(x).
        //   _neg_2(x) :- parent(x).
        //
        // This creates an apparent negation cycle:
        //   parent -> (neg) -> _forall_neg_1 -> (neg) -> _neg_2 -> (pos) -> parent
        // But the double negation makes it well-founded.
        let program = Program::new(vec![
            Rule::new(
                Atom::new("parent", vec![var("x")]),
                vec![
                    pos("base", vec![var("x")]),
                    neg("_forall_neg_1", vec![var("x")]),
                ],
            ),
            Rule::new(
                Atom::new("_forall_neg_1", vec![var("x")]),
                vec![
                    pos("guard", vec![var("x")]),
                    neg("_neg_2", vec![var("x")]),
                ],
            ),
            Rule::new(
                Atom::new("_neg_2", vec![var("x")]),
                vec![pos("parent", vec![var("x")])],
            ),
        ]);

        // Should succeed — double negation is well-founded
        let strata = stratify(&program).unwrap();
        // All three predicates are in the same SCC
        let has_parent = strata.iter().any(|s| s.predicates.contains("parent"));
        assert!(has_parent);
    }

    #[test]
    fn test_mixed_negation_cycle_with_user_pred_target_error() {
        // A negation cycle where a negative edge targets a user-defined predicate:
        //   p(x) :- base(x), not _neg_1(x).
        //   _neg_1(x) :- not p(x).
        //
        // The negative edge _neg_1 -> p targets user-defined "p", so this is rejected.
        let program = Program::new(vec![
            Rule::new(
                Atom::new("p", vec![var("x")]),
                vec![
                    pos("base", vec![var("x")]),
                    neg("_neg_1", vec![var("x")]),
                ],
            ),
            Rule::new(
                Atom::new("_neg_1", vec![var("x")]),
                vec![neg("p", vec![var("x")])],
            ),
        ]);

        let result = stratify(&program);
        assert!(result.is_err());
    }
}
