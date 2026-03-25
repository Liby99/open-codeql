//! MIR node definitions.
//!
//! A MIR program is a flat collection of predicates. Each predicate has typed
//! parameters and a body that is a disjunction of conjunctions.

use std::fmt;

// ============================================================
// Program
// ============================================================

/// A complete MIR program: a collection of named predicates.
#[derive(Debug, Clone)]
pub struct MirProgram {
    pub predicates: Vec<MirPredicate>,
}

impl MirProgram {
    pub fn new() -> Self {
        Self {
            predicates: Vec::new(),
        }
    }

    /// Find a predicate by name.
    pub fn find_predicate(&self, name: &str) -> Option<&MirPredicate> {
        self.predicates.iter().find(|p| p.name == name)
    }

    /// All predicate names.
    pub fn predicate_names(&self) -> Vec<&str> {
        self.predicates.iter().map(|p| p.name.as_str()).collect()
    }
}

// ============================================================
// Predicate
// ============================================================

/// A MIR predicate — the fundamental unit of computation.
#[derive(Debug, Clone)]
pub struct MirPredicate {
    /// Fully qualified name (e.g., "SmallInt#char", "SmallInt#double", "myPred")
    pub name: String,
    /// Parameters including implicit `this` and `result` where applicable.
    pub params: Vec<MirParam>,
    /// The predicate body: a disjunction of rule clauses.
    pub body: MirBody,
    /// Annotations that affect compilation.
    pub annotations: MirAnnotations,
    /// Whether this predicate is abstract (no body, implemented by subclasses).
    pub is_abstract: bool,
}

/// A predicate parameter with name and type.
#[derive(Debug, Clone, PartialEq)]
pub struct MirParam {
    pub name: String,
    pub ty: MirType,
}

impl MirParam {
    pub fn new(name: &str, ty: MirType) -> Self {
        Self {
            name: name.to_string(),
            ty,
        }
    }
}

// ============================================================
// Body
// ============================================================

/// A predicate body: union of rule clauses.
#[derive(Debug, Clone)]
pub enum MirBody {
    /// A single conjunction (the common case): all atoms must be satisfied.
    Conjunction(Vec<MirAtom>),
    /// A disjunction of conjunctions (from `or`, overrides, etc.).
    Disjunction(Vec<Vec<MirAtom>>),
    /// No body — for external/abstract predicates.
    None,
}

impl MirBody {
    /// Returns true if this body produces no results (empty disjunction).
    pub fn is_empty(&self) -> bool {
        match self {
            MirBody::Conjunction(atoms) => atoms.is_empty(),
            MirBody::Disjunction(clauses) => clauses.is_empty(),
            MirBody::None => true,
        }
    }

    /// Number of clauses in this body.
    pub fn clause_count(&self) -> usize {
        match self {
            MirBody::Conjunction(_) => 1,
            MirBody::Disjunction(clauses) => clauses.len(),
            MirBody::None => 0,
        }
    }
}

// ============================================================
// Atoms
// ============================================================

/// An atom in a rule body — the basic building block.
#[derive(Debug, Clone)]
pub enum MirAtom {
    /// Positive predicate call: `pred(t1, t2, ...)`
    Scan(MirScan),
    /// Negated predicate call: `not pred(t1, t2, ...)`
    NegScan(MirScan),
    /// Comparison guard: `t1 op t2`
    Guard(MirGuard),
    /// Variable binding via arithmetic: `result_var = left op right`
    Assign(MirAssign),
    /// Aggregation: `result_var = agg_fn(sub_predicate, group_by, agg_var)`
    Aggregate(MirAggregate),
    /// Type check: checks variable belongs to a type via its characteristic predicate.
    TypeCheck(MirTypeCheck),
}

/// A predicate scan (lookup/join).
#[derive(Debug, Clone)]
pub struct MirScan {
    pub predicate: String,
    pub args: Vec<MirTerm>,
}

impl MirScan {
    pub fn new(predicate: &str, args: Vec<MirTerm>) -> Self {
        Self {
            predicate: predicate.to_string(),
            args,
        }
    }
}

/// A comparison guard.
#[derive(Debug, Clone)]
pub struct MirGuard {
    pub left: MirTerm,
    pub op: MirCompOp,
    pub right: MirTerm,
}

/// An arithmetic assignment.
#[derive(Debug, Clone)]
pub struct MirAssign {
    pub result_var: String,
    pub expr: MirArithExpr,
}

/// A type check — lowered from `instanceof`.
#[derive(Debug, Clone)]
pub struct MirTypeCheck {
    pub var: String,
    pub type_predicate: String,
}

/// An aggregation computation.
#[derive(Debug, Clone)]
pub struct MirAggregate {
    pub result_var: String,
    pub function: MirAggFunction,
    /// The sub-query predicate that produces tuples for aggregation.
    pub sub_predicate: String,
    /// Variables from outer scope that ground the sub-query (group-by).
    pub group_by: Vec<String>,
    /// The variable in the sub-query whose values are aggregated.
    pub agg_var: String,
}

// ============================================================
// Terms
// ============================================================

