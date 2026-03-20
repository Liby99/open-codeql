# open-cql Documentation

## Design Documents

- [**Project Design**](design/README.md) — Architecture overview, crate structure, component design, implementation phases
- [**IR Pipeline**](design/ir-pipeline.md) — Detailed specification of each intermediate representation (AST, HIR, MIR, LIR, Query Plan)
- [**Datalog+Flow Engine**](design/datalog-flow-engine.md) — Evaluation engine design, flow tracking, provenance, algorithms
- [**Extractors**](design/extractors.md) — C/C++ and Java extractor design, database schema, tree-sitter integration
- [**QL Grammar**](design/ql-grammar.md) — Working grammar specification for the QL language parser

## Crawled Reference Documentation

### CodeQL Overview
- [About CodeQL](crawled/codeql-overview/about-codeql.md)
- [Glossary](crawled/codeql-overview/glossary.md)

### QL Language Reference
- [QL Language Reference](crawled/ql-language-reference/README.md) — Consolidated reference

### Language-Specific
- [C/C++ Library and Analysis](crawled/languages/cpp/README.md)
- [Java/Kotlin Library and Analysis](crawled/languages/java/README.md)

## Crawl Scripts

- [scripts/crawl_codeql_docs.py](../scripts/crawl_codeql_docs.py) — Crawls all CodeQL documentation pages
  - Run: `python3 scripts/crawl_codeql_docs.py`
  - Output: `docs/crawled/` directory with organized markdown files
  - Creates `docs/crawled/manifest.json` listing all downloaded pages
