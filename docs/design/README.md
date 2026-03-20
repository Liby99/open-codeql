# open-cql: Project Design Document

## 1. Project Overview

open-cql is an open-source re-implementation of the CodeQL static analysis engine,
built in Rust. It implements the QL query language and its evaluation engine from
scratch based on publicly available documentation.

### Goals
- Implement the QL language (parser, type checker, evaluator)
- Build language extractors for C/C++ and Java
- Implement a novel Datalog+flow engine that natively supports exact flow tracking
- Produce a multi-stage IR compilation pipeline with well-defined intermediate representations
- Achieve correctness first, then performance

### Non-Goals (for now)
- Full parity with CodeQL's standard libraries
- Support for all CodeQL-supported languages
- IDE integration (VS Code extension)
- Cloud/multi-repo analysis

## 2. Architecture Overview

```
                          ┌─────────────────────────┐
                          │      QL Source (.ql)      │
                          └────────────┬──────────────┘
                                       │
                                       ▼
                          ┌─────────────────────────┐
                          │     QL Parser (AST)      │
                          └────────────┬──────────────┘
                                       │
                                       ▼
                          ┌─────────────────────────┐
                          │   HIR (High-level IR)    │
                          │  - Name resolution       │
                          │  - Type checking         │
                          │  - Class linearization   │
                          │  - Desugaring            │
                          └────────────┬──────────────┘
                                       │
                                       ▼
                          ┌─────────────────────────┐
                          │    MIR (Mid-level IR)    │
                          │  - Predicate inlining    │
                          │  - Specialization        │
                          │  - Aggregate lowering    │
                          │  - Class hierarchy flat. │
                          └────────────┬──────────────┘
                                       │
                                       ▼
                          ┌─────────────────────────┐
                          │    LIR (Low-level IR)    │
                          │  - Relational algebra    │
                          │  - Joins, projections    │
                          │  - Unions, fixpoints     │
                          └────────────┬──────────────┘
                                       │
                                       ▼
                          ┌─────────────────────────┐
                          │      Query Plan          │
                          │  - Join ordering         │
                          │  - Index selection       │
                          │  - Magic set transforms  │
                          └────────────┬──────────────┘
                                       │
                                       ▼
  ┌──────────────┐        ┌─────────────────────────┐
  │  open-cql DB │───────▶│   Execution Engine      │
  │  (relations) │        │  - Semi-naive eval      │
  └──────────────┘        │  - Flow-tracking ext.   │
                          │  - Provenance tracking   │
                          └────────────┬──────────────┘
                                       │
                                       ▼
                          ┌─────────────────────────┐
                          │    Query Results         │
                          │  - Alerts / Paths        │
                          │  - SARIF output          │
                          └─────────────────────────┘
```

## 3. Crate Structure (Rust Workspace)

```
open-cql/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── ocql-cli/                 # CLI binary
│   ├── ocql-ql-parser/           # QL language parser (AST)
│   ├── ocql-ql-ast/              # QL AST data structures
│   ├── ocql-hir/                 # High-level IR
│   ├── ocql-mir/                 # Mid-level IR
│   ├── ocql-lir/                 # Low-level IR (relational algebra)
│   ├── ocql-planner/             # Query plan optimizer
│   ├── ocql-engine/              # Evaluation engine (Datalog+flow)
│   ├── ocql-database/            # Database storage and schema
│   ├── ocql-schema/              # .dbscheme parser and representation
│   ├── ocql-extractor-cpp/       # C/C++ extractor (tree-sitter)
│   ├── ocql-extractor-java/      # Java extractor (tree-sitter)
│   ├── ocql-common/              # Shared types, error handling
│   └── ocql-results/             # Result formatting (SARIF, etc.)
├── docs/
│   ├── design/                   # This design document + sub-docs
│   └── crawled/                  # Crawled CodeQL documentation
├── scripts/                      # Build/crawl scripts
└── tests/                        # Integration tests
```

## 4. Component Design

### 4.1 QL Parser (`ocql-ql-parser`, `ocql-ql-ast`)

**Input:** `.ql` and `.qll` source files
**Output:** Untyped AST

The QL language grammar (from the specification):
- Programs consist of query modules (.ql) and library modules (.qll)
- Key constructs: predicates, classes, modules, queries, imports
- Expressions: literals, variables, calls, aggregations, casts, ranges
- Formulas: comparisons, quantifiers, logical connectives
- Annotations on declarations

**Parser strategy:** Hand-written recursive descent parser (not a parser generator).
Rationale: better error messages, easier to maintain, proven approach for similar tools
(rustc, TypeScript compiler).

