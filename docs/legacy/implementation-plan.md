# Implementation Plan: Database Layer & Engine

This document describes the concrete implementation plan for the open-cql database
layer, evaluation engine, and their integration with the QL compiler. It is designed
to be read by implementors working on individual crates.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                        QL Source (.ql/.qll)                         │
│                               │                                     │
│                    ┌──────────▼──────────┐                          │
│  TRACK B           │  ocql-ql-parser     │  (exists, ~98% parse)   │
│  (QL Compiler)     │  ocql-ql-ast        │  (exists)               │
│                    └──────────┬──────────┘                          │
│                               │                                     │
│                    ┌──────────▼──────────┐                          │
│                    │  ocql-hir           │  (name res, type check)  │
│                    └──────────┬──────────┘                          │
│                               │                                     │
│                    ┌──────────▼──────────┐                          │
│                    │  ocql-mir           │  (class elim, Datalog)   │
│                    └──────────┬──────────┘                          │
│                               │  emits rules                       │
└───────────────────────────────┼─────────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────────┐
│  TRACK A                                                            │
│  (Database + Engine)                                                │
│                                                                     │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │ ocql-schema   │  │ ocql-database│  │ ocql-engine              │  │
│  │ (.dbscheme    │  │ (values,     │  │ (relational ops,         │  │
│  │  parser)      │  │  relations,  │  │  semi-naive evaluator,   │  │
│  │              │──▶│  indexes,    │──▶│  stratification,         │  │
│  │              │  │  storage)    │  │  aggregates)             │  │
│  └──────────────┘  └──────────────┘  └──────────────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Two Parallel Tracks

**Track A** and **Track B** can be developed largely independently, meeting at a
well-defined interface: Track B (the QL compiler) emits Datalog-like rules that
Track A (the engine) evaluates against a database.

### Track A: Database + Engine (this document's focus)

Crates: `ocql-schema`, `ocql-database`, `ocql-engine`

No dependency on the QL parser/AST. Can be tested with hand-written rules and
hand-crafted databases.

### Track B: QL Compiler (HIR + MIR)

Crates: `ocql-hir`, `ocql-mir` (and later `ocql-lir`)

Depends on `ocql-ql-parser` and `ocql-ql-ast` (which exist). Depends on
`ocql-schema` for database type information (the `@`-prefixed types). Does NOT
depend on `ocql-database` or `ocql-engine`.

### Interface Between Tracks

Track B's MIR output is a set of flat Datalog rules:
```
predicate_name(param1, param2, ...) :-
    atom1(x, y), atom2(y, z), guard(x > 0), ...
```

Track A's engine consumes exactly this. The shared representation is defined in
a small `ocql-ir` or placed in `ocql-common`.

---

## Track A: Detailed Plan

### Crate 1: `ocql-schema` — .dbscheme Parser

**Purpose**: Parse CodeQL `.dbscheme` files into a Rust representation of the
relational schema.

**No dependencies** on other ocql crates (except `ocql-common` for shared types).

#### .dbscheme Grammar

The format has exactly 5 constructs:

```
// 1. Table definition
[#keyset[col1, col2]]
table_name(
    [unique] <type> <name> : <col_type> [ref],
    ...
);

// 2. Union type definition (entity hierarchy)
@parent_type = @child1 | @child2 | @child3;

// 3. Case/enum block (discriminated union via integer column)
case @table.column of
    1 = @variant1
|   2 = @variant2
|   ...
;

// 4. Column types
//    int, float, string, boolean, date
//    @entity_type (reference to entity)
//    "ref" suffix means foreign key

// 5. Comments: /* ... */ and /** ... */ (QLDoc)
```

#### Output Data Structures

