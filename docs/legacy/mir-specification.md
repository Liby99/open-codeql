# MIR (Mid-level IR) Specification

## Overview

MIR is the central IR in the open-cql compilation pipeline. It bridges the gap between
the high-level, object-oriented QL language and the flat Datalog rules consumed by the
evaluation engine. MIR is a **typed, flat, relational representation** where every QL
construct (classes, modules, aggregations, closures) has been lowered to named predicates
with explicit rule bodies.

```
QL Source → Parse → AST → HIR (name resolution, typing) → MIR → Engine (Datalog evaluation)
```

## Design Goals

1. **Faithful semantics** — MIR preserves the meaning of every QL program
2. **Flat structure** — No nested classes/modules; just predicates and rules
3. **Explicit dispatching** — Override resolution is materialized as rule disjunctions
4. **Inspectable** — S-expression and Datalog-style textual formats for debugging
5. **Optimizable** — Clean structure amenable to constant folding, dead code elimination, inlining
6. **Incrementally compilable** — Features can be lowered one at a time

## 1. MIR Node Definitions

### 1.1 Program

```rust
/// A complete MIR program: a collection of named predicates.
pub struct MirProgram {
    pub predicates: Vec<MirPredicate>,
}
```

### 1.2 Predicate

```rust
/// A MIR predicate — the fundamental unit of computation.
pub struct MirPredicate {
    /// Fully qualified name (e.g., "SmallInt#char", "SmallInt#double", "myPred")
    pub name: String,
    /// Parameters including implicit `this` and `result` where applicable
    pub params: Vec<MirParam>,
    /// The predicate body: a disjunction of rule bodies (union of conjunctions)
    pub body: MirBody,
    /// Annotations that affect compilation (cached, inline, etc.)
    pub annotations: MirAnnotations,
    /// Whether this predicate is abstract (no body, implemented by subclasses)
    pub is_abstract: bool,
}

/// A predicate parameter with name and type.
pub struct MirParam {
    pub name: String,
    pub ty: MirType,
}
```

### 1.3 Body (Disjunction of Conjunctions)

```rust
/// A predicate body: union of rule clauses.
/// Empty disjunction = `none()` (always false).
/// Each clause is a conjunction of atoms.
pub enum MirBody {
    /// A single conjunction (the common case)
    Conjunction(Vec<MirAtom>),
    /// A disjunction of conjunctions (from `or`, overrides, etc.)
    Disjunction(Vec<Vec<MirAtom>>),
    /// No body — for external/abstract predicates
    None,
}
```

### 1.4 Atoms (Body Elements)

```rust
/// An atom in a rule body — the basic building block.
pub enum MirAtom {
    /// Positive predicate call: `pred(t1, t2, ...)`
    Scan(MirScan),
    /// Negated predicate call: `not pred(t1, t2, ...)`
    NegScan(MirScan),
    /// Comparison guard: `t1 op t2`
    Guard(MirGuard),
    /// Variable binding via arithmetic: `result_var = left op right`
    Assign(MirAssign),
    /// Aggregation: `result_var = agg_fn(vars | guard | expr)`
    Aggregate(MirAggregate),
    /// Type check: `instanceof(x, TypeName)` — checks x belongs to a type
    TypeCheck(MirTypeCheck),
}

/// A predicate scan (lookup/join).
pub struct MirScan {
    pub predicate: String,
    pub args: Vec<MirTerm>,
}

/// A comparison guard.
pub struct MirGuard {
    pub left: MirTerm,
    pub op: MirCompOp,
    pub right: MirTerm,
}

/// An arithmetic assignment.
pub struct MirAssign {
    pub result_var: String,
    pub expr: MirArithExpr,
}

/// A type check — lowered from `instanceof`.
pub struct MirTypeCheck {
    pub var: String,
    pub type_predicate: String, // The characteristic predicate of the type
}

/// An aggregation computation.
pub struct MirAggregate {
    pub result_var: String,
    pub function: MirAggFunction,
    /// The sub-query that produces tuples for aggregation
    pub sub_predicate: String,
    /// Variables from outer scope that ground the sub-query (group-by)
    pub group_by: Vec<String>,
    /// The variable in the sub-query whose values are aggregated
    pub agg_var: String,
}
```

### 1.5 Terms

```rust
/// A term — a value reference in atoms.
pub enum MirTerm {
    /// Variable reference
    Var(String),
    /// Literal constant
    Const(MirConst),
    /// Don't-care / wildcard (anonymous variable)
    Wildcard,
}

/// Constant values.
pub enum MirConst {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
}
```

