# L4-X — Extractor Contracts

- **Layer:** L4-X (a refinement of L4 per language)
- **Implementation crates:** `ocql-extractor-common`, `ocql-extractor-<lang>`
- **Status:** This file is the shared template. Per-language specs live alongside it.
- **Version:** 0.1

## 1. Purpose

L4-X specifies the contract between a **language extractor** and the rest
of the system:

- What relations the extractor populates (its **emitted schema**).
- What invariants those relations satisfy.
- What semantic depth the extractor commits to (AST? bindings? CFG?).

Each language has its own L4-X document. This README is the template they
share.

## 2. Maturity tiers

Every extractor self-declares its maturity tier. Higher tiers strictly
include lower-tier guarantees.

| Tier | Name | Guarantees |
|------|------|------------|
| **a** | Skeleton | Source files parse with tree-sitter. `files`, `locations`, basic node tables populated. |
| **b** | Basic AST | Every AST node in the language reference grammar has an entry in the corresponding declaration / statement / expression table. Parent / child links present. |
| **c** | AST + Bindings | (b) + name resolution: variable references point to their declarations; method calls point to their callees (when statically resolvable). Type information stored where syntactically determinable. |
| **d** | AST + Bindings + CFG | (c) + control-flow graph relations populated; method-overload resolution; type hierarchy. |
| **e** | Full schema | (d) + every relation in the upstream `<lang>.dbscheme` has matching extent. |

Tiers a–c can be reached purely with tree-sitter. Tier d typically needs
a real frontend (clang / javac / etc.) or a hand-written semantic
analyzer. Tier e is "do what upstream does."

## 3. Schema as contract

The schema is the binding contract. An extractor is conformant iff:

1. **Every** relation declared in its schema is populated (an empty
   relation is fine; a missing relation is a bug).
2. **No** relation outside its schema is populated.
3. Every emitted tuple respects column types (L4 §3) and ref/unique
   constraints (L4 §6).
4. Every `@`-entity id is owned by exactly one relation (the "key"
   relation for that entity type).
5. The extractor is **deterministic** for a fixed input: byte-for-byte
   identical databases for byte-for-byte identical inputs.

A change to the schema is a breaking change to the language's L5 qlpack
and requires a re-extraction of any database in flight.

## 4. Standard relation conventions

Every L4-X schema must declare at least:

```
files(unique int id: @file, string name: string ref)
folders(unique int id: @folder, string name: string ref)
@container = @file | @folder
containerparent(int parent: @container ref, unique int child: @container ref)
sourceLocationPrefix(string prefix: string ref)
locations_default(...)
@location = @location_default
hasLocation(int locatableid: @locatable ref, int id: @location ref)
@sourceline = @locatable
numlines(int element_id: @sourceline ref, int num_lines: int ref,
         int num_code: int ref, int num_comment: int ref)
```

These tables are the universal "how do I get a file/line/column?"
substrate. They are populated by code in `ocql-extractor-common`, not
the language extractor itself; the language extractor is responsible for
emitting `@locatable` ids that point into them.

## 5. Determinism rules

- File processing order may not affect the output (no implicit ids
  derived from arrival order; ids derived from `(file, span)`).
- Concurrent extraction of distinct files is allowed; the merge step
  must be order-independent.
- Tuple insertion order may not affect persistent ids.

A test that verifies determinism by extracting twice and diffing
databases is part of the language's L4-X conformance suite.

## 6. Tree-sitter integration

The extractor framework
(`ocql-extractor-common::tree_sitter_utils`) provides:

- A walker that yields `(node, depth, parent)` triples.
- Node-kind dispatch by string match (constant in the binary).
- A `LocationEmitter` that assigns ids to spans and emits
  `locations_default` rows.

A language extractor is typically:

```
file → tree_sitter_<lang>::parse
     → walker
     → match node.kind { ... emit relations ... }
```

The grammar version (a tree-sitter version + a tree-sitter-<lang>
version) is part of the extractor's L4-X spec.

## 7. Symbol binding (Tier c)

A Tier c extractor must implement, at minimum:

- **Variable resolution**: a `localvariableref(varuse, vardecl)` (or
  language equivalent) relation linking each use to its decl.
- **Call resolution**: a `callee(call_id, target_id)` relation linking
  each call to one or more callees.

Calls that cannot be resolved statically (virtual dispatch, dynamic
loading) emit no `callee` rows; downstream queries are expected to fall
back to flow-based resolution.

## 8. CFG (Tier d)

A Tier d extractor must populate a `cfg_edge(src, dst, kind)`-shaped
relation. The exact column shape is the language's choice, but the
semantics is:

- Every executable statement / expression has at least one outgoing
  edge unless it is a terminator (`return`, `throw`, infinite loop).
- Edges have a `kind` (sequential, branch-true, branch-false, exception,
  return).

## 9. Per-language docs

| Language | Spec | Tier (current) |
|----------|------|----------------|
| Java     | [`java.md`](java.md) | d |
| C/C++    | _(TBD)_ | c |
| Python   | _(TBD)_ | b |
| C#       | _(TBD)_ | b |
| Go       | _(TBD)_ | b |
| JavaScript / TypeScript | _(TBD)_ | b |
| Rust     | _(TBD)_ | b |
| Swift    | _(TBD)_ | b |
| Ruby     | _(TBD)_ | a |

A new language extractor is added by:

1. Creating `crates/ocql-extractor-<lang>` (mirrors an existing one).
2. Drafting `docs/spec/extractors/<lang>.md` from this template.
3. Wiring it into `crates/ocodeql/src/commands/database.rs`.
4. Adding a smoke test in `crates/ocql-e2e-tests/`.

## 10. Open questions

- **Trap-style ingestion**: upstream CodeQL emits TRAP files and ingests
  them in a separate pass. Should we adopt that to enable parallel
  extraction at scale, or stay with direct in-memory inserts?
- **Parser pluralism**: per-language we currently pick one parser
  (tree-sitter for most, javap-style bytecode for Java). Do we ever
  want a fallback parser per language?
- **Extractor versioning**: should each L4-X have its own version
  number, or is it pinned to the L4 format version?