**AST nodes include:**
```
Module { name, imports, members }
Predicate { annotations, result_type, name, params, body }
Class { annotations, name, supertypes, characteristic_pred, members }
Select { from_vars, where_formula, select_exprs, order_by }
Formula { kind: And | Or | Not | Exists | Forall | ... }
Expr { kind: Literal | Variable | Call | Aggregation | Cast | ... }
Type { kind: Primitive | Database | ClassName | ... }
```

### 4.2 HIR (High-level IR) (`ocql-hir`)

**Input:** Untyped AST
**Output:** Typed, resolved IR

Transformations performed:
1. **Name resolution** — Resolve all identifiers to their declarations.
   Handle the 6 namespaces per module (module, type, predicate, module-sig, type-sig, pred-sig).
2. **Import resolution** — Follow import paths, handle qualified references, private imports.
3. **Type checking** — Verify type compatibility, check predicate arities, validate class hierarchies.
4. **Class linearization** — Resolve multiple inheritance into a deterministic order for method dispatch.
5. **Desugaring** — Lower syntactic sugar:
   - `if-then-else` formulas -> `(cond and then) or (not cond and else)`
   - `implies` -> `not A or B`
   - `forex` -> `forall and exists`
   - Closures `+` and `*` -> explicit recursive predicates

### 4.3 MIR (Mid-level IR) (`ocql-mir`)

**Input:** Typed HIR
**Output:** Flattened predicate-centric IR

This is analogous to CodeQL's DIL (Datalog Intermediary Language).

Transformations:
1. **Class elimination** — Convert class hierarchies into:
   - Characteristic predicates (membership tests)
   - Flattened member predicates with explicit `this` parameter
   - Dispatch tables for overridden predicates
2. **Module elimination** — Fully qualify all names, inline module structure
3. **Predicate specialization** — Instantiate parameterized modules/predicates
4. **Aggregate lowering** — Convert aggregations into grouped fixpoint computations:
   - `count(...)` -> group-by + count
   - `min/max(...)` -> group-by + extremum
   - `rank[n](...)` -> group-by + sort + index
5. **Negation stratification** — Compute dependency graph, identify strata for
   evaluation ordering. Reject non-stratifiable programs.

**MIR representation:**
```
Predicate {
    name: QualifiedName,
    params: Vec<(Name, Type)>,
    result: Option<Type>,
    body: MirFormula,
    stratum: usize,
}

MirFormula = Join | Union | Project | Select | Negate | Fixpoint | ...
```

### 4.4 LIR (Low-level IR) (`ocql-lir`)

**Input:** MIR
**Output:** Relational algebra expressions

The LIR is a pure relational algebra with extensions for recursion and flow tracking.

**Core operators:**
```
Relation     — Base table reference
Join         — Natural join (equi-join on shared columns)
Project      — Column projection
Select       — Filter by predicate
Union        — Set union
Difference   — Set difference (for negation)
Rename       — Column renaming
Fixpoint     — Recursive computation (semi-naive)
FlowJoin     — Flow-aware join (tracks provenance)
Aggregate    — Group-by with aggregate function
```

**Key extension: FlowJoin**

Unlike standard Datalog joins, a FlowJoin tracks the exact derivation path.
When computing data flow from source to sink, a FlowJoin produces not just
(source, sink) pairs but (source, sink, path) triples where path encodes
the exact sequence of intermediate nodes.

### 4.5 Query Planner (`ocql-planner`)

**Input:** LIR
**Output:** Optimized execution plan

Optimizations (in order):
1. **Predicate pushdown** — Push selections as close to base relations as possible
2. **Join ordering** — Use dynamic programming or greedy algorithm to find
   optimal join order based on cardinality estimates
3. **Index selection** — Choose which columns to index for efficient lookups
4. **Magic set transformation** — Restrict recursive computations to only
   compute tuples relevant to the query (demand-driven evaluation)
5. **Common subexpression elimination** — Share computation across predicates
6. **Materialization decisions** — Decide which intermediate results to cache

### 4.6 Execution Engine (`ocql-engine`)

**Input:** Query plan + Database
**Output:** Result tuples

The engine is the heart of open-cql. Key design decisions:

#### 4.6.1 Semi-Naive Evaluation
For recursive predicates, use semi-naive evaluation:
- Track only *new* tuples per iteration (delta relations)
- Each iteration joins delta with full to find new derivations
- Converge when no new tuples are produced

#### 4.6.2 Flow-Tracking Extension
This is where open-cql goes beyond traditional Datalog:

