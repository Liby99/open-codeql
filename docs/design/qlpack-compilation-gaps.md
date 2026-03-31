# Qlpack Compilation Gap Analysis

**Date**: 2026-04-01
**Status**: Working from mini-qlpack; vendor/codeql C++ library compiles through MIR but fails at engine evaluation.

## Pipeline Status (vendor/codeql/cpp/ql/lib)

| Stage | Status | Numbers |
|-------|--------|---------|
| HIR analysis | **100%** | 594/594 files, 0 errors, ~6s |
| MIR lowering (per-file) | **100%** | 594/594 files, 41,379 predicates |
| MIR lowering (merged) | **100%** | 41,379 predicates |
| Engine emission | **100%** | 56,105 rules, 37,638 head predicates |
| Evaluation against basic.c | **FAILS** | Stratification error (false negation cycles) |

## Blocking Issues (P0)

### 1. False Negation Cycles in Stratification

**Impact**: Prevents evaluation of any program containing `forall`/`implies`/`if-then-else` patterns
**Root cause**: MIR lowers `forall(vars | guard | body)` as `not exists(vars | guard | not body)`, creating auxiliary predicates `_forall_neg_N` with double-negation:
- `_forall_neg_N(x) :- guard(x), not body(x)`
- Parent uses `not _forall_neg_N(x)`

When these auxiliary predicates end up in Tarjan SCCs (via transitive dependencies), the stratifier sees a negation cycle, which is illegal in stratified Datalog. But these are actually well-founded — the inner negation is always over a fully-computed predicate.

**Affected**: ~41 files in vendor/codeql, but since all are merged into one program, ONE bad cycle blocks everything.

**Possible fixes**:
- **(A) Inline auxiliary predicates**: Instead of creating `_forall_neg_N` as a separate predicate and negating it, inline the negation directly using AntiJoin in the LIR. This eliminates the artificial predicate that creates the cycle.
- **(B) Refined stratification**: Detect double-negation patterns (`not _forall_neg_N` where `_forall_neg_N` uses `not`) and treat the outer negation as effectively positive (since double negation = positive).
- **(C) Semi-naive with well-founded semantics**: Replace Tarjan-based stratification with well-founded semantics that can handle stratified double negation.
- **(D) Skip problematic predicates**: For testing purposes, filter out predicates that cause cycles and evaluate the rest.

**Recommended**: Start with **(D)** for immediate progress, then implement **(A)** as the proper fix.

### 2. Inheritance Dispatch for Member Calls

**Impact**: `f.method()` where `f` is a subclass type can't find parent class methods
**Root cause**: When `from SubClass f` is declared, the MIR resolves `f.method()` to `SubClass#method` but the method is defined on `ParentClass`. Only `ParentClass#method` exists as a predicate.

**Scope**: Every real query using class hierarchies — virtually all CodeQL queries.

**Possible fixes**:
- **(A) Dispatch predicates**: For each method name, generate a union predicate that dispatches to all classes defining it. `getName(this, result) :- Function#getName(this, result); getName(this, result) :- Element#getName(this, result)`.
- **(B) Walk supertype chain**: When resolving `SubClass#method`, if not found, check `ParentClass#method`, then `GrandparentClass#method`, etc.
- **(C) Use HIR type resolution**: The HIR resolves member calls to specific DefIds. Pass this resolution info to MIR so it generates the correct qualified name.

**Recommended**: **(A)** is simplest and matches how CodeQL works internally.

### 3. Transitive Closure Operators (`+`, `*`)

**Impact**: Graph traversal queries produce only one-hop results
**Root cause**: `ClosureOp::Plus` and `ClosureOp::Star` on calls are silently ignored. `edges+(a, b)` is evaluated as `edges(a, b)`.

**Scope**: Path queries, reachability analysis, call graph traversal — core of security analysis.

**Possible fixes**:
- **(A) Generate recursive predicates**: `edges+(a, b)` → create `_tc_edges(a, b) :- edges(a, b). _tc_edges(a, b) :- edges(a, c), _tc_edges(c, b).`
- **(B) Engine-level closure**: Add a `TransitiveClosure` atom type to the engine.

**Recommended**: **(A)** — pure MIR transformation, no engine changes needed.

## High-Priority Issues (P1)

### 4. SetLiteral Only Uses First Element

**Impact**: `x in [1, 2, 3]` only matches `1`
**Fix**: Create disjunction over all elements. `x in [a, b, c]` → auxiliary predicate with `MirBody::Disjunction([[x=a], [x=b], [x=c]])`.

### 5. Predicate Name Collisions in Merged Lowering

**Impact**: Different files defining predicates with the same name will collide
**Root cause**: MIR uses flat predicate names. Two files both defining `helper(x)` will merge into one predicate.
**Fix**: Namespace predicates by file/module when lowering multi-file programs.

### 6. `bindingset` Annotations Lost

**Impact**: Performance — some predicates may compute over infinite domains
**Root cause**: MIR doesn't propagate `bindingset` annotations to engine rules.
**Fix**: Add binding mode information to MIR predicates and engine rules.

## Medium-Priority Issues (P2)

### 7. `cached` / `pragma` Annotations Lost
- Performance hints that help query planning
- Not functionally blocking

### 8. String Built-in Methods Not Implemented
- `regexpMatch()`, `replaceAll()`, `toLowerCase()`, `matches()`, `splitAt()`
- Needed for string-processing queries
- Requires engine-level built-in predicates

### 9. `super` Keyword in Member Predicates
- Partially handled but may not resolve correctly in all cases
- Depends on inheritance dispatch (#2)

### 10. Abstract Predicates with No Body
- Lowered as `MirBody::None` — correct for truly abstract predicates
- But real CodeQL abstract predicates get bodies from subclass overrides
- Need to collect override bodies and union them

## Low-Priority Issues (P3)

### 11. Newtype Declarations Skipped
### 12. Module Aliases Not Lowered (pre-resolved by HIR)
### 13. Class Fields Not Lowered (schema-level)
### 14. Predicate Aliases Not Lowered (pre-resolved by HIR)

## Empirical Data

### Mini-qlpack (crates/ocql-e2e-tests/tests/fixtures/mini-qlpack/)
- 5 files: Function.qll, LocalVariable.qll, 3 .ql queries
- **Fully working end-to-end**: HIR → MIR → engine → correct results against basic.c
- Tests: `qlpack_tests.rs` (5 tests, all pass)

### Vendor C++ (vendor/codeql/cpp/ql/lib/)
- 594 files, 41,379 predicates, 56,105 engine rules
- HIR + MIR: 100% success
- Engine evaluation: blocked by stratification error (#1)

### Estimated Fix Priority for End-to-End on Real Queries

```
#1 (stratification) → unlocks evaluation of 56K rules against databases
#2 (inheritance)    → unlocks correct method dispatch for class hierarchies
#3 (closure ops)    → unlocks graph/reachability queries
#4 (set literals)   → correctness for set membership tests
```

With #1 and #2 fixed, simple queries like `ExtractedFiles.ql`, `DeadCodeFunction.ql` become possible. With #3, path/security queries become possible.
