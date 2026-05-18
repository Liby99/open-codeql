# open-codeql Specification

This directory holds the **layered specifications** for open-codeql. Each layer
specifies a single seam in the system, and each implementation crate is
expected to conform to exactly one layer.

The specs are written so that:

1. A faithful CodeQL substitute can be built against them (Goal 1).
2. Neurosymbolic extensions can be slotted into named, well-typed seams without
   changing higher or lower layers (Goal 2).

Earlier, looser design notes have been moved to [`../legacy/`](../legacy/).
They remain useful as background reading but are no longer load-bearing.

## Layers

| Layer | Spec | Implementation crates |
|-------|------|----------------------|
| **L0** — Concrete Syntax (delta from QL) | [`L0-concrete-syntax.md`](L0-concrete-syntax.md) | `ocql-ql-parser` |
| **L1** — Abstract Syntax + Static Semantics | [`L1-static-semantics.md`](L1-static-semantics.md) | `ocql-ql-ast`, `ocql-hir` |
| **L2** — Core QL (MIR) | [`L2-core-ql.md`](L2-core-ql.md) | `ocql-mir`, `ocql-lir` |
| **L3** — Evaluation Semantics | [`L3-evaluation.md`](L3-evaluation.md) | `ocql-engine` |
| **L4** — Database & Schema Contract | [`L4-database.md`](L4-database.md) | `ocql-database`, `ocql-schema` |
| **L4-X** — Extractor Contracts | [`extractors/`](extractors/) | `ocql-extractor-*` |
| **L5** — Standard Library (oqlpack) | [`L5-stdlib.md`](L5-stdlib.md) | `oqlpack/` |
| **C** — Conformance | [`conformance.md`](conformance.md) | `ocql-e2e-tests` |

## Reading order

- Implementers and reviewers read **top-down**: a change to L1 may invalidate
  L2 lowering rules; a change to L4 may invalidate every extractor.
- New contributors read **bottom-up**: L4 and L4-X explain the data model that
  the rest of the pipeline manipulates.
- For neurosymbolic research, the principal seam is **L3**, with secondary
  seams at **L1 type checking** and **L5 query authoring**.

## Versioning

Each spec carries a `Version` and a `Status` line in its header.

- `Status: Draft` — the spec is being written.
- `Status: Ratified` — the spec is the binding contract; deviations in code
  are bugs.
- `Status: Provisional` — the spec describes a future contract; the current
  code may not yet satisfy it. Provisional sections are clearly marked.

## Conformance

A change to a Ratified spec must come with one of:

- a code change that re-establishes conformance, **or**
- a Conformance Suite (see [`conformance.md`](conformance.md)) update that
  records the deviation as an accepted divergence from upstream CodeQL.

## Out of scope

The specs deliberately do **not** prescribe:

- File layout inside a crate.
- Choice of data structures (HashMap vs. SmallVec, etc.).
- Performance budgets — those live in benchmarks, not the spec.
- IDE / LSP behaviour — see `editors/` and the LSP crate's own docs.