**Problem:** Standard Datalog computes reachability (can data flow from A to B?).
CodeQL needs *exact flow paths* (what is the specific path from A to B?).

**Solution: Provenance-Tracking Evaluation**

Each derived tuple carries provenance metadata:
```rust
struct Tuple {
    values: Vec<Value>,
    provenance: Option<Provenance>,
}

enum Provenance {
    Base,                           // From base relation
    Derived {
        rule: RuleId,              // Which rule derived this
        inputs: Vec<TupleRef>,     // Which input tuples were used
    },
    Flow {
        path: Vec<FlowStep>,      // Exact flow path
    },
}

struct FlowStep {
    node: Value,                   // The intermediate node
    kind: FlowKind,               // What kind of step (call, return, field, etc.)
}
```

For data flow queries, the engine uses a specialized **flow-sensitive fixpoint**:
- Maintains a worklist of (node, access_path, context) triples
- Propagates flow along edges in the flow graph
- Tracks exact paths, not just reachability
- Supports context sensitivity (call-site sensitivity)
- Handles access paths (obj.field.subfield tracking)

#### 4.6.3 Storage Backend

Relations stored as sorted arrays of tuples with B-tree indexes on selected columns.
This provides:
- Cache-friendly sequential scans
- O(log n) lookups on indexed columns
- Efficient merge joins on sorted data

### 4.7 Database and Extractors

#### 4.7.1 Database Format (`ocql-database`)

The database is a directory containing:
```
database/
├── schema.ocqlscheme          # Schema definition
├── relations/                 # One file per relation
│   ├── files.rel
│   ├── locations.rel
│   ├── exprs.rel
│   └── ...
├── source/                    # Source archive
│   └── ...
└── metadata.json              # Database metadata
```

#### 4.7.2 Schema (`ocql-schema`)

Parse CodeQL `.dbscheme` files to understand the relational schema. The schema defines:
- Table names and their columns
- Column types (int, string, entity references)
- Key constraints
- Union types

We should aim for compatibility with CodeQL's `.dbscheme` format to leverage
existing schemas.

#### 4.7.3 C/C++ Extractor (`ocql-extractor-cpp`)

**Strategy:** Use tree-sitter for parsing, then map the CST to relational facts.

```
C/C++ Source → [tree-sitter-cpp] → CST → [Mapper] → Relations
```

The extractor must populate all tables defined in `semmlecode.cpp.dbscheme`:
- File and location tables
- Type tables (built-in, derived, user-defined)
- Declaration tables (functions, variables, classes)
- Expression and statement tables
- Control flow edges
- Call graph edges

**Phases:**
1. Parse all source files with tree-sitter
2. Build symbol table (types, functions, variables)
3. Resolve references and types
4. Emit relational facts (TRAP-like format)
5. Build database

