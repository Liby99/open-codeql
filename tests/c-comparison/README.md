# C Comparison Test Suite

Compare open-codeql (ocodeql) against CodeQL for C analysis.

## Test Structure

```
tests/c-comparison/
├── projects/              # C source files for testing
│   ├── BasicStructure.c     # Functions, structs, unions, enums, globals, typedefs
│   ├── ControlFlow.c        # All statement types (if/switch/for/while/do/break/continue/goto)
│   ├── CallGraph.c          # Direct calls, mutual recursion, deep chains
│   ├── Operators.c          # Every C operator form (arithmetic/bitwise/logical/…)
│   ├── Literals.c           # Numeric / char / string / hex / octal literals
│   └── SecurityPatterns.c   # Known vulnerable patterns (CWE-120, CWE-134, etc.)
├── queries/               # Raw-table QL queries (compile against both schemas)
│   ├── 01_function_names.ql … 16_literal_values.ql
├── run_comparison.sh      # Script to run both codeql + ocodeql and diff results
├── results/               # (generated) comparison output
└── README.md
```

Current status: **96/96 PASS** (6 projects × 16 queries).

## Running

```bash
# Build ocodeql first
cargo build --release -p ocodeql

# Then run the harness (auto-detects vendor/codeql-cli-*/codeql)
cd tests/c-comparison
./run_comparison.sh                     # All projects x all queries
./run_comparison.sh BasicStructure      # One project, all queries
./run_comparison.sh BasicStructure 01   # One project, one query
```

## Comparison semantics

The script normalises both sides (sort, dedup, strip quotes) and reports:

- **PASS**: outputs are identical, OR one side is a strict subset of the
  other (treated as PASS because vendor codeql sees preprocessor-expanded
  headers while ocodeql currently sees only the input `.c` file).
- **DIFF**: both sides have rows the other does not — a genuine
  divergence in extractor or query semantics.
- **SKIP**: one side errored out.

The normaliser also trims leading/trailing whitespace (CodeQL emits a leading
space on some constant valuetexts — a frontend serialisation quirk).

Current status: **96/96 PASS**.

## Rust parity tests

`crates/ocql-e2e-tests/tests/c_parity.rs` pins the extractor's output for
each comparison project. Run with:

```bash
cargo test -p ocql-e2e-tests --test c_parity
```

## Schema overlap

Queries are written so the same `.ql` source compiles against both:

- our extractor schema (`crates/ocql-extractor-cpp/src/schema.rs`) and
- vendor `semmlecode.cpp.dbscheme`.

Tables targeted: `functions`, `usertypes`, `stmts`, `exprs`, `iscall`,
`funbind`, `enclosingfunction`. These have identical column shapes
across both schemas.

Tables NOT yet covered (column shapes diverge):
`params`, `fields`, `localvariables`, `globalvariables`,
`membervariables`, `enumconstants`, `derivations`. Queries against these
will need either schema realignment or per-side query variants.

## Known gaps

| Gap | Status | Impact |
|-----|--------|--------|
| Header / preprocessor extraction | NOT IMPLEMENTED | We see only the `.c` file; vendor sees the expanded TU. Comparator handles this via subset-PASS. |
| `params` schema alignment | NOT ALIGNED | Vendor: `(@parameter, function, index, type_id)`; ocodeql: `(function, index, name, param_type)`. Different `unique` key. |
| `fields` schema alignment | NOT ALIGNED | Vendor uses `@membervariable`-keyed rows; we emit `(parent, index, name, field_type)`. |
| `derivations` schema alignment | NOT ALIGNED | Different column order and key. |
| `enumconstants` schema alignment | NOT ALIGNED | Different column count. |
| `localvariables` / `globalvariables` schema alignment | NOT ALIGNED | Different column order and types. |
| Function/type entity ID parity | NOT TARGETED | We don't aim to match vendor's IDs — only names/kinds. |

## Gaps found & fixed

Surfaced by adding the `Operators.c` / `Literals.c` projects and per-kind
count probes, then fixed in the extractor:

- **Expression-kind alignment** — `EXPR_*` constants now match vendor's
  `case @expr.kind of` enum (e.g. `@routineexpr` 97 for calls, `@c_style_cast`
  214, `@literal` 123, swapped comparison/shift/crement kinds).
- **Parentheses stripped** — `parenthesized_expression` no longer emits a
  `@parexpr` (kind 12) wrapper; CodeQL's clang frontend strips them.
- **Typedef of anonymous struct** — emits TWO rows like vendor: the anonymous
  struct (`(unnamed class/struct/union)`, kind 1) plus a kind-18 `@alias`.
- **`valuetext` vs `accessname` split** — `valuetext` now holds only literal
  values (matching vendor); variable/function/field access names moved to the
  ocodeql-internal `accessname` table. Fixed a `@varaccess` over-count (we used
  to emit a redundant callee `@varaccess` per call).
- **String-literal value text** — stored without surrounding double quotes
  (`"hello"` → `hello`) to match CodeQL.
- **`for`-initializer declarations** — `for (int i = 0; …)` now records the
  initializer as a `@stmt_decl` (kind 17) child, like CodeQL.

## Inherently unmatchable (not gaps to fix)

A tree-sitter extractor reading one `.c` file cannot reproduce nodes that
clang *synthesises* during semantic analysis, so exact `count`-per-kind parity
is not a goal (distinct-kind queries 05/06 are the parity bar instead):

- Implicit conversions: `@array_to_pointer` (8), glvalue→prvalue, implicit
  `@c_style_cast`, aggregate-literal expansion of `{1,2}` initialisers.
- Synthetic cleanup/end-of-scope labels and blocks.
- libc hardening rewrites: `sprintf`/`printf` → `__builtin___*_chk`, which
  reorder/drop the format-literal argument (so format strings are not a stable
  `valuetext` signal — `SecurityPatterns.c` uses a non-literal format to avoid
  this).