### 1.6 Arithmetic Expressions

```rust
pub struct MirArithExpr {
    pub left: MirTerm,
    pub op: MirArithOp,
    pub right: MirTerm,
}

pub enum MirArithOp {
    Add, Sub, Mul, Div, Mod,
}
```

### 1.7 Comparison Operators

```rust
pub enum MirCompOp {
    Eq, Ne, Lt, Le, Gt, Ge,
}
```

### 1.8 Aggregate Functions

```rust
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
```

### 1.9 Types

```rust
/// MIR-level type representation (simplified from HIR).
pub enum MirType {
    /// Primitive types
    Int,
    Float,
    String,
    Boolean,
    Date,
    /// Database entity type (from .dbscheme)
    Entity(String),
    /// User-defined class type — resolved to its characteristic predicate
    Class(String),
    /// Any type (for unresolved or polymorphic cases)
    Any,
}
```

### 1.10 Annotations

```rust
/// Annotations that affect MIR compilation and optimization.
pub struct MirAnnotations {
    pub cached: bool,
    pub nomagic: bool,
    pub noinline: bool,
    pub inline: bool,
    pub inline_late: bool,
    pub binding_set: Vec<Vec<String>>,  // @bindingset constraints
}
```

## 2. S-Expression Concrete Syntax

MIR has a textual S-expression format for debugging, testing, and hard-coded MIR files.

### Grammar

```
program     ::= predicate*
predicate   ::= '(' 'predicate' NAME params body ')'
              | '(' 'predicate' NAME params ')'             // abstract, no body
params      ::= '(' 'params' param* ')'
param       ::= '(' NAME ':' type ')'
body        ::= conjunction | disjunction
conjunction ::= '(' 'and' atom* ')'
disjunction ::= '(' 'or' conjunction* ')'
atom        ::= scan | negscan | guard | assign | aggregate | typecheck
scan        ::= '(' 'scan' NAME term* ')'
negscan     ::= '(' 'not' NAME term* ')'
guard       ::= '(' comp_op term term ')'
assign      ::= '(' '=' NAME arith_expr ')'
aggregate   ::= '(' agg_fn NAME NAME group_by NAME ')'     // result sub-pred group-by agg-var
typecheck   ::= '(' 'instanceof' NAME NAME ')'             // var type-pred
term        ::= NAME                                        // variable
              | INT                                         // integer constant
              | FLOAT                                       // float constant
              | STRING                                      // string constant
              | 'true' | 'false'                            // boolean
              | '_'                                         // wildcard
arith_expr  ::= '(' arith_op term term ')'
comp_op     ::= '=' | '!=' | '<' | '<=' | '>' | '>='
arith_op    ::= '+' | '-' | '*' | '/' | '%'
agg_fn      ::= 'count' | 'sum' | 'min' | 'max' | 'avg' | 'any'
              | 'rank' | 'concat' | 'strictcount' | 'strictsum'
type        ::= 'int' | 'float' | 'string' | 'boolean' | 'date'
              | '@' NAME                                    // entity type
              | NAME                                        // class type
              | 'any'
NAME        ::= [a-zA-Z_#][a-zA-Z0-9_#.]*
```

### Examples

**Simple predicate:**
```lisp
(predicate isSmall
  (params (x : int))
  (and
    (scan vals x)
    (< x 10)))
```

**Class with characteristic predicate and member:**
```lisp
(predicate SmallInt#char
  (params (this : int))
  (and
    (>= this 1)
    (<= this 9)))

(predicate SmallInt#double
  (params (this : int) (result : int))
  (and
    (scan SmallInt#char this)
    (= result (* this 2))))
```

**Aggregation:**
```lisp
(predicate countSmall
  (params (result : int))
  (and
    (count result _agg_countSmall_0 () _agg_var)))
```

**Disjunction (override dispatch):**
```lisp
(predicate Element#toString
  (params (this : @element) (result : string))
  (or
    (and (scan Function#char this) (scan Function#toString this result))
    (and (scan Variable#char this) (scan Variable#toString this result))
    (and (not Function#char this) (not Variable#char this)
         (scan Element#toString_base this result))))
```

## 3. Lowering Steps: HIR → MIR

The lowering proceeds in well-defined stages. Each stage handles one category of
QL constructs and produces MIR predicates.

### Stage 1: Module Flattening