```rust
/// A parsed .dbscheme file
pub struct DbScheme {
    /// Table definitions (e.g., "functions", "exprs", "stmts")
    pub tables: Vec<Table>,
    /// Entity union types (e.g., @type = @builtintype | @derivedtype | ...)
    pub unions: Vec<UnionType>,
    /// Case/enum blocks (e.g., case @function.kind of 1 = @constructor | ...)
    pub cases: Vec<CaseType>,
}

pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
    pub keysets: Vec<Vec<String>>,  // composite key constraints
}

pub struct Column {
    pub name: String,
    pub col_type: ColumnType,
    pub is_unique: bool,
    pub is_ref: bool,  // foreign key
}

pub enum ColumnType {
    Int,
    Float,
    String,
    Boolean,
    Date,
    Entity(String),  // e.g., "@function", "@type"
}

pub struct UnionType {
    pub name: String,           // e.g., "@type"
    pub variants: Vec<String>,  // e.g., ["@builtintype", "@derivedtype", ...]
}

pub struct CaseType {
    pub entity: String,         // e.g., "@function"
    pub column: String,         // e.g., "kind"
    pub variants: Vec<(i64, String)>,  // e.g., [(1, "@constructor"), ...]
}
```

#### Implementation Notes

- Use a simple hand-written parser (recursive descent or `nom`/`winnow`).
  The grammar is trivial — no ambiguity, no precedence issues.
- Skip QLDoc comments but preserve table-level doc comments in the output
  (useful for debugging).
- Test against the actual C++ schema (2,545 lines, 235 tables) and Java
  schema (1,241 lines). Both are in `vendor/codeql/`.

#### Test Files

- `vendor/codeql/cpp/ql/lib/semmlecode.cpp.dbscheme` (C++, 235 tables)
- `vendor/codeql/java/ql/lib/config/semmlecode.dbscheme` (Java)

#### Acceptance Criteria

- Successfully parses both C++ and Java .dbscheme files
- Produces a `DbScheme` struct with all tables, unions, and cases
- Round-trip test: parse → pretty-print → parse again → equal

---

### Crate 2: `ocql-database` — Value Types, Relations, Storage

**Purpose**: Core data representation — values, tuples, relations with indexes,
and the database container.

**Depends on**: `ocql-schema` (for schema-aware database construction)

#### Value Representation

```rust
/// A single value in the database.
/// Designed to be compact (single enum, no heap allocation for common cases).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Value {
    Int(i64),
    Float(OrderedFloat<f64>),     // use `ordered-float` crate
    String(InternedString),       // index into string interner
    Bool(bool),
    Entity(EntityId),             // u64 entity reference
    Null,
}

/// Interned string handle (u32 index into string table)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InternedString(u32);

/// Entity ID (used for @-prefixed database types)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityId(pub u64);
```

#### String Interner

```rust
/// Global string table for the database. All strings are interned.
pub struct StringInterner {
    strings: Vec<String>,            // indexed by InternedString
    lookup: HashMap<String, InternedString>,
}
```

#### Tuple and Relation

```rust
/// A tuple is a fixed-size row of values.
/// Use SmallVec to avoid heap allocation for small tuples (most are 2-6 columns).
pub type Tuple = SmallVec<[Value; 6]>;

/// A relation is a named set of tuples with a schema.
pub struct Relation {
    pub schema: RelationSchema,
    /// Primary storage: sorted set of tuples for dedup and merge joins.
    tuples: BTreeSet<Tuple>,
    /// Secondary indexes: column(s) → matching tuples.
    /// Created on demand based on query patterns.
    indexes: Vec<Index>,
}

pub struct RelationSchema {
    pub name: String,
    pub columns: Vec<ColumnDef>,
}

pub struct ColumnDef {
    pub name: String,
    pub col_type: ColumnType,
}

/// An index on one or more columns.
pub struct Index {
    pub columns: Vec<usize>,  // which columns are indexed
    pub data: BTreeMap<Vec<Value>, Vec<usize>>,  // key → tuple indices
}
```

#### Database Container

