//! Datalog rule representation.
//!
//! A Datalog program is a set of rules. Each rule has a head atom and a body
//! consisting of positive atoms, negated atoms, guards, and aggregates.

use ocql_database::Value;

/// A Datalog rule: `head(...) :- body_element1, body_element2, ...`
#[derive(Debug, Clone)]
pub struct Rule {
    pub head: Atom,
    pub body: Vec<BodyElement>,
}

/// An atom: `predicate_name(term1, term2, ...)`
#[derive(Debug, Clone)]
pub struct Atom {
    pub predicate: String,
    pub terms: Vec<Term>,
}

/// A term in an atom — either a variable or a constant.
#[derive(Debug, Clone)]
pub enum Term {
    Var(String),
    Const(Value),
}

/// An element of a rule body.
#[derive(Debug, Clone)]
pub enum BodyElement {
    /// Positive atom: `p(x, y)`
    Positive(Atom),
    /// Negated atom: `not p(x, y)`
    Negated(Atom),
    /// Guard/filter: `x > 0`, `x = y`
    Guard(Guard),
    /// Aggregate: `result = agg(body_pred, group_by_vars, agg_var)`
    Aggregate {
        result_var: String,
        function: AggFunction,
        sub_rule: Box<Rule>,
        group_by: Vec<String>,
        agg_var: String,
    },
}

/// A guard (comparison between two terms).
#[derive(Debug, Clone)]
pub struct Guard {
    pub left: Term,
    pub op: CompOp,
    pub right: Term,
}

/// Comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Aggregate functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggFunction {
    Count,
    Sum,
    Min,
    Max,
}

/// A Datalog program: a named collection of rules.
#[derive(Debug, Clone)]
pub struct Program {
    pub rules: Vec<Rule>,
}

// ============================================================
// Builder helpers for constructing rules programmatically
// ============================================================

impl Rule {
    pub fn new(head: Atom, body: Vec<BodyElement>) -> Self {
        Self { head, body }
    }
}

impl Atom {
    pub fn new(predicate: &str, terms: Vec<Term>) -> Self {
        Self {
            predicate: predicate.to_string(),
            terms,
        }
    }
}

impl Program {
    pub fn new(rules: Vec<Rule>) -> Self {
        Self { rules }
    }

    /// Get all predicates that appear in rule heads (IDB predicates).
    pub fn head_predicates(&self) -> Vec<String> {
        let mut preds: Vec<String> = self.rules.iter()
            .map(|r| r.head.predicate.clone())
            .collect();
        preds.sort();
        preds.dedup();
        preds
    }

    /// Get all predicates that appear in rule bodies but never in heads (EDB predicates).
    pub fn base_predicates(&self) -> Vec<String> {
        let heads: std::collections::HashSet<&str> = self.rules.iter()
            .map(|r| r.head.predicate.as_str())
            .collect();

        let mut body_preds = std::collections::HashSet::new();
        for rule in &self.rules {
            for elem in &rule.body {
                match elem {
                    BodyElement::Positive(atom) | BodyElement::Negated(atom) => {
                        if !heads.contains(atom.predicate.as_str()) {
                            body_preds.insert(atom.predicate.clone());
                        }
                    }
                    _ => {}
                }
            }
        }

        let mut result: Vec<String> = body_preds.into_iter().collect();
        result.sort();
        result
    }
}

/// Convenience: variable term.
pub fn var(name: &str) -> Term {
    Term::Var(name.to_string())
}

/// Convenience: integer constant term.
pub fn int(v: i64) -> Term {
    Term::Const(Value::Int(v))
}

/// Convenience: entity constant term.
pub fn entity(id: u64) -> Term {
    Term::Const(Value::Entity(ocql_database::EntityId(id)))
}
