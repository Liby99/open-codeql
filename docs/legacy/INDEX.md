# Legacy design notes

These documents are the **pre-spec** design notes for open-codeql. They
were written incrementally as the project grew in March–May 2026 and
have been superseded by the layered specs in `../spec/`.

They are kept for two reasons:

1. **Rationale archaeology.** Many decisions in the current code are
   easier to understand against the original design intent than against
   the present spec text.
2. **Salvageable detail.** Some sections (e.g. specific lowering rules,
   schema fragments, examples) are still accurate and may be lifted
   wholesale into the spec set as it matures.

If you find yourself relying on a legacy doc to make a decision, that's
a signal to fold the relevant content into the corresponding spec
section and link to it from there.

## Mapping

| Legacy file | Superseded by |
|---|---|
| `README.md` (top-level project design) | `../spec/README.md` plus all L-specs |
| `ir-pipeline.md` | `../spec/L1-static-semantics.md`, `L2-core-ql.md`, `L3-evaluation.md` |
| `hir-design.md` | `../spec/L1-static-semantics.md` |
| `mir-specification.md` | `../spec/L2-core-ql.md` |
| `datalog-flow-engine.md` | `../spec/L3-evaluation.md` |
| `dbscheme-reference.md` | `../spec/L4-database.md` |
| `extractors.md` | `../spec/extractors/README.md` |
| `ql-grammar.md` | `../spec/L0-concrete-syntax.md` |
| `implementation-plan.md` | `../spec/conformance.md` (status tracking) |
| `parallel-work-plan.md` | (no successor; obsolete planning doc) |
| `qlpack-compilation-gaps.md` | `../spec/L5-stdlib.md`, `../spec/conformance.md` |
| `schema-alignment-report.md` | `../spec/extractors/java.md` (and future per-language docs) |

## Status

Do not edit these files. If a legacy doc needs correction, either:

- update the corresponding spec section instead, or
- add a note at the top of the legacy file pointing at the spec
  section that contains the corrected text.