```rust
/// A database is a collection of named relations plus metadata.
pub struct Database {
    /// The schema this database conforms to
    pub schema: DbScheme,
    /// Relations keyed by table name
    relations: HashMap<String, Relation>,
    /// Global string interner (shared across all relations)
    pub strings: StringInterner,
    /// Entity type information (which entity IDs belong to which @types)
    entity_types: HashMap<EntityId, String>,
}

impl Database {
    /// Create an empty database from a schema
    pub fn from_schema(schema: DbScheme) -> Self;

    /// Get a relation by name
    pub fn relation(&self, name: &str) -> Option<&Relation>;

    /// Get a mutable relation by name
    pub fn relation_mut(&mut self, name: &str) -> Option<&mut Relation>;

    /// Insert a tuple into a relation
    pub fn insert(&mut self, table: &str, tuple: Tuple) -> Result<()>;

    /// Iterate all tuples in a relation
    pub fn scan(&self, table: &str) -> impl Iterator<Item = &Tuple>;
}
```

#### Acceptance Criteria

- Can create a Database from a parsed DbScheme
- Can insert tuples and scan relations
- Indexes created on demand, correct lookup results
- String interning works correctly (same string → same InternedString)
- Basic serialization to/from disk (can be simple JSON/bincode initially)

---

### Crate 3: `ocql-engine` — Relational Operators & Datalog Evaluator

**Purpose**: Execute relational algebra operations and evaluate Datalog rules
to a fixed point.

**Depends on**: `ocql-database`

This crate has two layers:

#### Layer 1: Relational Operators

Stateless functions that take relation(s) and produce a new relation.

```rust
/// Filter rows matching a predicate
pub fn filter(rel: &Relation, pred: &dyn Fn(&Tuple) -> bool) -> Relation;

/// Project to selected columns
pub fn project(rel: &Relation, columns: &[usize]) -> Relation;

/// Equi-join two relations on specified column pairs
pub fn join(
    left: &Relation,
    right: &Relation,
    left_cols: &[usize],
    right_cols: &[usize],
) -> Relation;

/// Set union (deduplicating)
pub fn union(a: &Relation, b: &Relation) -> Relation;

/// Set difference: tuples in `a` not in `b`
pub fn difference(a: &Relation, b: &Relation) -> Relation;

/// Aggregate: group by some columns, apply aggregate function
pub fn aggregate(
    rel: &Relation,
    group_by: &[usize],
    agg_col: usize,
    agg_fn: AggregateFunction,
) -> Relation;

pub enum AggregateFunction {
    Count,
    Sum,
    Min,
    Max,
    Avg,
    Concat { separator: String },
}
```

Start with hash join. Add merge join later as an optimization.

#### Layer 2: Datalog Evaluator

The rule representation and fixpoint evaluator.

```rust
/// A Datalog rule: head(...) :- body_atom1, body_atom2, ...guard...
pub struct Rule {
    pub head: Atom,
    pub body: Vec<BodyElement>,
}

/// A positive or negative atom: predicate_name(term1, term2, ...)
pub struct Atom {
    pub predicate: String,
    pub terms: Vec<Term>,
}

pub enum Term {
    Var(String),
    Const(Value),
}

pub enum BodyElement {
    /// Positive atom: p(x, y)
    Positive(Atom),
    /// Negated atom: not p(x, y)
    Negated(Atom),
    /// Guard/filter: x > 0, x = y + 1, etc.
    Guard(Guard),
    /// Aggregate: result = count(body_pred, group_by_vars)
    Aggregate {
        result_var: String,
        function: AggregateFunction,
        body_pred: String,
        group_by: Vec<String>,
        agg_var: String,
    },
}

pub struct Guard {
    pub left: Term,
    pub op: CompOp,
    pub right: Term,
}

pub enum CompOp { Eq, Ne, Lt, Le, Gt, Ge }
```

##### Semi-Naive Evaluation Algorithm

