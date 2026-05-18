# L1 — Abstract Syntax + Static Semantics

- **Layer:** L1
- **Implementation crates:** `ocql-ql-ast`, `ocql-hir`
- **Status:** Provisional. The current implementation realises ~⅔ of this spec; the remaining ⅓ is an instruction list for what to build.
- **Version:** 0.1

## 1. Purpose

L1 specifies what a syntactically valid L0 program **means before
evaluation**:

- Identifier resolution across imports and modules.
- The QL type system, including class hierarchy and characteristic-predicate
  semantics.
- Validation rules that reject programs which would otherwise lower
  ambiguously to L2.

L1 does **not** specify evaluation order or runtime values — those belong
to L2 / L3.

## 2. Namespaces

QL has six namespaces per module:

1. `module` — module names and module aliases.
2. `type` — class names, newtype names, primitive type aliases.
3. `predicate` — predicate names, keyed by `(name, arity)`.
4. `module-sig` — module signatures.
5. `type-sig` — type signatures.
6. `pred-sig` — predicate signatures.

### 2.1 Conformance gap

`ocql-hir/src/namespace.rs` currently implements **(1), (2), (3)** only.
Programs that depend on signatures (4–6) parse at L0 but fail to type-check
at L1.

> **Action:** a Provisional-section follow-up adds three signature-namespace
> tables and a verification pass that reports a missing-signature
> implementation as a typed diagnostic.

### 2.2 Resolution order

When resolving an identifier `x` from a use site `U`:

1. **Local scope** (parameters, quantifier-bound vars, `let`-binders) at
   `U`.
2. **Enclosing module(s)** of `U`, walking outward.
3. **Imports** of `U`'s file, in declaration order. An `import M as A`
   introduces `A` into the `module` namespace pointing at `M`.
4. **Project fallback**: any sibling library file. (See
   `lib.rs:354` for the implementation.) This step is a deliberate
   compromise vs. upstream QL, which has stricter visibility rules.

The first match wins; later matches are silently shadowed. A name that
resolves into more than one namespace is **not** an ambiguity error: which
namespace is selected is determined by the use-site syntax (e.g. a name in
type position binds in `type`).

### 2.3 Imports

`import path.to.Module` resolves a dotted path to a `.qll` file. The path
segments are interpreted as directory components rooted at the qlpack root
(see L5).

`import path.to.Module as Alias` adds `Alias` (uppercase or lowercase per
L0 §3.5) to the importing file's `module` namespace.

`import Alias::Member` is the **module-member import** and adds `Member`
into the importing file's namespaces of all types it inhabits in `Alias`.

### 2.4 Parameterized modules — Provisional

L1 currently parses parameterized modules but **does not instantiate**
them. A use site `Mod::<Cfg>` (in L0 form) is treated as `Mod` with the
type arguments discarded.

The intended semantics is monomorphization at L1:

- A signature parameter is a positional binding from an L1 signature name
  to a concrete name.
- Each distinct argument tuple yields a separately-named instantiation
  added to the `module` namespace as `Mod_<hash-of-args>`. Use sites refer
  to the instantiation, not the generic.
- The L1 → L2 lowering sees only instantiations and does not know that
  generics existed.

This is the principal blocker for running upstream CodeQL libraries
(`DataFlow::Make`, `TaintTracking::Make`, etc.) against open-codeql.

## 3. Type system

### 3.1 Types

```
PrimitiveType  ::= boolean | int | float | string | date
DatabaseType   ::= '@' lower_ident
ClassType      ::= UpperName ('::' UpperName)*
NewtypeType    ::= UpperName ('::' UpperName)*
UnionType      ::= ClassType ('|' ClassType)*           (in newtype branches)
SignatureType  ::= UpperName                              (provisional)
```

Database types `@x` are introduced by the schema (L4); class and newtype
types are introduced by user code (L1).

### 3.2 Class semantics

A class is a **predicate over `this`** plus a set of member predicates:

```
class C extends T1, T2 instanceof T3 {
   C() { /* characteristic predicate body */ }
   member-predicates …
}
```

The class membership predicate is the conjunction:

