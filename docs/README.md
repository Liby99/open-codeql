# open-codeql Documentation

## Specifications (binding)

The implementation is organized into **layers**, each with its own spec.
See [`spec/README.md`](spec/README.md) for the index.

- [L0 — Concrete Syntax (delta from QL)](spec/L0-concrete-syntax.md)
- [L1 — Abstract Syntax + Static Semantics](spec/L1-static-semantics.md)
- [L2 — Core QL (MIR)](spec/L2-core-ql.md)
- [L3 — Evaluation Semantics](spec/L3-evaluation.md)
- [L4 — Database & Schema Contract](spec/L4-database.md)
- [L4-X — Extractor Contracts](spec/extractors/README.md)
  - [Java](spec/extractors/java.md)
- [L5 — Standard Library (oqlpack)](spec/L5-stdlib.md)
- [Conformance](spec/conformance.md)

## Legacy design notes

The earlier, less-structured design notes are kept under
[`legacy/`](legacy/) for historical reference. They are no longer
load-bearing; treat any conflict between a legacy doc and a current spec
as resolved in favour of the spec.

- [legacy/README.md](legacy/README.md) — overall architecture (superseded by spec/)
- [legacy/ir-pipeline.md](legacy/ir-pipeline.md) — superseded by spec/L1, L2, L3
- [legacy/datalog-flow-engine.md](legacy/datalog-flow-engine.md) — superseded by spec/L3
- [legacy/extractors.md](legacy/extractors.md) — superseded by spec/extractors/
- [legacy/dbscheme-reference.md](legacy/dbscheme-reference.md) — superseded by spec/L4
- [legacy/hir-design.md](legacy/hir-design.md) — superseded by spec/L1
- [legacy/mir-specification.md](legacy/mir-specification.md) — superseded by spec/L2
- [legacy/ql-grammar.md](legacy/ql-grammar.md) — superseded by spec/L0
- [legacy/implementation-plan.md](legacy/implementation-plan.md), [legacy/parallel-work-plan.md](legacy/parallel-work-plan.md), [legacy/qlpack-compilation-gaps.md](legacy/qlpack-compilation-gaps.md), [legacy/schema-alignment-report.md](legacy/schema-alignment-report.md) — historical

## Crawled reference documentation

Crawled CodeQL upstream docs (used for cross-checking spec text):

- [About CodeQL](crawled/codeql-overview/about-codeql.md)
- [Glossary](crawled/codeql-overview/glossary.md)
- [QL Language Reference](crawled/ql-language-reference/README.md)
- [C/C++ Library and Analysis](crawled/languages/cpp/README.md)
- [Java/Kotlin Library and Analysis](crawled/languages/java/README.md)

## Crawl scripts

- [scripts/crawl_codeql_docs.py](../scripts/crawl_codeql_docs.py) — crawls all CodeQL documentation pages
  - Run: `python3 scripts/crawl_codeql_docs.py`
  - Output: `docs/crawled/` directory with organized markdown files

## Other docs

- [todo.md](todo.md) — short list of pending work
