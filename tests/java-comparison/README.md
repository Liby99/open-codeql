# Java Comparison Test Suite

Compare open-codeql (ocodeql) against CodeQL for Java analysis.

## Test Structure

```
tests/java-comparison/
├── projects/              # Java source files for testing
│   ├── BasicStructure.java   # Classes, interfaces, enums, inheritance, modifiers
│   ├── ControlFlow.java      # All statement types, exception handling, loops
│   ├── CallGraph.java        # Method calls, virtual dispatch, constructor chains
│   └── SecurityPatterns.java # Known vulnerable patterns (CWE examples)
├── queries/               # Raw-table QL queries (work with both tools)
│   ├── 01_count_classes.ql through 15_string_literals.ql
│   └── (15 comparison queries)
├── run_comparison.sh      # Script to run both codeql + ocodeql and diff results
├── results/               # (generated) comparison output
└── README.md
```

## Rust E2E Tests

```bash
# Run all Java parity tests (19 tests)
cargo test -p ocql-e2e-tests --test java_parity -- --nocapture

# Run all Java tests together (81 tests)
cargo test -p ocql-e2e-tests --test e2e_tests --test oqlpack_java --test java_parity
```

## Docker Comparison

```bash
# Inside Docker container (has both codeql and ocodeql):
cd /workspace/tests/java-comparison
./run_comparison.sh                     # All projects x all queries
./run_comparison.sh BasicStructure      # One project, all queries
./run_comparison.sh BasicStructure 01   # One project, one query
```

## Known Gaps (ocodeql vs codeql)

### P0 — Foundation
| Gap | Status | Impact |
|-----|--------|--------|
| Implicit default constructors | IMPLEMENTED | Classes without explicit constructors get synthetic default |
| Java call binding (callableBinding) | IMPLEMENTED | Method call → target resolution (simple name matching) |
| Variable binding (variableBinding) | IMPLEMENTED | Variable access → declaration (name-based) |

### P1 — Core Analysis
| Gap | Status | Impact |
|-----|--------|--------|
| Virtual dispatch resolution | NOT IMPLEMENTED | polyCalls, dynamic dispatch |
| Type resolution (generics) | PARTIAL | typeVars/typeBounds extracted, but not fully resolved |
| Override detection | NOT IMPLEMENTED | Which methods override which |
| Scope-aware variable binding | NOT IMPLEMENTED | Current binding is file-global, not scope-aware |

### P2 — Query Coverage
| Gap | Status | Impact |
|-----|--------|--------|
| Likely Bugs queries (116 vendor) | 0 IMPLEMENTED | Pattern-based bug detection |
| Best Practice queries (61 vendor) | 0 IMPLEMENTED | Code quality checks |
| Oqlpack class library completeness | 13/470 .qll files | Missing most QL library predicates |

### P3 — Advanced Analysis
| Gap | Status | Impact |
|-----|--------|--------|
| Data flow / taint tracking | NOT IMPLEMENTED | Core security analysis |
| Control flow graph | NOT IMPLEMENTED | Precise analysis |
| Framework models | 0/70+ | Spring, Android, etc. |

## Current Test Coverage

- **Extractor unit tests**: 17 (crates/ocql-extractor-java)
- **E2E Java tests**: 34 (crates/ocql-e2e-tests/tests/e2e_tests.rs)
- **Oqlpack tests**: 28 (crates/ocql-e2e-tests/tests/oqlpack_java.rs)
- **Parity tests**: 21 (crates/ocql-e2e-tests/tests/java_parity.rs)
- **Real project tests**: 5 (crates/ocql-extractor-java, #[ignore])
- **Total**: 105 Java-related tests
