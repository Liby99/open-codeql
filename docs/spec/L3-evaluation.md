# L3 — Evaluation Semantics

- **Layer:** L3
- **Implementation crate:** `ocql-engine`
- **Status:** Ratified for §4 (Datalog core); Provisional for §5 (Flow extension) and §7 (Neurosymbolic seams).
- **Version:** 0.1

## 1. Purpose

L3 specifies how an L2 program is evaluated against an L4 database. It has
two parts:

- **§4 Datalog core** — the standard semi-naive evaluation of a stratified
  Datalog program. This is what `ocql-engine` does today.
- **§5 Flow extension** — provenance-tracking, access-path-aware,
  context-sensitive evaluation. This is the principal differentiator from
  vanilla Datalog and the principal piece still to be built.

L3 is written so that the Flow extension is **strictly additive** over the
Datalog core: if a program does not use the flow operators of L5, the
engine evaluates it as plain stratified Datalog and produces the same
results either way.

## 2. Inputs and outputs

```
L3 evaluator: (MirProgram, Database) → Database'
```

- The input `Database` is a sealed L4 database (no further extraction
  occurs).
- The output `Database'` is `Database` plus one relation per `MirPredicate`
  of `MirProgram`. Predicates whose body produces no tuples are present
  with an empty extent.
- Idempotence: evaluating the same `(Program, Database)` twice yields
  identical `Database'` (modulo non-deterministic ordering inside
  individual relations, which L4 normalizes — see L4 §3).

## 3. Values and tuples

L3 inherits the value model of L4:

```
Value  = Int(i64) | Float(f64) | String(InternedString)
       | Bool(bool) | Entity(u64) | Null
Tuple  = SmallVec<Value>
```

Equality on `Value` is structural; `Float` uses ordered-float total
ordering. `String` equality is by handle (interning is mandatory for
correctness, not just performance).

## 4. Datalog core

### 4.1 Stratification

L3 receives the stratification computed at L2 §4. Strata are evaluated in
topological order; within a stratum the engine has discretion over join
order.

### 4.2 Semi-naive evaluation

For each stratum:

- **Non-recursive stratum** (no SCC of size > 1, no self-loop): a single
  forward chaining pass over each rule.
- **Recursive stratum**: semi-naive iteration. Each predicate `p` carries
  three sets: `full(p)`, `delta(p)`, `to_add(p)`. At iteration *i+1*:

  ```
  for each rule r with head p:
     to_add(p) ∪= eval(r) using delta(...) for at least one body atom
                  and full(...) for the rest
  delta(p)   = to_add(p) \ full(p)
  full(p)   ∪= delta(p)
  to_add(p)  = ∅
  ```

  The fixpoint is reached when every `delta(p) = ∅` for `p` in the
  stratum.

The implementation uses **slot-based variable binding** (a `Vec<Option<
Value>>` indexed by variable id) for join inner loops; `HashMap<String,
Value>` is forbidden in the hot path. See `ocql-engine/src/eval.rs:80-126`.

### 4.3 Negation

`NegScan(p, args)` succeeds iff no tuple in `full(p)` matches `args` after
the existing variable bindings of the rule. Negation is only legal in a
stratum strictly later than `p`'s stratum (enforced at L2 §4).

### 4.4 Guards and arithmetic

Guards (`Guard`) and assignments (`Assign`) are evaluated **after** all
positive atoms of the rule have produced bindings for the referenced
variables. Range expressions (`x in [a..b]`) lower to a `Guard` plus an
`Assign` chain at L2 and follow the same rule.

### 4.5 Aggregates

For an `Aggregate { result, fn, sub_pred, group_by, agg_var }`:

1. Evaluate `sub_pred` to fixpoint under the standard semi-naive rules.
2. Group its tuples by the `group_by` columns.
3. Apply `fn ∈ { Count, Sum, Min, Max, Avg, Concat, Rank, Unique, Any,
   StrictCount, StrictSum, StrictConcat }` to the projection on
   `agg_var`.
4. Bind `result` to the per-group aggregate.

> **Conformance gap (Provisional):** `Concat` separator strings and `order
> by` clauses for `Concat` / `Rank` are accepted by L2 but not honoured by
> the current engine.

### 4.6 Termination

The Datalog core terminates iff every stratum has a finite fixpoint.
Because L2 admits no infinite-domain constructors at L3 (no Skolem
functions, no `succ/1`), this is guaranteed for every well-typed L2
program.

### 4.7 Determinism

The output relation extents are sets, so they are order-independent.
Within a relation L4 stores tuples sorted; the engine emits tuples in
arbitrary order during evaluation and L4 normalizes on save.

