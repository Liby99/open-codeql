//! LIR node definitions.
//!
//! A LIR program is a stratified sequence of relational algebra rules.
//! The core join primitive is worst-case optimal join (WCO join).

use std::fmt;

// ============================================================
// Program
// ============================================================

/// A complete LIR program: an ordered list of strata.
///
/// Strata must be evaluated in order. Within a recursive stratum,
/// rules are evaluated to a fixpoint using semi-naive iteration.
#[derive(Debug, Clone)]
pub struct LirProgram {
    pub strata: Vec<LirStratum>,
}

impl LirProgram {
    /// All relation names defined by this program (IDB relations).
    pub fn defined_relations(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.strata.iter()
            .flat_map(|s| s.rules.iter().map(|r| r.target.as_str()))
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// Total number of rules across all strata.
    pub fn rule_count(&self) -> usize {
        self.strata.iter().map(|s| s.rules.len()).sum()
    }
}

// ============================================================
// Stratum
// ============================================================

/// A stratum: a group of rules that can be evaluated together.
///
/// Non-recursive strata are evaluated in a single pass.
/// Recursive strata require semi-naive fixpoint iteration.
#[derive(Debug, Clone)]
pub struct LirStratum {
    pub rules: Vec<LirRule>,
    pub recursive: bool,
}

// ============================================================
// Rule
// ============================================================

/// A LIR rule: `target(columns) ← body`.
///
/// Each rule inserts the result of its body plan into the target relation.
/// Multiple rules for the same target produce a union.
#[derive(Debug, Clone)]
pub struct LirRule {
    /// Target relation name.
    pub target: String,
    /// Columns in the target relation, in order.
    pub target_columns: Vec<String>,
    /// The relational algebra plan that produces tuples.
    pub body: LirPlan,
}

// ============================================================
// Plan (relational algebra)
// ============================================================

/// A relational algebra plan — the core of the LIR.
#[derive(Debug, Clone)]
pub enum LirPlan {
    /// Scan a base or derived relation.
    ///
    /// Each binding maps a relation column (by position) to a variable name
    /// used in the enclosing plan.
    Scan {
        relation: String,
        bindings: Vec<LirBinding>,
    },

    /// Worst-case optimal join over multiple atoms.
    ///
    /// This is the primary join strategy. Given N atoms sharing variables,
    /// the WCO join iterates over a variable ordering, using index lookups
    /// (leapfrog trie join) to intersect candidate values for each variable.
    ///
    /// Semantics: the natural join of all atoms, projected to `project`.
    WcoJoin {
        /// The atoms participating in the join.
        atoms: Vec<LirAtom>,
        /// Variable ordering for the leapfrog iteration.
        /// Variables are bound in this order; each atom provides an iterator
        /// over candidate values for the current variable, filtered by
        /// previously-bound variables.
        variable_order: Vec<String>,
        /// Variables to keep in the output (projection after join).
        project: Vec<String>,
    },

    /// Filter: keep only tuples satisfying a condition.
    Filter {
        input: Box<LirPlan>,
        condition: LirFilter,
    },

    /// Project: keep only the named columns.
    Project {
        input: Box<LirPlan>,
        columns: Vec<String>,
    },

    /// Union: combine results from multiple sub-plans.
    ///
    /// Used for disjunctions and multiple rules targeting the same relation.
    Union {
        inputs: Vec<LirPlan>,
    },

    /// Anti-join: tuples from `positive` with no match in `negative`.
    ///
    /// Used for stratified negation: `not P(x)` becomes anti-join
    /// on the columns shared between the positive context and P.
    AntiJoin {
        positive: Box<LirPlan>,
        negative: Box<LirPlan>,
        /// Columns to match on (the shared variables).
        key_columns: Vec<String>,
    },

    /// Aggregation: group-by + aggregate function.
    Aggregate {
        input: Box<LirPlan>,
        group_by: Vec<String>,
        function: LirAggFunction,
        /// Column in the input to aggregate over.
        agg_column: String,
        /// Column name for the aggregation result.
        result_column: String,
    },

    /// Extend: compute a new column from existing ones.
    ///
    /// Used for arithmetic assignments like `z = x + y`.
    Extend {
        input: Box<LirPlan>,
        column: String,
        expr: LirExpr,
    },