**Challenge:** C++ requires name lookup, template instantiation, and overload
resolution for accurate extraction. We may need to integrate with a real C++ frontend
(e.g., clang's libTooling) for semantic analysis.

#### 4.7.4 Java Extractor (`ocql-extractor-java`)

Similar approach but simpler due to Java's cleaner semantics:
```
Java Source → [tree-sitter-java] → CST → [Mapper] → Relations
```

Java extraction is more straightforward because:
- No preprocessing
- No templates (generics are simpler)
- Clear name resolution rules
- Well-defined type system

## 5. Evaluation Engine: Deep Dive

### 5.1 Why Not Pure Datalog

Standard Datalog computes the *least fixed point* of a set of rules. This gives
reachability: "there exists a path from A to B." But CodeQL needs more:

1. **Exact flow paths** — Not just "A reaches B" but "A reaches B via C, D, E"
2. **Access paths** — Tracking `obj.field.subfield` through assignments
3. **Context sensitivity** — Distinguishing flow through different call sites
4. **Partial flow** — "Show me how far tainted data gets" (not just full source-to-sink)

### 5.2 Proposed: Datalog with Provenance

Our engine extends Datalog with:

**Path-tracking joins:** When joining flow relations, concatenate paths:
```
flow(a, c, path_ac) :- flow(a, b, path_ab), step(b, c), path_ac = path_ab ++ [c]
```

**Access path tracking:** Maintain abstract access paths alongside flow:
```
flow(node, ap, ctx) :- flow(prev, ap', ctx'), step(prev, node, transform),
                       ap = transform(ap'), ctx = update_ctx(ctx', node)
```

**Stratified evaluation with flow awareness:**
- Stratum 0: Base facts (from database)
- Stratum 1: Local flow (within functions)
- Stratum 2: Interprocedural flow (across calls/returns)
- Stratum 3: Global taint (including non-value-preserving steps)
- Stratum 4: User queries (using flow results)

### 5.3 Performance Considerations

- **Indexed relations** — B-tree indexes on columns used in joins
- **Incremental maintenance** — When adding new facts, recompute only affected strata
- **Parallelism** — Independent strata can be evaluated in parallel
- **Compression** — Intern strings and large values; use integer IDs
- **Memory management** — Arena allocation for tuples within a stratum

## 6. Implementation Phases

### Phase 1: Foundation (Weeks 1-4)
- [ ] Rust workspace setup
- [ ] QL lexer and parser (basic subset)
- [ ] AST data structures
- [ ] .dbscheme parser
- [ ] Basic database storage

### Phase 2: Core Pipeline (Weeks 5-10)
- [ ] HIR: name resolution, type checking
- [ ] MIR: class elimination, predicate flattening
- [ ] LIR: relational algebra representation
- [ ] Basic evaluation engine (non-recursive)
- [ ] Simple queries working end-to-end

### Phase 3: Recursion & Flow (Weeks 11-16)
- [ ] Semi-naive evaluation for recursive predicates
- [ ] Negation stratification
- [ ] Flow-tracking extension
- [ ] Access path tracking
- [ ] Context-sensitive analysis

### Phase 4: Extractors (Weeks 17-22)
- [ ] tree-sitter integration
- [ ] C/C++ extractor (basic AST extraction)
- [ ] Java extractor (basic AST extraction)
- [ ] Type resolution for both languages
- [ ] Control flow graph construction

### Phase 5: Standard Library (Weeks 23-30)
- [ ] Core QL standard library (basic predicates)
- [ ] C/C++ standard library (AST classes, basic data flow)
- [ ] Java standard library (AST classes, basic data flow)
- [ ] DataFlow::ConfigSig implementation
- [ ] TaintTracking module

### Phase 6: Optimization (Ongoing)
- [ ] Join ordering optimization
- [ ] Magic set transformation
- [ ] Index selection
- [ ] Parallelization
- [ ] Benchmarking against CodeQL

## 7. Key Design Decisions

### 7.1 Hand-written parser vs parser generator
**Decision:** Hand-written recursive descent.
**Rationale:** Better error messages, easier incremental parsing, proven approach.

### 7.2 tree-sitter vs full compiler frontend for extractors
**Decision:** Start with tree-sitter, upgrade to clang/javac integration if needed.
**Rationale:** tree-sitter is fast and sufficient for AST extraction. Semantic analysis
(type resolution, overload resolution) may require a full frontend later.

### 7.3 Storage format
**Decision:** Custom binary format with sorted arrays and B-tree indexes.
**Rationale:** Optimized for the specific access patterns of Datalog evaluation
(sequential scans + index lookups). No need for a general-purpose database.

### 7.4 Flow tracking approach
**Decision:** Provenance-enriched tuples with path metadata.
**Rationale:** Enables exact flow path reporting without post-hoc path reconstruction.
Trade-off is higher memory usage, mitigated by lazy path materialization.

### 7.5 .dbscheme compatibility
**Decision:** Parse CodeQL's .dbscheme format directly.
**Rationale:** Allows us to reuse existing schema definitions and potentially
existing CodeQL databases for testing.

## 8. Testing Strategy

### 8.1 Unit tests per crate
Each crate has its own unit tests covering core functionality.

### 8.2 Snapshot tests for IR stages
Each IR transformation produces textual output that is snapshot-tested:
- Parse a .ql file -> compare AST dump
- Lower to HIR -> compare HIR dump
- Lower to MIR -> compare MIR dump
- Lower to LIR -> compare LIR dump

### 8.3 End-to-end tests
Small CodeQL queries run against small test databases, comparing results
with CodeQL's own output.

### 8.4 Behavioral compatibility tests
Use CodeQL's published test cases as behavioral specifications:
- Given this source code and this query, expect these results.
- We don't copy the test infrastructure, just the expected behaviors.

## 9. References

- [QL Language Reference](https://codeql.github.com/docs/ql-language-reference/)
- [CodeQL Documentation](https://codeql.github.com/docs/)
- [CodeQL Repository](https://github.com/github/codeql) (MIT-licensed queries)
- [Soufflé Datalog](https://souffle-lang.github.io/) (reference implementation, not used)
- Scholz et al., "On fast large-scale program analysis in Datalog" (2016)
- Smaragdakis & Bravenboer, "Using Datalog for Fast and Easy Program Analysis" (2010)
