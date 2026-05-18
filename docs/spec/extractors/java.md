# L4-X Java

- **Layer:** L4-X
- **Implementation crate:** `ocql-extractor-java`
- **Status:** Active. Tier d (AST + bindings + CFG-light + JDK bytecode).
- **Version:** 0.1

This is the worked example for L4-X. Other languages should mirror its
structure.

## 1. Maturity tier

**Tier d** (per `extractors/README.md` §2). Realised by:

- AST extraction via `tree-sitter-java` (`extract.rs`).
- Symbol binding via `resolve_bindings` (post-pass; `extract.rs`).
- JDK bytecode extraction via `bytecode.rs` and `bytecode_extract.rs`,
  triggered when `JAVA_HOME` is found (`jdk.rs`).

## 2. Schema

The binding schema is the const string in
`crates/ocql-extractor-java/src/schema.rs:11`. It mirrors a subset of
upstream `semmlecode.dbscheme` and is the source of truth for both this
crate and `oqlpack/java/lib/config/semmlecode.dbscheme` — the two
**must** match.

The schema declares (non-exhaustive list — full schema in `schema.rs`):

### 2.1 Containers and locations

`files`, `folders`, `containerparent`, `sourceLocationPrefix`,
`locations_default`, `hasLocation`, `numlines`. Standard per L4-X §4.

### 2.2 Packages and types

```
packages(@package, nodeName)
cupackage(@file ref, @package ref)
primitives(@primitive, nodeName)
classes_or_interfaces(@classorinterface, nodeName, @package, @classorinterface)
isInterface, isRecord, isEnumType, isAnnotType
```

### 2.3 Members

```
methods(@method, name, signature, @type, @reftype, source)
constrs(@constructor, ...)
fields(@field, ...)
params(@param, position, @type, @callable, source)
```

### 2.4 Statements and expressions

`stmts`, `exprs`, `stmt_parents`, `expr_parents`, `localvariables`,
`namestrings`. Statement kinds and expression kinds use integer tags
declared in the schema as casetypes.

### 2.5 Modifiers and annotations

`modifiers`, `hasModifier`, `annotations`, `annotValue`.

### 2.6 Bindings

`callableBinding(@call, @callable)` is populated by `resolve_bindings`
after the JDK bytecode pass. Variable binding is via
`localvariablebinding(@varaccess, @localvar)`.

## 3. Tree-sitter version

- `tree-sitter` ≥ 0.20.
- `tree-sitter-java` pinned in `Cargo.toml` of the crate.

A bump of either of these is a Tier d-affecting change because node
kind names may shift.

## 4. JDK bytecode

`ocql_extractor_java::jdk::find_java_home` discovers `JAVA_HOME`. If
present, `extract_jdk(db, java_home)` walks `rt.jar` (or the Java 9+
`.jmod` files) and emits:

- `classes_or_interfaces` rows for every public class.
- `methods` / `fields` rows for every member.
- `methodOverrides` rows for the JDK class hierarchy.

The bytecode parser is hand-rolled in `bytecode.rs` (≈1 KLOC) and
supports class file major versions up to a documented cap (currently
Java 21).

## 5. Determinism

- Source files are extracted in sorted path order.
- Within a file, ids are assigned by walk order (deterministic given
  tree-sitter's deterministic parse).
- The JDK pass is deterministic given a fixed JDK installation.

A re-extraction yields a byte-identical database; a different `JAVA_HOME`
yields a different database (this is recorded as a deviation —
"databases are JDK-version-sensitive").

## 6. Conformance gaps

- **Generics:** type arguments on parameterized types are stored as
  strings, not entity refs. A real extension would introduce an
  `@generic_instantiation` entity type.
- **Lambdas / method references:** stored as ordinary `MethodAccess`
  with synthetic targets. Upstream CodeQL has dedicated tables for
  these.
- **Modules (Java 9+):** not extracted; the `module-info.java` file is
  ignored.
- **Annotations on type uses:** stored only on declarations.
- **Switch expressions / pattern matching (Java 14+):** lowered to the
  closest classic-switch shape; `instanceof` patterns are not modelled
  as their own kind.

These are recorded individually as L4-X-Java-NN entries in
`docs/spec/deviations/` (Provisional).

## 7. CFG

`stmts` and `exprs` carry parent links (`stmtparents`, `exprparents`)
sufficient for syntactic data-flow queries (the existing `examples/
dataflow.dl`). A first-class `cfg_edge` relation is **Provisional** —
the existing schema does not yet declare one. This is the single
biggest gap between this crate and Tier d as defined in
`extractors/README.md` §8.

## 8. Test surface

- `crates/ocql-e2e-tests/tests/e2e_tests.rs` — Java extraction smoke
  tests.
- `crates/ocql-e2e-tests/tests/java_parity.rs` — comparison against
  upstream-extracted databases (currently using snapshot fixtures;
  switches to live `codeql` invocation when `vendor/codeql/` is
  present).
- `crates/ocql-e2e-tests/tests/oqlpack_java.rs` — exercises the
  `oqlpack/java/` library against extracted databases.

## 9. CLI integration

```
ocodeql database create --language java --source <dir> --output <db>
```

The CLI also opportunistically runs the JDK bytecode pass and the
binding resolver after primary extraction (`commands/database.rs:90-110`).

## 10. Open questions

- **CFG materialization (§7):** add a dedicated `cfg_edge` relation in
  the schema, or compute CFG edges from `stmts`/`exprs` at query time?
- **JDK pinning:** ship a baked-in JDK summary so databases are not
  JDK-version-sensitive?
- **Kotlin:** kotlin shares the JVM and much of the QL library shape.
  Is a `ocql-extractor-kotlin` crate planned, and does it share this
  schema?