**Input:** HIR module tree with nested modules, imports, re-exports
**Output:** Flat namespace of fully-qualified predicate names

Transformations:
- Nested modules `M::N::pred` → predicate name `M.N.pred`
- Import aliases resolved to target names
- Type aliases resolved to target types
- Module parameters instantiated (parameterized modules)
- Private/public visibility tracked for dead-code elimination

### Stage 2: Class Elimination

**Input:** Class declarations with char predicates, members, fields, supertypes
**Output:** Flat predicates with `#char`, `#member` naming convention

Transformations:
- `class C extends S { C() { body } }` → `C#char(this) :- S#char(this), body`
- `class C { T member() { body } }` → `C#member(this, result) :- C#char(this), body`
- `class C { T field; }` → `C#field(this, field) :- C#char(this), ...`
- `class C extends int { C() { this in [1..9] } }` → primitive type constraint
- Abstract predicates → no body, must be implemented by subclasses
- `instanceof` → scan on characteristic predicate

### Stage 3: Override Dispatch

**Input:** Class hierarchies with overridden predicates
**Output:** Dispatch predicates that select the most-specific implementation

For a predicate `p` defined in class `Base` and overridden in `Sub1`, `Sub2`:
```
Base#p(this, result) :-
  (Sub1#char(this), Sub1#p(this, result)) OR
  (Sub2#char(this), Sub2#p(this, result)) OR
  (NOT Sub1#char(this), NOT Sub2#char(this), Base#p_impl(this, result))
```

### Stage 4: Formula Lowering

Each QL formula kind maps to MIR atoms:

