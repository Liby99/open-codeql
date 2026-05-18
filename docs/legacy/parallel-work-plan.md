# Parallel Work Plan: Qlpack End-to-End Execution

## Track A: Fix Stratification (Evaluation Unblocking)

**Goal**: Get the 56,105 engine rules from vendor/codeql/cpp to evaluate successfully.

**Deliverables**:
1. Engine evaluates 56K rules against basic.c without stratification error
2. Class characteristic predicates (`Function#char`, `Element#char`) produce correct tuples
3. Regression test: `qlpack_probe.rs` evaluation phase succeeds

**Tasks**:
1. Add a "skip-cycles" mode to the engine stratifier that drops rules involved in negation cycles
2. Evaluate the remaining rules and verify core predicates work
3. Implement proper fix: inline double-negation patterns in MIR (`forall`/`implies` → direct AntiJoin without auxiliary predicate)
4. Re-run full evaluation and measure how many predicates produce results

**Testable Milestones**:
- [ ] Engine evaluates with cycle-skipping, >90% of rules execute
- [ ] `Function#char` relation has 3 rows when run against basic.c
- [ ] Proper fix: 0 false negation cycles on vendor/codeql

---

## Track B: Fix Inheritance Dispatch + Closure Operators (MIR)

**Goal**: Member calls on subclass-typed variables resolve to parent class methods. Transitive closure works.

**Deliverables**:
1. `f.getName()` where `f : SubClass` finds `ParentClass#getName`
2. `edges+(a, b)` generates recursive transitive closure predicate
3. `x in [1, 2, 3]` creates proper disjunction
4. Regression tests for all three

**Tasks**:
1. **Dispatch predicates**: After lowering all files, build a map of `method_name → [Class#method_name]`. For each method name, generate a dispatch predicate: `method_name(this, args...) :- Class1#method_name(this, args...); ...`
2. **Closure operators**: When `ClosureOp::Plus` is present on a call, generate a recursive predicate: `_tc_P(a, b) :- P(a, b). _tc_P(a, b) :- P(a, c), _tc_P(c, b).`
3. **SetLiteral fix**: Generate disjunction for `[a, b, c]`.

**Testable Milestones**:
- [ ] E2E test: `from SubClass f select f.parentMethod()` returns correct results
- [ ] E2E test: `edges+(a, b)` returns transitive closure
- [ ] E2E test: `x in [1, 2, 3]` matches all three values
- [ ] Existing 34 E2E tests still pass

---

## Track C: Schema Alignment + String Builtins (Engine/Database)

**Goal**: Our C++ extractor schema matches what vendor library classes expect. String methods work.

**Deliverables**:
1. Audit of which database tables vendor library classes reference vs what our extractor produces
2. Engine built-in predicates for string methods (`toString`, `regexpMatch`, `matches`)
3. Mapping layer between our schema column names and CodeQL schema column names

**Tasks**:
1. **Schema audit**: Compare `semmlecode.cpp.dbscheme` tables with our `cpp_schema()`. List missing tables and column mismatches.
2. **String builtins**: Add engine support for `string.toString()`, `int.toString()`, `string.length()`, `string.matches(pattern)`.
3. **Table name mapping**: If our extractor uses different table/column names than the vendor schema, add a translation layer.

**Testable Milestones**:
- [ ] Document listing all table mismatches between schemas
- [ ] Engine test: `"hello".length() = 5` evaluates correctly
- [ ] Engine test: `42.toString() = "42"` evaluates correctly
- [ ] Schema coverage report: X/Y tables from vendor schema are populated by our extractor
