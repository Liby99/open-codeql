# Conformance

- **Layer:** Cross-cutting
- **Implementation crate:** `ocql-e2e-tests` (plus per-crate test directories)
- **Status:** Provisional. The current test suite is the seed; the matrix below is the target.
- **Version:** 0.1

## 1. Purpose

This document defines what it means for open-codeql to be **conformant**
to its own specs and to **upstream CodeQL**.

There are two distinct conformance concerns:

1. **Self-conformance:** the implementation matches L0–L5 as written.
2. **Upstream parity:** for inputs (databases and queries) that exist in
   both systems, results match upstream CodeQL up to documented
   deviations.

A change that breaks self-conformance is a bug. A change that introduces
a new deviation from upstream parity is acceptable but **must be
recorded** here.

## 2. Self-conformance matrix

Each spec layer pairs with a test surface:

| Layer | Test surface | Status |
|-------|--------------|--------|
| L0 | `crates/ocql-ql-parser/tests/syntax_deviations/` | Provisional |
| L1 | `crates/ocql-hir/tests/`, `crates/ocql-e2e-tests/tests/oqlpack_*.rs` | Partial |
| L2 | `crates/ocql-mir/tests/snapshots/` (Provisional) | Partial |
| L3 (Datalog core) | `crates/ocql-engine/tests/` | Active |
| L3 (Flow extension) | (TBD when L3 §5 ships) | Not started |
| L4 | `crates/ocql-database/tests/` (Provisional) | Not started |
| L4-X (Java) | `crates/ocql-e2e-tests/tests/e2e_tests.rs` Java cases, `java_parity.rs` | Active |
| L4-X (C/C++) | `crates/ocql-e2e-tests/tests/e2e_tests.rs` C cases | Active |
| L4-X (others) | (TBD when CLI exposes them) | Not started |
| L5 (Java) | `crates/ocql-e2e-tests/tests/oqlpack_java.rs` | Active |

A layer's status is **Active** once at least one test exists for every
section of the spec; **Partial** when some sections are tested; **Not
started** otherwise.

### 2.1 Adding tests when adding a feature

When adding a feature that is described in a spec section, add a test in
the corresponding test surface with a comment that names the spec
section, e.g.:

```rust
/// Conformance: L1 §3.4 (subtyping `int <: float` for arithmetic).
#[test]
fn int_promotes_to_float_in_arithmetic() { … }
```

The comment is the audit trail.

## 3. Upstream parity

When `vendor/codeql/` is checked in, parity is verified by a per-fixture
harness:

```
fixture: vendor/test-repos/<repo>
query  : <query.ql>

upstream: codeql query run --database=<extracted-by-codeql> <query>
oqlcl   : ocodeql query run --database=<extracted-by-ocodeql> <query>

result : compare normalized result tables.
```

### 3.1 Normalization

Direct table comparison fails because of identity differences (entity
ids, file path absoluteness). The parity harness applies:

- Replace `Entity(_)` with the result of the entity's `toString()`.
- Replace absolute file paths with paths relative to the repo root.
- Sort the result rows lexicographically.

### 3.2 Tolerated deviations

Each tolerated deviation lives in `docs/spec/deviations/` (Provisional)
as a markdown file with a stable id:

```
deviations/
   D-flow-precision.md            // open-codeql may report fewer flow paths
   D-template-instantiation.md    // open-codeql does not instantiate C++ templates
   D-string-canonicalization.md   // open-codeql normalizes string literals differently
```

Each file:

- States the deviation in one sentence.
- Names the affected layer(s).
- Is referenced by the parity harness so that affected fixtures don't
  fail the build.

### 3.3 Adding a deviation

Adding `D-NEW.md` requires:

- A failing parity test that exhibits the deviation.
- An entry in `D-NEW.md` describing root cause and fix plan (or "won't
  fix").
- Mention in the relevant L-spec under `Open questions` or as a
  Provisional clause.

## 4. Test corpus

Self-conformance and parity both run against fixtures already in the
repo:

- `vendor/test-repos/qoi`, `rax`, `utf8.h`, `lua-cjson`, `dperf`,
  `libpng`, `lua`, `coreutils` — C/C++.
- `vendor/test-repos/gson`, `jsoup`, `GsonFactory`,
  `auto-value-{gson,moshi}` — Java.
- `vendor/test-repos/flask`, `httpie`, `black` — Python.
- `vendor/test-repos/ripgrep`, `fd`, `bat`, `hugo`, `coreutils` —
  Rust/Go/Hugo.
- … (see `.gitmodules` for the full list).

The corpus is *not* the same as the conformance fixtures. A fixture is a
**curated, minimal** input designed to exercise one spec section. The
corpus is for empirical "does this work on real code" testing.

## 5. CI gates

A green build requires:

- All Active-status tests pass.
- No Provisional test that has been promoted to Active is regressing.
- The parity harness passes on the parity-pinned subset of the corpus.

A test that fails because of a known deviation is annotated `#[ignore =
"D-NEW"]` and counted in the Tolerated Deviations report (TBD).

## 6. Promotion lifecycle

A spec section's status moves through:

```
Draft → Provisional → Active → Ratified
```

- **Draft → Provisional**: the spec section is written and reviewed but
  has no tests.
- **Provisional → Active**: at least one test for every part of the
  section exists and passes.
- **Active → Ratified**: the section has been Active for at least one
  release with no deviations recorded against it.

Demotion (Ratified → Active, etc.) requires the same review process as
promotion and is performed by adding an entry to the layer's "Open
questions" section.

## 7. Tools

- `scripts/analyze_parse_errors.py` — categorizes parse failures by
  spec section.
- `scripts/turbofish_transform.py` — applies the L0 transform to a tree
  of `.ql` / `.qll` files.
- (Provisional) `scripts/parity_run.sh` — invokes upstream CodeQL on a
  fixture and compares results with normalization §3.1.

## 8. Reporting conformance

Each release should ship a short conformance report listing:

- Layers at each status level.
- Tolerated deviations.
- Parity coverage on the corpus (count of fixtures, count of queries
  passing parity).

The report lives at `docs/spec/conformance-reports/<version>.md`
(Provisional).