```rust
/// Evaluate a set of Datalog rules against a database to fixpoint.
pub fn evaluate(rules: &[Rule], db: &mut Database) -> Result<()> {
    // 1. Compute stratification
    let strata = stratify(rules)?;

    // 2. Evaluate each stratum bottom-up
    for stratum in &strata {
        if stratum.is_recursive {
            evaluate_recursive(stratum, db)?;
        } else {
            evaluate_nonrecursive(stratum, db)?;
        }
    }
    Ok(())
}

/// Semi-naive fixpoint for a recursive stratum
fn evaluate_recursive(stratum: &Stratum, db: &mut Database) -> Result<()> {
    // Initialize: evaluate non-recursive parts of each rule
    // to get initial delta relations
    let mut deltas: HashMap<String, Relation> = initial_deltas(stratum, db);

    loop {
        let mut new_deltas: HashMap<String, Relation> = HashMap::new();

        for rule in &stratum.rules {
            // Evaluate rule body, using delta for at least one
            // recursive predicate (semi-naive optimization)
            let new_tuples = evaluate_rule_with_delta(rule, db, &deltas);

            // Filter out already-known tuples
            let truly_new = difference(&new_tuples, db.relation(&rule.head.predicate));

            new_deltas.entry(rule.head.predicate.clone())
                .or_default()
                .extend(truly_new);
        }

        // Check termination
        if new_deltas.values().all(|r| r.is_empty()) {
            break;  // Fixed point reached
        }

        // Merge deltas into database
        for (pred, delta) in &new_deltas {
            db.relation_mut(pred).extend(delta);
        }
        deltas = new_deltas;
    }
    Ok(())
}
```

##### Stratification

```rust
/// Compute negation stratification.
/// Returns error if the program is not stratifiable
/// (negation cycle = non-monotonic recursion).
pub fn stratify(rules: &[Rule]) -> Result<Vec<Stratum>> {
    // 1. Build predicate dependency graph
    //    - Positive edge: head depends on body predicate
    //    - Negative edge: head depends negatively on negated body predicate
    // 2. Find SCCs (strongly connected components)
    // 3. Check: no negative edge within an SCC (else non-stratifiable)
    // 4. Topological sort of SCCs = strata
}

pub struct Stratum {
    pub rules: Vec<Rule>,
    pub predicates: HashSet<String>,
    pub is_recursive: bool,  // true if the SCC has >1 node or self-loop
}
```

#### Acceptance Criteria

- Relational operators produce correct results (test with small hand-crafted relations)
- Can evaluate non-recursive rules (single pass)
- Can evaluate recursive rules to fixpoint (e.g., transitive closure)
- Stratification correctly handles negation
- Aggregates work for count, min, max, sum
- Error on non-stratifiable programs (negation cycles)

#### End-to-End Test

A complete test that exercises the full Track A pipeline:

```
1. Parse vendor/codeql/cpp/ql/lib/semmlecode.cpp.dbscheme
2. Create an empty Database from the schema
3. Insert hand-crafted tuples (a tiny C program's facts)
4. Define Datalog rules (e.g., "find all functions that call function X")
5. Evaluate rules
6. Check results
```

---

## Track B: QL Compiler (Summary)

This track is for a separate implementation agent. Brief overview:

### Crate: `ocql-hir` — High-Level IR

**Depends on**: `ocql-ql-ast`, `ocql-schema` (for database type info)

Key tasks:
1. **Name resolution** — resolve identifiers across 6 namespaces per module
2. **Import resolution** — follow import paths, handle qualified references
3. **Type checking** — verify predicate arities, class hierarchy constraints
4. **Class linearization** — MRO for multiple inheritance
5. **Desugaring** — `implies` → disjunction, `forex` → forall+exists, closures → recursion

### Crate: `ocql-mir` — Mid-Level IR

**Depends on**: `ocql-hir`

Key tasks:
1. **Class elimination** — classes → characteristic predicates + dispatch tables
2. **Module elimination** — fully qualify all names
3. **Aggregate lowering** — aggregations → auxiliary predicates + group-by
4. **Negation stratification** — assign strata to predicates