/// A term — a value reference in atoms.
#[derive(Debug, Clone, PartialEq)]
pub enum MirTerm {
    /// Variable reference.
    Var(String),
    /// Literal constant.
    Const(MirConst),
    /// Wildcard (anonymous variable, don't-care).
    Wildcard,
}

impl MirTerm {
    pub fn var(name: &str) -> Self {
        MirTerm::Var(name.to_string())
    }

    pub fn int(v: i64) -> Self {
        MirTerm::Const(MirConst::Int(v))
    }

    pub fn float(v: f64) -> Self {
        MirTerm::Const(MirConst::Float(v))
    }

    pub fn string(s: &str) -> Self {
        MirTerm::Const(MirConst::String(s.to_string()))
    }

    pub fn bool(b: bool) -> Self {
        MirTerm::Const(MirConst::Bool(b))
    }
}

// ============================================================
// Constants
// ============================================================

/// Constant values.
#[derive(Debug, Clone, PartialEq)]
pub enum MirConst {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
}

impl fmt::Display for MirConst {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MirConst::Int(v) => write!(f, "{}", v),
            MirConst::Float(v) => write!(f, "{}", v),
            MirConst::String(s) => write!(f, "\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
            MirConst::Bool(b) => write!(f, "{}", b),
        }
    }
}

// ============================================================
// Arithmetic
// ============================================================

/// An arithmetic expression.
#[derive(Debug, Clone)]
pub struct MirArithExpr {
    pub left: MirTerm,
    pub op: MirArithOp,
    pub right: MirTerm,
}

/// Arithmetic operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirArithOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

impl fmt::Display for MirArithOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MirArithOp::Add => write!(f, "+"),
            MirArithOp::Sub => write!(f, "-"),
            MirArithOp::Mul => write!(f, "*"),
            MirArithOp::Div => write!(f, "/"),
            MirArithOp::Mod => write!(f, "%"),
        }
    }
}

// ============================================================
// Comparison
// ============================================================

/// Comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirCompOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl fmt::Display for MirCompOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MirCompOp::Eq => write!(f, "="),
            MirCompOp::Ne => write!(f, "!="),
            MirCompOp::Lt => write!(f, "<"),
            MirCompOp::Le => write!(f, "<="),
            MirCompOp::Gt => write!(f, ">"),
            MirCompOp::Ge => write!(f, ">="),
        }
    }
}

// ============================================================
// Aggregation
// ============================================================

/// Aggregate functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirAggFunction {
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

impl fmt::Display for MirAggFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MirAggFunction::Count => write!(f, "count"),
            MirAggFunction::Sum => write!(f, "sum"),
            MirAggFunction::Min => write!(f, "min"),
            MirAggFunction::Max => write!(f, "max"),
            MirAggFunction::Avg => write!(f, "avg"),
            MirAggFunction::Concat => write!(f, "concat"),
            MirAggFunction::Rank => write!(f, "rank"),
            MirAggFunction::StrictCount => write!(f, "strictcount"),
            MirAggFunction::StrictSum => write!(f, "strictsum"),
            MirAggFunction::StrictConcat => write!(f, "strictconcat"),
            MirAggFunction::Any => write!(f, "any"),
        }
    }
}

// ============================================================
// Types
// ============================================================

/// MIR-level type representation (simplified from HIR).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MirType {
    Int,
    Float,
    String,
    Boolean,
    Date,
    /// Database entity type (from .dbscheme), e.g., "@function"
    Entity(std::string::String),
    /// User-defined class type — resolved to its characteristic predicate
    Class(std::string::String),
    /// Any type (for unresolved or polymorphic cases)
    Any,
}

impl fmt::Display for MirType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MirType::Int => write!(f, "int"),
            MirType::Float => write!(f, "float"),
            MirType::String => write!(f, "string"),
            MirType::Boolean => write!(f, "boolean"),
            MirType::Date => write!(f, "date"),
            MirType::Entity(name) => write!(f, "@{}", name),
            MirType::Class(name) => write!(f, "{}", name),
            MirType::Any => write!(f, "any"),
        }
    }
}

// ============================================================
// Annotations
// ============================================================

/// Annotations that affect MIR compilation and optimization.
#[derive(Debug, Clone, Default)]
pub struct MirAnnotations {
    pub cached: bool,
    pub nomagic: bool,
    pub noinline: bool,
    pub inline: bool,
    pub inline_late: bool,
    pub binding_set: Vec<Vec<String>>,
}

// ============================================================
// Builder helpers
// ============================================================

impl MirPredicate {
    /// Create a simple predicate with a conjunction body.
    pub fn new(name: &str, params: Vec<MirParam>, atoms: Vec<MirAtom>) -> Self {
        Self {
            name: name.to_string(),
            params,
            body: MirBody::Conjunction(atoms),
            annotations: MirAnnotations::default(),
            is_abstract: false,
        }
    }

    /// Create an abstract predicate with no body.
    pub fn abstract_pred(name: &str, params: Vec<MirParam>) -> Self {
        Self {
            name: name.to_string(),
            params,
            body: MirBody::None,
            annotations: MirAnnotations::default(),
            is_abstract: true,
        }
    }
}