- the bodies of all `extends` parents' membership predicates,
- the body of the `instanceof` constraint,
- the explicit characteristic predicate body, if present.

A value is a member of the class iff this conjunction holds.

### 3.3 Class linearization

Multiple inheritance is resolved by a **C3 linearization** (the algorithm
used by Python and Dylan) computed once per class at L1.

> **Conformance gap:** the current implementation uses ad-hoc walk-and-merge
> rules; switching to C3 is straightforward and is the action item for
> §3.3.

### 3.4 Subtyping

Subtyping `A <: B` holds iff one of:

- `A` and `B` are the same type, **or**
- `A` is `extends`-reachable from `B` (transitively), **or**
- `A` is `instanceof`-restricted to a type that subtypes `B`.

The implicit subtype relation `int <: float` holds for arithmetic only;
it does not propagate to call sites that demand `float`.

### 3.5 Type checking

A program type-checks iff:

- Every variable use has a declared or inferred type compatible with all
  its constraints (parameter type, comparison operands, predicate
  arguments).
- Every predicate call resolves to exactly one predicate of matching
  arity.
- Every member call's receiver has a static type that defines the named
  member.
- Every `instanceof` operand's type is compatible with the right-hand
  side.
- No predicate has a recursive cycle that crosses negation in an
  unstratifiable way (the cycle check moves to L2; here we only require
  syntactic well-formedness).

### 3.6 Diagnostics

Diagnostics are values of `Diagnostic { span, severity, code, message }`
where:

- `severity ∈ { error, warning, info }`,
- `code` is a stable string of the form `ocql-hir-NNNN`,
- `span` points into the **L0 source** (after preprocessing). Tools that
  need original-source spans must keep their own map.

A program with any `error`-severity diagnostic does **not** lower to L2.

## 4. Validation rules (non-exhaustive)

| ID | Rule |
|----|------|
| `ocql-hir-0001` | Predicate of arity *n* called with *m* ≠ *n* arguments. |
| `ocql-hir-0002` | Class extends itself directly or transitively. |
| `ocql-hir-0003` | Member access on a receiver whose type does not define it. |
| `ocql-hir-0004` | Comparison between values of incompatible types. |
| `ocql-hir-0005` | Variable used outside its binding scope. |
| `ocql-hir-0006` | Newtype branch overlaps another branch on its database type. |
| `ocql-hir-0007` | Import path does not resolve to a `.qll` file. |
| `ocql-hir-0008` | Identifier resolves in a namespace inconsistent with use-site syntax. |
| `ocql-hir-0009` *(Provisional)* | Module signature parameter has no matching implementation. |

## 5. AST stability

The AST in `ocql-ql-ast` is **load-bearing for L1 but not for L2**:
breaking changes to AST node shapes are allowed without a major version
bump as long as the L1 contract (the diagnostics, the resolved-name tables,
the typed-expression tables) is preserved.

This is deliberate. The AST is an internal serialization format between
the parser and HIR; users who want a stable representation should target
L2 (MIR) or L3 (engine rules).

## 6. Conformance tests

`crates/ocql-hir` ships with `analyze_single_file` and `analyze_project`
entry points. The test harness in `crates/ocql-e2e-tests` should grow:

- One test per validation rule (`ocql-hir-NNNN`) demonstrating the rule
  fires on a minimal failing input and does not fire on a minimal passing
  input.
- A "QL stdlib smoke" test: every `.qll` in `oqlpack/` analyzes without
  errors (warnings are allowed).
- For each Provisional gap (signature namespaces, parameterized modules,
  C3 linearization), a `should_fail` test that pins current behaviour and
  is converted to a positive test when the gap closes.

## 7. Open questions

- **Implicit `int <: float` (§3.4):** should it be transitive across
  predicate signatures, or only at arithmetic operators?
- **Project fallback (§2.2 step 4):** is this a permanent design decision
  or a transitional crutch for the missing signature-namespace work?
- **Diagnostic codes:** should `ocql-hir-NNNN` be flat or hierarchical
  (e.g. `ocql-hir-resolve-0001`)? Flat is simpler; hierarchical scales
  better.