**Output**: Flat Datalog rules that `ocql-engine` can evaluate.

---

## Dependency Graph (Crate Level)

```
ocql-common          (shared types, Value, Span — already exists)
  ├── ocql-schema        [Track A] — .dbscheme parser
  ├── ocql-ql-ast        [Track B] — AST types (exists)
  └── ocql-ql-parser     [Track B] — QL parser (exists)

ocql-schema
  └── ocql-database      [Track A] — relations, indexes, storage

ocql-database
  └── ocql-engine        [Track A] — relational ops, Datalog evaluator

ocql-ql-ast + ocql-schema
  └── ocql-hir           [Track B] — name resolution, type checking

ocql-hir
  └── ocql-mir           [Track B] — class elimination → Datalog rules

ocql-mir + ocql-engine
  └── ocql-driver        [Later]   — end-to-end: QL source → results
```

### What Can Be Parallel

```
Track A agent works on:          Track B agent works on:
  ocql-schema                      ocql-hir (name resolution)
  ocql-database                    ocql-hir (type checking)
  ocql-engine (relational ops)     ocql-mir (class elimination)
  ocql-engine (Datalog eval)       ocql-mir (aggregate lowering)
```

The only shared dependency is `ocql-schema`, which Track B needs for
database type resolution in HIR. Track A should build `ocql-schema` first
(it's the smallest crate), and Track B can start on HIR once the schema
types are available.

Within Track A, the crates are sequential: schema → database → engine.
Within Track B, the crates are sequential: HIR → MIR.
But the two tracks are independent of each other.

---

## Suggested Workspace Cargo.toml Update

Add these new crates:

```toml
[workspace]
resolver = "2"
members = [
    "crates/ocql-common",
    "crates/ocql-ql-ast",
    "crates/ocql-ql-parser",
    "crates/ocql-eval",        # existing parse-rate evaluator
    "crates/ocql-schema",      # NEW: .dbscheme parser
    "crates/ocql-database",    # NEW: value types, relations, storage
    "crates/ocql-engine",      # NEW: relational ops + Datalog evaluator
]
```

---

## Key Design Decisions

1. **Parse CodeQL's .dbscheme format directly** — gives us schema compatibility
   with CodeQL's QL libraries. Our extractors will produce data conforming to
   the same schema.

2. **Build our own storage format** — we do NOT read CodeQL's proprietary
   binary database format. Our extractors will populate our own format.

3. **Build our own Datalog engine** — existing Rust engines (Crepe, Ascent,
   Datafrog) don't support dynamic rule loading. Our engine evaluates rules
   produced by the QL compiler at runtime.

4. **Path tracking in QL, not the engine** — following CodeQL's architecture,
   data flow path tracking is implemented as QL library predicates
   (`PathNode`, `flowPath`), not as engine-level provenance. The engine just
   evaluates Datalog rules; the path graph emerges from the QL library's
   recursive predicates.

5. **Correctness first, performance later** — start with hash joins and
   BTreeSet storage. Optimize with leapfrog triejoin, better indexing, and
   parallelism later.

---

## Testing Strategy

### Unit Tests Per Crate

- **ocql-schema**: Parse both C++ and Java .dbscheme files from vendor/codeql.
  Snapshot test the parsed output.
- **ocql-database**: Insert/query/index operations on small hand-crafted relations.
- **ocql-engine**: Evaluate known Datalog programs (transitive closure, same
  generation, path queries) against small databases. Compare results with
  expected output.

### Integration Tests

- Parse a .dbscheme → create database → insert facts → evaluate rules → check results
- Start with simple "find functions by name" style queries
- Progress to recursive queries (reachability, transitive closure)
- Eventually: aggregation queries (count, min/max)

### Compatibility Tests

- Verify that our .dbscheme parser agrees with CodeQL's schema on all
  table names, column types, and entity hierarchies
- When extractors are built later, verify our database output matches
  the schema constraints
