//! Negation stratification for Datalog programs.
//!
//! Groups rules into strata such that negated predicates are always
//! fully computed in a prior stratum. Uses Tarjan's SCC algorithm
//! on the predicate dependency graph.

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
    // 1. Collect all IDB predicates (those appearing in rule heads)
    let idb_preds: HashSet<String> = program.rules.iter()
        .map(|r| r.head.predicate.clone())
        .collect();

    // 2. Build predicate dependency graph
    let mut edges: HashMap<String, Vec<(String, EdgeKind)>> = HashMap::new();
    for pred in &idb_preds {
        edges.entry(pred.clone()).or_default();
    }

    for rule in &program.rules {
        let head = &rule.head.predicate;
        for elem in &rule.body {
            match elem {
                BodyElement::Positive(atom) => {
                    if idb_preds.contains(&atom.predicate) {
                        edges.entry(head.clone()).or_default()
                            .push((atom.predicate.clone(), EdgeKind::Positive));
                    }
                }
                BodyElement::Negated(atom) => {
                    if idb_preds.contains(&atom.predicate) {
                        edges.entry(head.clone()).or_default()
                            .push((atom.predicate.clone(), EdgeKind::Negative));
                    }
                }
                BodyElement::Aggregate { sub_rule, .. } => {
                    // Aggregated predicates are like negation (must be fully computed first)
                    for sub_elem in &sub_rule.body {
                        if let BodyElement::Positive(atom) = sub_elem {
                            if idb_preds.contains(&atom.predicate) {
                                edges.entry(head.clone()).or_default()
                                    .push((atom.predicate.clone(), EdgeKind::Negative));
                            }
                        }
                    }
                }
                BodyElement::Guard(_) | BodyElement::Assign { .. } => {}
            }
        }
    }

    // 3. Find SCCs using Tarjan's algorithm
    let pred_list: Vec<String> = idb_preds.iter().cloned().collect();
    let sccs = tarjan_scc(&pred_list, &edges);

    // 4. Check for negation within SCCs
    for scc in &sccs {
        if scc.len() == 1 {
            // Check self-negation
            let pred = &scc[0];
            if let Some(deps) = edges.get(pred) {
                for (dep, kind) in deps {
                    if dep == pred && *kind == EdgeKind::Negative {
                        return Err(StratificationError::NegationCycle(scc.clone()));
                    }
                }
            }
        } else {
            // Check for any negative edge within the SCC
            let scc_set: HashSet<&String> = scc.iter().collect();
            for pred in scc {
                if let Some(deps) = edges.get(pred) {
                    for (dep, kind) in deps {
                        if scc_set.contains(dep) && *kind == EdgeKind::Negative {
                            return Err(StratificationError::NegationCycle(scc.clone()));
                        }
                    }
                }
            }
        }
    }

    // 5. Build strata from SCCs (Tarjan outputs leaves-first, which is correct order)
    let mut strata = Vec::new();
    for scc in sccs {
        let pred_set: HashSet<String> = scc.iter().cloned().collect();

        // A stratum is recursive if the SCC has >1 node, or a self-loop
        let is_recursive = if scc.len() > 1 {
            true
        } else {
            let pred = &scc[0];
            edges.get(pred).map_or(false, |deps| {
                deps.iter().any(|(dep, kind)| dep == pred && *kind == EdgeKind::Positive)
            })
        };

        // Collect all rules whose head is in this SCC
        let rules: Vec<Rule> = program.rules.iter()
            .filter(|r| pred_set.contains(&r.head.predicate))
            .cloned()
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
// Tarjan's SCC algorithm
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
    edges: &HashMap<String, Vec<(String, EdgeKind)>>,
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

    // Tarjan produces SCCs in reverse topological order (leaves first)
    // which is what we want: evaluate base predicates first
    state.result
}

fn strongconnect(
    v: &str,
    edges: &HashMap<String, Vec<(String, EdgeKind)>>,
    state: &mut TarjanState,
) {
    state.index.insert(v.to_string(), state.index_counter);
    state.lowlink.insert(v.to_string(), state.index_counter);
    state.index_counter += 1;
    state.stack.push(v.to_string());
    state.on_stack.insert(v.to_string());

    if let Some(neighbors) = edges.get(v) {
        for (w, _kind) in neighbors {
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

    // If v is a root node, pop the SCC
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
}