## 5. Flow extension — Provisional

### 5.1 Motivation

Plain Datalog computes **reachability**: "there exists a derivation of
`p(t)` in the program." CodeQL queries need more:

- **Exact paths**: the *witness* of `flow(source, sink)`, not just its
  truth.
- **Access paths**: tracking `obj.f.g` through assignments without losing
  field selectors.
- **Context sensitivity**: distinguishing flow through different call
  sites of the same callee.
- **Partial flow**: "show me how far tainted data gets" (a stratified
  fragment of a flow query).

### 5.2 Provenance-tagged tuples

A tuple in a **flow predicate** carries a provenance tag:

```
Provenance = Base
           | Derived { rule_id, inputs : [TupleRef] }
           | Flow    { path : [FlowStep] }

FlowStep   = { node : Value, kind : FlowKind, ctx : ContextId }
FlowKind   = Local | Call | Return | Field(InternedString) | Through
```

Flow predicates are marked at L2 (a `MirPredicate` flag, **TBD**); other
predicates evaluate without provenance overhead. Provenance is *materialized
lazily*: only the path-witnesses for tuples that survive into the final
projection are reified.

### 5.3 Flow fixpoint

The flow extension augments §4.2 with:

- A **flow-aware join** that concatenates paths instead of intersecting
  domains.
- An **access-path lattice** with a configurable depth bound (default 4)
  and a widening operator that collapses paths exceeding the bound to a
  top-of-lattice element.
- A **context store** keyed by call-site, with a configurable depth bound
  (default 1; i.e. 1-CFA).

### 5.4 Soundness vs. precision

The flow extension is **sound up to** the configured depth bounds and the
honesty of L4's call-graph relation. It is **not** sound under reflective
or dynamic dispatch unless those edges are present in the database.

The bounds are tunable per query via L2 pragmas (TBD).

### 5.5 Compatibility with §4

A flow predicate, viewed without provenance, satisfies the same Datalog
semantics as in §4. A query that only inspects the truth values of flow
predicates produces the same answers under both evaluators.

## 6. Engine API

```
ocql_engine::evaluate(program: &Program, db: &mut Database)
    -> Result<(), EvalError>
```

`Program` is the `ocql-engine`-level rule program (the current evaluator
input); the L2 `MirProgram` lowers to it via
`compile_ql_to_engine`. Once LIR (L2 §5) is built this becomes
`(LirProgram, Database) → Database'`.

Errors:

- `Stratification(span, predicate)` — caught at L2 but surfaced through
  L3 for diagnostic continuity.
- `TypeMismatch(span)` — runtime type tag mismatch (should be impossible
  for L1-checked programs; raised as a defect of higher layers).
- `Resource(kind)` — out of memory / out of time / depth bound exceeded.

## 7. Neurosymbolic seams — Provisional

The flow extension exposes three principled hook points for neural
augmentation:

### 7.1 Learned step relations

A flow step `step(a, b)` may be **provided by a model**, not the L4
database. The model is consulted at evaluation time:

```
LearnedStep : (a) → distribution over { b }
```

The engine treats the model's output as a relation with a confidence
column and propagates confidence multiplicatively along paths.

### 7.2 Learned barriers

A model may classify a candidate flow tuple as "almost certainly
impossible" and remove it before fixpoint. This is a precision
improvement, not a soundness one.

### 7.3 LLM-proposed summaries

A model may produce an L2 predicate definition (for an external library
function) on the fly. The engine accepts a structured `Summary { pred,
inputs, outputs, kind }` and treats it as a base relation for the
duration of the query.

These three seams are deliberately specified at **L3**, not at L1, so
that a neural extension does not require re-parsing or re-typing of the
QL program. They are also deliberately specified separately so that an
implementation may build them in any order.

## 8. Conformance tests

- **Datalog core** (§4): the existing `crates/ocql-engine/tests/` suite
  is the seed. Every test file there is part of the L3 conformance suite.
- **Flow extension** (§5): when implemented, each kind of flow step gets
  a fixture `(database, query, expected paths)`. Path equality is up to
  the order of complete paths (the set of paths is the contract).
- **Neurosymbolic seams** (§7): mock models with deterministic outputs
  for reproducibility.

## 9. Open questions

- **Path interning (§5.2):** are paths first-class `Value`s (and thus
  storable in L4), or only ephemeral provenance metadata?
- **Confidence semantics (§7.1):** are confidences combined as
  probabilities, log-probabilities, or as a custom semiring?
- **Flow-predicate marking (§5.2):** is this a syntactic mark in L0/L1,
  an annotation in the standard library, or a planner inference?