| QL Formula | MIR Atoms |
|---|---|
| `A and B` | Flatten into single conjunction |
| `A or B` | Create disjunction or auxiliary predicate |
| `not F` | NegScan (for simple calls) or auxiliary + negate |
| `x = y`, `x < y` | Guard |
| `exists(T x \| F)` | Auxiliary predicate with x existentially quantified |
| `forall(T x \| G \| B)` | `not exists(T x \| G \| not B)` |
| `forex(T x \| G \| B)` | `exists(T x \| G)` and `forall(T x \| G \| B)` |
| `if C then T else E` | `(C and T) or (not C and E)` |
| `A implies B` | `not A or B` |
| `x instanceof T` | TypeCheck(x, T#char) |
| `none()` | Empty disjunction (no rules) |
| `any()` | True (no constraint) |

### Stage 5: Expression Lowering

Expressions lower to terms or generate auxiliary atoms:

| QL Expression | MIR Output |
|---|---|
| Literal `42` | MirTerm::Const(Int(42)) |
| Variable `x` | MirTerm::Var("x") |
| `this` | MirTerm::Var("this") |
| `result` | MirTerm::Var("result") |
| `_` | MirTerm::Wildcard |
| `x + y` | Assign { result_var: fresh, expr: Add(x, y) } |
| `p(x)` (no result) | Scan("p", [x]) — formula context |
| `p(x)` (with result) | Scan("p", [x, fresh_var]) + bind fresh_var |
| `x.m(args)` | Scan("T#m", [x, args..., result_var]) |
| `x.m+(args)` | Generate recursive closure predicate |
| `(Type)x` or `x.(Type)` | TypeCheck + scan characteristic predicate |
| `count(...)` | Aggregate { function: Count, ... } |
| `[lo..hi]` | Guard(>= lo), Guard(<= hi) |
| `{a, b, c}` | Disjunction of equalities |

### Stage 6: Closure Lowering

Transitive closure `p+()` and reflexive-transitive closure `p*()`:

```
// p+(x, result) — transitive closure
_closure_p_0(x, result) :- x.p(result).        // base
_closure_p_0(x, result) :- _closure_p_0(x, z), z.p(result).  // step

// p*(x, result) — reflexive-transitive closure
_closure_p_1(x, result) :- x = result.          // reflexive base
_closure_p_1(x, result) :- _closure_p_0(x, result).  // transitive part
```

### Stage 7: Aggregate Lowering

Aggregations create auxiliary sub-predicates:

```ql
count(int x | x = [1..10] and x % 2 = 0)
```
→
```
_agg_body_0(x) :- x >= 1, x <= 10, x % 2 = 0.
result = count(_agg_body_0, [], x)
```

For grouped aggregation:
```ql
int countArgs(Function f) { result = count(Parameter p | p = f.getAParam()) }
```
→
```
_agg_body_1(f, p) :- Function#char(f), Function#getAParam(f, p).
countArgs(f, result) :- Function#char(f), result = count(_agg_body_1, [f], p).
```

### Stage 8: Newtype Lowering

Newtype branches become characteristic predicates:

```ql
newtype TMyNode = TExprNode(Expr e) or TStmtNode(Stmt s)
```
→
```
TExprNode#char(this, e) :- Expr#char(e), this = new_entity(e).
TStmtNode#char(this, s) :- Stmt#char(s), this = new_entity(s).
TMyNode#char(this) :- TExprNode#char(this, _) OR TStmtNode#char(this, _).
```

## 4. Analysis Stages within MIR

After lowering, MIR undergoes several analysis/optimization passes:

### 4.1 Binding Analysis

Verify that every variable in a rule head is bound by some body atom.
Flag unbound variables as errors. Determine binding order for optimal evaluation.

### 4.2 Stratification

Compute evaluation strata using Tarjan SCC on predicate dependency graph:
- Positive edges: pred A depends positively on pred B
- Negative edges: pred A depends negatively on pred B (via NegScan)
- Aggregate edges: pred A depends on pred B via aggregation (treated like negation)
- Reject programs with negative cycles

### 4.3 Type Propagation

Propagate types through MIR predicates to:
- Verify type consistency at call sites
- Enable type-based optimizations (e.g., entity lookups)
- Generate schema for intermediate relations

## 5. Optimization Steps within MIR

### 5.1 Constant Folding

Evaluate compile-time-known arithmetic: `3 + 4` → `7`

### 5.2 Dead Predicate Elimination

Remove predicates not reachable from any query/select predicate.

### 5.3 Predicate Inlining

For small, non-recursive predicates called once:
- Inline the body at the call site
- Removes intermediate predicate overhead
- Respects `@pragma[noinline]` annotations

### 5.4 Guard Simplification

- `x = x` → remove (always true)
- `x = 5, x = 5` → deduplicate
- `x = 5, x = 6` → detect contradiction, remove rule

### 5.5 Join Ordering Hint Generation

Analyze which atoms have the most selective filters and reorder body atoms
to minimize intermediate result sizes. (Hint only — final ordering is engine's choice.)

## 6. Lowering from MIR to Engine Rules

The final step converts MirProgram to ocql_engine::rule::Program:

```
MirPredicate → one or more Rule (one per disjunct)
MirAtom::Scan → BodyElement::Positive(Atom)
MirAtom::NegScan → BodyElement::Negated(Atom)
MirAtom::Guard → BodyElement::Guard(Guard)
MirAtom::Assign → BodyElement::Assign { result_var, expr }
MirAtom::Aggregate → BodyElement::Aggregate { ... }
MirAtom::TypeCheck → BodyElement::Positive(Atom) on char predicate
MirTerm::Var → Term::Var
MirTerm::Const → Term::Const or Term::StrLit
MirTerm::Wildcard → Term::Var("_anonN")
```

## 7. Implementation Phases

### Phase 1: Core Definitions + S-Expr Format
- MIR node types (Rust structs/enums)
- S-expression printer
- S-expression parser (for hard-coded MIR files)
- Round-trip tests

### Phase 2: Simplest HIR→MIR Lowering
- Predicates without bodies (facts)
- Predicates with comparisons and arithmetic
- Variable binding and `result` handling
- Lowering to engine rules + evaluation tests

### Phase 3: Formula Lowering
- Conjunction, disjunction
- Negation
- Comparison operators
- `exists`, `forall`, `implies`, `if-then-else`
- `instanceof`

### Phase 4: Class and Type Lowering
- Characteristic predicates
- Member predicates with `this`
- Supertype chaining
- Override dispatch

### Phase 5: Expression Lowering
- Predicate calls with results
- Member calls
- Qualified calls
- Arithmetic expressions
- Cast expressions
- Range and set literal expressions

### Phase 6: Aggregation Lowering
- count, sum, min, max
- Grouped aggregation
- strictcount, strictsum
- any, rank, concat

### Phase 7: Advanced Features
- Closure (transitive/reflexive-transitive)
- Newtype lowering
- Module flattening with qualified names

### Phase 8: Optimization Passes
- Constant folding
- Dead predicate elimination
- Predicate inlining
- Guard simplification

### Phase 9: Integration Testing
- Parse + HIR + MIR for all .qll/.ql files in vendor/codeql
- Measure lowering success rate per language pack
- End-to-end query evaluation on extracted databases
