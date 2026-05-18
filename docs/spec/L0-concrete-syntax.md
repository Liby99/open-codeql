# L0 — Concrete Syntax (Delta from CodeQL QL)

- **Layer:** L0
- **Implementation crate:** `ocql-ql-parser`
- **Status:** Ratified for the listed deviations; Provisional for the rest of the grammar (the LALRPOP grammar is the de-facto reference).
- **Version:** 0.1

## 1. Purpose

L0 specifies the **concrete syntax accepted by `ocql-ql-parser`** as a
**delta from upstream CodeQL QL**. It exists so that the syntax compromises
made for parser ergonomics are documented in one place and cannot drift
silently.

A `.ql` or `.qll` file written for upstream CodeQL is *not* guaranteed to
parse under L0. A file that conforms to L0 is *not* guaranteed to parse under
upstream CodeQL. The expected workflow is:

```
upstream .ql/.qll  ──[scripts/turbofish_transform.py]──▶  L0-conformant text
```

## 2. Reference grammar

The binding grammar is `crates/ocql-ql-parser/src/ql.lalrpop`. The lexer is
`crates/ocql-ql-parser/src/lexer.rs`. Both are referenced from this document
by file path; this document does not duplicate them production-by-production.

The grammar covers (non-exhaustive):

- Source files, imports (with aliases), modules, parameterized modules,
  predicates (with optional result type), classes (with `extends`,
  `instanceof`, characteristic predicate, fields, member predicates),
  newtypes (with OR branches), select queries (`from … where … select …
  [order by …]`).
- All 14 declaration annotations: `abstract`, `cached`, `external`,
  `extensible`, `final`, `transient`, `override`, `default`, `deprecated`,
  `query`, `additional`, `private`, `library`, `signature`, plus
  `pragma[…]`, `bindingset[…]`, and `overlay[…]` variants.
- Formulas: `implies`, `or`, `and`, `not`, `if … then … else`, comparisons,
  `instanceof`, range `in`, `exists` (three forms), `forall`, `forex`,
  `any()`, `none()`.
- Expressions: int/float/string/bool literals, `this`, `result`, `super`,
  don't-care `_`, member calls, type calls, qualified calls, ranges, set
  literals, all 11 aggregations (`count`, `sum`, `min`, `max`, `avg`,
  `concat`, `unique`, `rank`, `any`, plus the `strict*` variants).

## 3. Deviations from upstream QL

Each deviation is listed with **rationale**, the **mechanical transform** if
any, and the **detection rule** a tool can use to translate upstream QL into
L0.

### 3.1 Turbofish for parameterized type/module application — **D1**

| | |
|---|---|
| Upstream | `Foo<T>(args)`, `Module<Cfg>::pred(args)` |
| L0 | `Foo::<T>(args)`, `Module::<Cfg>::pred(args)` |
| Rationale | Eliminates LALR(1) ambiguity between `<` as type-argument bracket and `<` as comparison. |
| Transform | `scripts/turbofish_transform.py` |
| Detection | `UpperIdent '<'` where the matched `>` is followed by `(` or `::`. |

### 3.2 Qualified-chain type arguments — **D2**

The current preprocessor (`strip_qual_chain_type_args` in
`crates/ocql-ql-parser/src/lib.rs:111`) **erases** type arguments on
non-leading segments of a qualified chain:

```
Foo<A>::bar     ──▶  Foo::bar
```

This is a **syntactic erasure**, not a semantic one. L1 currently has no
notion of the discarded arguments either; see L1 §2.4 for the consequences.

> **Provisional intent:** when L1 grows real module instantiation, this
> deviation should be promoted to a semantic one (the type args carried
> through to HIR), not removed.

### 3.3 Nested angle-bracket type arguments — **D3**

The grammar accepts a single level of `<…>` in module expressions and
signature parameters. Nested `<…>` (e.g. `Foo<Bar<Baz>>`) is flattened to
spaces by `flatten_nested_type_args`
(`crates/ocql-ql-parser/src/lib.rs:218`). The outer `<>` is preserved.

> **Provisional intent:** lift this restriction in the grammar before L1
> instantiation lands; the current behaviour silently discards inner type
> args.

### 3.4 File-level `module;` declarations and overlay annotations — **D4**