    /// Constant: a literal inline relation.
    Constant {
        columns: Vec<String>,
        rows: Vec<Vec<LirValue>>,
    },
}

// ============================================================
// Atoms (for WCO join)
// ============================================================

/// An atom in a WCO join: a relation scan with variable bindings.
///
/// Each atom contributes iterators for the variables it binds.
/// During WCO join execution, atoms are indexed by their bound
/// variable prefixes.
#[derive(Debug, Clone)]
pub struct LirAtom {
    /// Relation to scan.
    pub relation: String,
    /// How each column of the relation maps to join variables.
    pub bindings: Vec<LirBinding>,
}

/// A column binding: maps a relation column to a variable or constant.
#[derive(Debug, Clone)]
pub enum LirBinding {
    /// Bind this column to a named variable.
    Var(String),
    /// Filter this column to a constant value.
    Const(LirValue),
}

impl LirBinding {
    pub fn var(name: &str) -> Self {
        LirBinding::Var(name.to_string())
    }

    pub fn int(v: i64) -> Self {
        LirBinding::Const(LirValue::Int(v))
    }

    /// Returns the variable name if this is a Var binding.
    pub fn as_var(&self) -> Option<&str> {
        match self {
            LirBinding::Var(name) => Some(name),
            LirBinding::Const(_) => None,
        }
    }
}

// ============================================================
// Filter conditions
// ============================================================

/// A filter condition on tuples.
#[derive(Debug, Clone)]
pub enum LirFilter {
    /// Comparison between two operands.
    Comparison {
        left: LirOperand,
        op: LirCompOp,
        right: LirOperand,
    },
    /// Conjunction of conditions.
    And(Vec<LirFilter>),
}

/// An operand in a filter.
#[derive(Debug, Clone)]
pub enum LirOperand {
    /// Reference to a column/variable.
    Column(String),
    /// Literal value.
    Literal(LirValue),
}

// ============================================================
// Expressions
// ============================================================

/// An arithmetic expression for Extend nodes.
#[derive(Debug, Clone)]
pub struct LirExpr {
    pub left: LirOperand,
    pub op: LirArithOp,
    pub right: LirOperand,
}

/// Arithmetic operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LirArithOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

impl fmt::Display for LirArithOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LirArithOp::Add => write!(f, "+"),
            LirArithOp::Sub => write!(f, "-"),
            LirArithOp::Mul => write!(f, "*"),
            LirArithOp::Div => write!(f, "/"),
            LirArithOp::Mod => write!(f, "%"),
        }
    }
}

// ============================================================
// Comparison
// ============================================================

/// Comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LirCompOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl fmt::Display for LirCompOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LirCompOp::Eq => write!(f, "="),
            LirCompOp::Ne => write!(f, "!="),
            LirCompOp::Lt => write!(f, "<"),
            LirCompOp::Le => write!(f, "<="),
            LirCompOp::Gt => write!(f, ">"),
            LirCompOp::Ge => write!(f, ">="),
        }
    }
}

// ============================================================
// Aggregation
// ============================================================

/// Aggregate functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LirAggFunction {
    Count,
    Sum,
    Min,
    Max,
    Avg,
    Concat,
    Rank,
    StrictCount,
    StrictSum,
    StrictConcat,
    Any,
}

impl fmt::Display for LirAggFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LirAggFunction::Count => write!(f, "count"),
            LirAggFunction::Sum => write!(f, "sum"),
            LirAggFunction::Min => write!(f, "min"),
            LirAggFunction::Max => write!(f, "max"),
            LirAggFunction::Avg => write!(f, "avg"),
            LirAggFunction::Concat => write!(f, "concat"),
            LirAggFunction::Rank => write!(f, "rank"),
            LirAggFunction::StrictCount => write!(f, "strictcount"),
            LirAggFunction::StrictSum => write!(f, "strictsum"),
            LirAggFunction::StrictConcat => write!(f, "strictconcat"),
            LirAggFunction::Any => write!(f, "any"),
        }
    }
}

// ============================================================
// Values
// ============================================================

/// A literal value in the LIR.
#[derive(Debug, Clone, PartialEq)]
pub enum LirValue {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
}

impl fmt::Display for LirValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LirValue::Int(v) => write!(f, "{}", v),
            LirValue::Float(v) => write!(f, "{}", v),
            LirValue::String(s) => write!(f, "\"{}\"", s),
            LirValue::Bool(b) => write!(f, "{}", b),
        }
    }
}
