# L5 — Standard Library (oqlpack)

- **Layer:** L5
- **Implementation directory:** `oqlpack/`
- **Status:** Provisional. Java is sketched (~13 files); other languages are unstarted.
- **Version:** 0.1

## 1. Purpose

L5 is the **QL-level standard library** that turns the relational tables
of L4 into the object-oriented predicate vocabulary that CodeQL queries
expect (classes like `Method`, `MethodAccess`, `Type`, `DataFlow::Node`,
etc.).

L5 has two goals:

1. **CodeQL substitution.** A user's existing CodeQL query, after L0
   syntax transformation, should compile and run against `oqlpack/`.
2. **Neurosymbolic extension surface.** The library is the natural place
   to expose neural augmentations as ordinary QL predicates (e.g.
   `LLMNamedSink`, `LearnedTaintStep`).

## 2. Layout

```
oqlpack/
   <lang>/
      qlpack.yml              // Provisional — currently absent
      lib/
         <lang>.qll           // top-level reexport
         semmle/
            code/
               FileSystem.qll
               Location.qll
               Unit.qll
               <lang>/
                  Element.qll
                  Type.qll
                  ...
      config/
         semmlecode.dbscheme  // mirrors L4 schema for that language
```

Today only `oqlpack/java/` exists, with the `lib/semmle/code/{,java/}`
hierarchy and a `config/semmlecode.dbscheme`.

## 3. Layering with respect to upstream CodeQL

**L5 mirrors the file structure of upstream `codeql/<lang>/ql/lib/`** so
that a user can move a query between systems with minimal churn.
Differences from upstream are scoped to:

- The L0 syntax compromises (turbofish, etc.) — applied to the entire
  `oqlpack/` tree.
- L1 features still missing (parameterized modules, signatures) — when
  they land, L5 modules are migrated *back* toward upstream shape.
- Engine features still missing (flow extension) — until §5 of L3 ships,
  the dataflow-related modules in L5 are stubs that use the plain
  Datalog engine and document the precision deficit.

## 4. qlpack manifest

Each language directory holds a `qlpack.yml` (Provisional — file does not
yet exist):

```yaml
name: open-codeql/java
version: 0.1.0
library: true
extractor: java
upgrades: upgrades/                  # Provisional
default-suite-file: codeql-suites/code-scanning.qls
dependencies: {}
```

The manifest is parsed by an L5 loader (TBD) that resolves imports, picks
an extractor, and ties together the L4 schema with the QL library.

## 5. Module conventions

### 5.1 Element classes

Every language has an `Element` class that sits at the root of its AST
class hierarchy. Member predicates of `Element` should at minimum
include:

- `string toString()`
- `Location getLocation()`
- `File getFile()`

### 5.2 Type / Expr / Stmt

Each language has three trees:

- A **type** hierarchy (`Type` → `PrimitiveType`, `RefType`, …).
- An **expression** hierarchy (`Expr` → `Literal`, `BinaryExpr`, …).
- A **statement** hierarchy (`Stmt` → `IfStmt`, `LoopStmt`, …).

Each leaf class is the characteristic predicate of a database tag (e.g.
`exprs(_, kind, _)` where `kind = 84` is `VarAccess`). The mapping from
tag to class is the concern of the language's extractor (L4-X).

### 5.3 Modifier / Annotation / Comment

Cross-cutting decorations live in their own modules and are never
embedded in the Element subtree.

### 5.4 DataFlow / TaintTracking

Until L3 §5 lands, `DataFlow.qll` and `TaintTracking.qll` are *minimal*
modules that:

- Define `Node`, `Configuration`, `flow(Node, Node)` predicates that
  evaluate to plain Datalog reachability.
- Carry a documented precision warning at the top of the file.
- Expose a stable signature so that user queries written today continue
  to work when the flow engine ships.

## 6. Naming conventions

L5 uses CodeQL's naming conventions verbatim (PascalCase classes,
camelCase predicates, `Module` for module names) so that user-written
queries port cleanly. The L0 turbofish compromise is the only deviation
inside `.qll` files.

## 7. Tests

- **L5 smoke tests** (`crates/ocql-e2e-tests/tests/oqlpack_java.rs`):
  every `.qll` analyzes without errors; every documented class has at
  least one populated extent on a fixture database.
- **L5 parity tests** (`crates/ocql-e2e-tests/tests/java_parity.rs`):
  for a fixed input fixture, classes like `Method`, `Field`, `Type`
  match the upstream CodeQL extraction in count and (for entity classes)
  in `toString()` shape.

When `vendor/codeql/` is checked in, parity tests should be elaborated
into a `codeql query run` harness that runs the same query under both
systems and diffs the results.

## 8. Versioning

- The qlpack version (`qlpack.yml`) is bumped per release of `oqlpack`.
- A bump in the L4 schema for a language forces a bump in that
  language's qlpack version.

## 9. Authoring guidelines

- A new class predicate should derive its class membership from
  characteristic predicates, not from an `instanceof` cast inside its
  body. (This makes L2 lowering predictable and L3 stratification
  cleaner.)
- A predicate that may grow large (e.g. transitive closure of a step
  relation) should be marked `cached` if it is reused.
- A predicate intended as a hot extension point for downstream queries
  should use a module-signature surface, even though L1 doesn't yet
  enforce it. This documents intent and unblocks signature-aware
  consumers later.

## 10. Open questions

- **Multi-language scope:** which languages get oqlpack content next
  (C/C++ first, given the test corpus, vs. JavaScript first, given user
  demand)?
- **Compatibility shim layer:** do we ship a shim that
  rewrites legacy CodeQL syntax to L0 at import time, or do we only
  accept pre-transformed `.qll` files?
- **Versioning:** semantic versioning per qlpack, or a single
  open-codeql version pinned across all qlpacks?