| | |
|---|---|
| Upstream | `overlay[local?]\nmodule;` at the top of a `.qll` |
| L0 | Stripped during preprocessing (treated as if absent) |
| Implementation | `strip_file_module_decl` in `crates/ocql-ql-parser/src/lib.rs:308` |
| Rationale | Every file is already a module; `overlay[…]` semantics are not yet implemented. |

> **Provisional intent:** when overlay support is added, this preprocessor
> step is removed and overlay annotations become a parsed declaration.

### 3.5 Lowercase module aliases — **D5**

```
import javascript as js
js::Foo            // upstream, ok
Js::Foo            // L0 grammar requires UpperIdent prefix
```

The preprocessor (`capitalize_lowercase_qual_prefix` in
`crates/ocql-ql-parser/src/lib.rs:34`) capitalises the first segment of any
`ident::…` chain. This is **observable in error messages and diagnostics**:
spans are preserved, but the identifier text reported is the capitalised
form.

### 3.6 Keywords used as class names — **D6**

The keywords `import`, `module`, `select`, `this` are reserved in the lexer
but accepted as type names in upstream QL after `class`, `extends`, or
`instanceof`. The preprocessor (`capitalize_keywords_as_classnames` in
`crates/ocql-ql-parser/src/lib.rs:343`) capitalises them in those contexts
only.

### 3.7 Block comments — **D7**

Block comments are stripped in preprocessing rather than handled by the
lexer (`strip_block_comments` in `crates/ocql-ql-parser/src/lib.rs:439`).
Spans are preserved by replacing comment bytes with spaces. This is an
implementation detail with no observable difference vs. upstream — it is
listed for completeness.

### 3.8 Predicate equations / higher-order predicate definitions — **D8**

Upstream form (sketch):

```
predicate name(...) = higherOrderPred(pred/arity, ...)(args)
```

The grammar parses this production but the AST node is not currently
populated (returns `None`; see `crates/ocql-ql-parser/src/ql.lalrpop` near
line 166). Files that use this form will parse but fail in L1.

> **Provisional intent:** finish the AST node and propagate to L1.

## 4. Lexical rules

The lexer is hand-written
(`crates/ocql-ql-parser/src/lexer.rs`). Notable rules:

- Identifiers are `[A-Za-z_][A-Za-z0-9_]*`; lower-case-leading and
  upper-case-leading identifiers are distinguished by the lexer to drive
  grammar rules (`lower_ident` vs. `upper_ident`).
- String literals are double-quoted with `\"`, `\\`, `\n`, `\t`, `\r` escapes.
- Numeric literals: integer (`-?[0-9]+`) and float (`-?[0-9]+\.[0-9]+`),
  the leading `-` is parsed as part of the literal in some contexts and as
  unary `Neg` in others — the binding behaviour is whatever the LALRPOP
  grammar yields.
- All preprocessor passes are applied **before** lexing.

## 5. Pragmatics

L0 is **lossy with respect to upstream QL spans**: the preprocessor replaces
text with spaces of equal length so byte offsets are preserved, but tokens
that were elided (e.g. `<Args>` in §3.2) are absent from the AST. Tools that
need to round-trip should preserve the *original* source alongside the
parse, not reconstruct it from the AST.

## 6. Conformance tests

A file `crates/ocql-ql-parser/tests/syntax_deviations/` (Provisional — to be
created) should hold one input pair per deviation:

```
syntax_deviations/
  d1_turbofish/         upstream.ql      l0.ql
  d2_qual_chain_args/   upstream.ql      l0.ql
  …
```

Each pair documents the deviation by example; the test asserts that the
preprocessor maps `upstream.ql` to a string that parses identically to
`l0.ql`.

## 7. Open questions

- **D2/D3 promotion**: when L1 supports module instantiation, do we keep
  the `Foo::<T>` turbofish (D1) but recover `<T>` on intermediate segments?
- **Predicate equations (D8)**: what is the minimal subset we want to
  support before we re-enable the production in the AST?
- **Overlay annotations (D4)**: what is their intended runtime semantics in
  open-codeql, and do we model them at L0 or L2?

These questions block promoting this spec from `Ratified for listed
deviations` to fully `Ratified`.
