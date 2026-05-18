# HIR (High-Level IR) Design Document

This document describes the design and implementation plan for `ocql-hir`, the
high-level intermediate representation for the open-cql QL compiler (Track B).

## 1. Role of the HIR

The HIR sits between the untyped AST (from `ocql-ql-parser`) and the MIR (which
lowers to flat Datalog rules). Its job is to take the raw parse tree and produce
a fully-resolved, type-checked, validated representation where:

- Every name reference points to a specific declaration (`DefId`)
- Every expression and variable has a known type
- Bridge nodes (`ExprFormula`/`FormulaExpr`) are resolved
- Class hierarchies are linearized and validated
- All variables are checked for boundness (range restriction)
- Syntactic sugar is partially desugared
- Annotations are validated and associated with their targets
- Errors are collected with source locations for diagnostics

The HIR does **not** eliminate classes, flatten modules, or lower aggregates —
that is MIR's job.

## 2. Architecture Overview

```
                  ┌──────────────────────┐
                  │ .ql/.qll source files │
                  └──────────┬───────────┘
                             │
                  ┌──────────▼───────────┐
                  │  ocql-ql-parser       │  Per-file parsing
                  │  → Vec<SourceFile>    │
                  └──────────┬───────────┘
                             │
            ┌────────────────▼────────────────┐
            │         ocql-hir                 │
            │                                  │
            │  Phase 1: Project Loading        │  Parse all files, build FileId map
            │  Phase 2: Module Graph           │  Resolve imports, build dep graph
            │  Phase 3: Declaration Collection │  Populate 6 namespaces per module
            │  Phase 4: Name Resolution        │  Resolve all references → DefId
            │  Phase 5: Type Checking          │  Infer/check types of all nodes
            │  Phase 6: Class Analysis         │  Linearization, overrides, abstract
            │  Phase 7: Boundness Analysis     │  Verify range restriction
            │  Phase 8: Validation             │  Annotations, monotonicity, etc.
            │                                  │
            │  Output: HirDatabase             │
            └────────────────┬────────────────┘
                             │
                  ┌──────────▼───────────┐
                  │  ocql-mir (future)    │
                  └──────────────────────┘
```

## 3. Core Data Structures

### 3.1 Identity: DefId System

Every declaration in the program gets a unique `DefId`. This is the backbone
of the HIR — all references point to DefIds, all type information is keyed
by DefIds.

```rust
/// Identifies a source file in the project.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct FileId(u32);

/// Identifies a declaration within a file.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct LocalDefId(u32);

/// Globally unique identifier for any declaration.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct DefId {
    pub file: FileId,
    pub local: LocalDefId,
}

/// What kind of thing a DefId refers to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DefKind {
    Module,             // explicit module or file-level module
    Class,              // class declaration
    Newtype,            // newtype declaration
    NewtypeBranch,      // branch of a newtype
    Predicate,          // non-member predicate
    MemberPredicate,    // member predicate (in a class)
    CharPredicate,      // characteristic predicate
    Field,              // class field
    Variable,           // local variable (from VarDecl, quantifier, select)
    TypeAlias,          // type alias (class X = Y)
    ModuleAlias,        // module alias
    PredicateAlias,     // predicate alias
    SignatureModule,    // signature module declaration
    SignatureType,      // signature type declaration
    SignaturePredicate, // signature predicate declaration
    TypeParam,          // parameterized module type parameter
    DbTable,            // database table (from .dbscheme)
    DbEntity,           // database entity type (from .dbscheme)
}
```

### 3.2 Type System

```rust
/// A resolved type in the HIR.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    /// Primitive: boolean, int, float, string, date
    Primitive(PrimitiveType),

    /// A class type, referring to its DefId
    Class(DefId),

    /// A database entity type: @element, @function, etc.
    DbEntity(DbEntityId),

    /// A newtype branch
    NewtypeBranch(DefId),

    /// A type parameter (unresolved until module instantiation)
    TypeParam(DefId),

    /// Union of multiple types (from type alias `class X = A or B`)
    Union(Vec<Type>),

    /// The "none" type (bottom — no values, for `none()`)
    Bottom,

    /// Error type (produced when type checking fails, prevents cascading)
    Error,
}

/// Identifies a database entity from the .dbscheme
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct DbEntityId(u32);
```

### 3.3 Module and Namespace

```rust
/// The 6 namespaces per QL module.
pub struct ModuleNamespaces {
    /// Module namespace: module names → DefId
    pub modules: HashMap<String, Vec<DefId>>,

    /// Type namespace: type names → DefId
    pub types: HashMap<String, Vec<DefId>>,

    /// Predicate namespace: (name, arity) → DefId
    pub predicates: HashMap<(String, usize), Vec<DefId>>,

    /// Module signature namespace
    pub module_sigs: HashMap<String, Vec<DefId>>,

    /// Type signature namespace
    pub type_sigs: HashMap<String, Vec<DefId>>,

    /// Predicate signature namespace
    pub pred_sigs: HashMap<(String, usize), Vec<DefId>>,
}

/// Visibility of a declaration
pub enum Visibility {
    Public,
    Private,
}
```

### 3.4 HIR Nodes

The HIR mirrors the AST structure but with resolved references. Key differences
from the AST:

1. All name references become `DefId` or `ResolvedRef`
2. Every `Expr` carries a `Type`
3. Bridge nodes (`ExprFormula`/`FormulaExpr`) are eliminated
4. Some sugar is lowered (`implies`, `forex`, `if-then-else`)
5. Annotations are validated and structured

```rust
/// A resolved reference to a declaration.
#[derive(Clone, Debug)]
pub enum ResolvedRef {
    /// Reference to a local or imported declaration
    Def(DefId),
    /// Reference to a database predicate (table)
    DbPredicate(DbEntityId, String), // entity, table name
    /// Builtin predicate or operation
    Builtin(BuiltinId),
    /// Unresolved (error case — still present for error recovery)
    Unresolved,
}

/// An HIR expression node. Every expression has a resolved type.
pub struct HirExpr {
    pub kind: HirExprKind,
    pub ty: Type,
    pub span: Span,
}

/// An HIR formula node.
pub struct HirFormula {
    pub kind: HirFormulaKind,
    pub span: Span,
}
```

We do **not** duplicate all 35+ ExprKind variants in the HIR. Instead, we
reuse the AST types and layer resolution information on top via side tables
(indexed by NodeId or Span). This avoids a massive parallel type hierarchy
and keeps the codebase manageable. See §3.5.

### 3.5 Resolution Strategy: Side Tables vs. New AST

**Decision: Side-table approach (like rustc's `TypeckResults`).**

Rather than building a complete parallel HIR tree, we keep the original AST
and attach resolution information in indexed side tables:

```rust
/// Per-file analysis results, keyed by AST node identity.
pub struct FileAnalysis {
    /// The parsed AST (owned)
    pub ast: SourceFile,

    /// DefId assigned to each declaration in this file
    pub def_ids: Vec<(Span, DefId, DefKind)>,

    /// Name resolution: maps reference spans → target DefId
    pub name_resolution: HashMap<Span, ResolvedRef>,

    /// Type of each expression (keyed by expression span)
    pub expr_types: HashMap<Span, Type>,

    /// Type of each variable declaration
    pub var_types: HashMap<DefId, Type>,

    /// Resolved bridge nodes: ExprFormula → what it actually is
    pub bridge_resolutions: HashMap<Span, BridgeResolution>,

    /// Diagnostics produced during analysis
    pub diagnostics: Vec<Diagnostic>,
}

pub enum BridgeResolution {
    /// This ExprFormula is actually a predicate call (no result)
    PredicateCall { target: DefId },
    /// This FormulaExpr is actually an expression with a boolean type
    BooleanExpr,
    /// This is a type test or other special form
    Other,
}
```

**Rationale:** The AST is already well-designed and comprehensive. Duplicating
it would double the type surface area with minimal benefit. The side-table
approach is proven (rustc, TypeScript) and lets us incrementally add analysis
without rewriting the tree. The MIR phase will build a genuinely different
representation when it eliminates classes and flattens modules.

### 3.6 The HirDatabase (Central Query Interface)

```rust
/// The central interface to all HIR analysis results.
/// Named "database" in the salsa/query-system sense, not the CodeQL sense.
pub struct HirDatabase {
    /// Source files: FileId → (path, source text, parsed AST)
    files: HashMap<FileId, FileData>,

    /// Module graph: which file imports which
    module_graph: ModuleGraph,

    /// Per-module namespace tables
    namespaces: HashMap<DefId, ModuleNamespaces>,

    /// Per-file analysis results
    analyses: HashMap<FileId, FileAnalysis>,

    /// Class hierarchy information
    class_hierarchy: ClassHierarchy,

    /// Database schema (from .dbscheme, provided by Track A's ocql-schema)
    db_schema: Option<DbSchemeInfo>,

    /// Global diagnostics
    diagnostics: Vec<Diagnostic>,
}

/// Processed .dbscheme information for HIR use
pub struct DbSchemeInfo {
    /// Entity name → DbEntityId mapping
    pub entities: HashMap<String, DbEntityId>,
    /// Entity hierarchy (union types)
    pub entity_hierarchy: HashMap<DbEntityId, Vec<DbEntityId>>,
    /// Table predicates: table name → (column names, column types)
    pub tables: HashMap<String, Vec<(String, Type)>>,
}
```

## 4. Phase-by-Phase Design

### Phase 1: Project Loading

**Input:** A root path (directory or .ql file) + optional .dbscheme path
**Output:** `HashMap<FileId, FileData>` — all parsed files

Steps:
1. If root is a .ql file, parse it and transitively discover imports
2. If root is a directory, find all .ql/.qll files
3. Parse each file with `ocql-ql-parser::parse_source_file()`
4. Assign `FileId` to each file
5. Store (path, source text, AST) triples

**Key decisions:**
- Lazy vs eager loading: **eager** for simplicity (parse everything up front)
- qlpack.yml: For now, treat the root directory as the package root. Full
  qlpack.yml support is future work.
- Error handling: Files that fail to parse still get a FileId but their AST
  is marked as erroneous. Analysis continues on parseable files.

### Phase 2: Module Graph Construction

**Input:** All parsed files
**Output:** `ModuleGraph` with import edges

Steps:
1. Walk each file's top-level members, extract `Import` nodes
2. Resolve import paths to files:
   - `import cpp` → find `cpp.qll` relative to project root
   - `import semmle.code.cpp.Element` → find `semmle/code/cpp/Element.qll`
   - Aliased imports: record the alias name
3. Build a directed graph: FileId → Vec<(FileId, ImportInfo)>
4. Topological sort for processing order
5. Detect cycles (QL allows mutual imports — they form a single SCC and
   their namespaces are resolved jointly)

```rust
pub struct ModuleGraph {
    /// Import edges: importer → [(imported file, import info)]
    pub edges: HashMap<FileId, Vec<ImportEdge>>,
    /// Topological order (SCCs grouped together)
    pub topo_order: Vec<Vec<FileId>>,
}

pub struct ImportEdge {
    pub target: FileId,
    pub alias: Option<String>,
    pub is_private: bool,
    pub span: Span,
}
```

### Phase 3: Declaration Collection

**Input:** Parsed ASTs + module graph
**Output:** Per-module `ModuleNamespaces`, DefId assignments

Process files in topological order. For each module (file-level or explicit):

1. **Assign DefIds** to every declaration:
   - Each `Predicate`, `ClassDecl`, `ModuleDecl`, `NewtypeDecl`, etc. gets a DefId
   - Each `VarDecl` in predicate params, quantifiers, select clauses gets a DefId
   - Each class member (field, member predicate, char predicate) gets a DefId

2. **Populate namespaces:**
   - Classes → type namespace (name → DefId)
   - Modules → module namespace
   - Predicates → predicate namespace (keyed by (name, arity))
   - Type aliases → type namespace
   - Newtypes → type namespace
   - Signatures → appropriate signature namespace

3. **Handle visibility:**
   - `private` annotation → Private visibility
   - Everything else → Public visibility

4. **Compute "exported" sets:**
   - Public declarations of this module
   - Plus re-exported names from non-private imports (transitively)
   - Minus names shadowed by this module's own declarations

5. **Database types from .dbscheme:**
   - If a .dbscheme is provided, register all `@entity` types in the global
     type namespace and all table predicates in the global predicate namespace
   - Build the entity hierarchy from union types

**Nested modules:** When encountering a `ModuleDecl`, recursively create its
namespaces. The nested module's DefId goes into the parent's module namespace.
The nested module inherits access to the parent's visible names (plus its own
private names).

### Phase 4: Name Resolution

**Input:** ASTs + populated namespaces
**Output:** `name_resolution` maps (Span → ResolvedRef) per file

This is the most complex phase. Walk every AST node and resolve references:

#### 4.1 Simple Name Resolution

For each name reference, search in order:
1. Local scope (quantifier variables, predicate parameters, `this`, `result`)
2. Current module's visible names (declared + imported + parent scope)
3. Global namespace (primitives, database types, builtins)

#### 4.2 Qualified Name Resolution

For `A::B::C::pred(...)`:
1. Resolve `A` in the module namespace → get module DefId
2. In A's exported namespace, resolve `B` → module DefId
3. In B's exported namespace, resolve `C` → module DefId
4. In C's exported namespace, resolve `pred` in predicate namespace (by arity)

#### 4.3 Bridge Node Resolution

The parser produces bridge nodes where formulas and expressions are ambiguous.
Resolution rules:

- **`FormulaKind::ExprFormula(expr)`**: The parser put a bare `Expr` (usually a
  `Call`) in formula context. Check if the called predicate has no result type →
  it's a predicate call formula. If it has a result → it's actually a boolean
  expression (error or implicit `exists`).

- **`ExprKind::FormulaExpr(formula)`**: A parenthesized formula appeared in
  expression context. This is valid when the formula is used as a boolean
  expression in certain contexts (mainly inside aggregation guards).

Resolution:
1. For `ExprFormula(Call { name, args })`:
   - Look up `name` with arity `args.len()` in predicate namespace
   - If found and predicate has no result → resolve as `PredicateCall`
   - If found and predicate has result → resolve as `ExistsExpr` (implicit exists)
   - If not found → error

2. For `ExprFormula(MemberCall { receiver, name, args })`:
   - Type-check receiver → get receiver type
   - Look up `name` as member predicate of receiver type
   - Same result/no-result distinction

3. For `FormulaExpr(formula)`:
   - This occurs in aggregation guard/body positions
   - Keep as-is (the formula is used as a constraint, not an expression)

#### 4.4 Overload Resolution

Predicates are overloaded by arity (name, arity) pairs. When resolving a call:
1. Count the number of arguments
2. Look up (name, arg_count) in the predicate namespace
3. For predicates with result type, the result is NOT counted in arity

#### 4.5 Type Resolution in Expressions

Type expressions (`TypeExpr`) are resolved to `Type`:
- `PrimitiveType` → `Type::Primitive(...)`
- `Database("@foo")` → look up in db_schema → `Type::DbEntity(...)`
- `ClassName(name)` → look up in type namespace → `Type::Class(def_id)`
- `ModuleAccess(mod, ty)` → resolve module, then type in module's exports

#### 4.6 Scoping Rules

QL has relatively simple scoping:
- **File level:** All file-level declarations are in scope everywhere in the file
  (no forward-declaration needed)
- **Class level:** All class members are in scope within the class body
  (`this` is implicitly in scope)
- **Predicate level:** Parameters are in scope in the body. `result` is in scope
  if the predicate has a result type.
- **Quantifiers:** Variables declared in `exists(Type x | ...)` scope over
  the guard and body of that quantifier only.
- **Aggregations:** Variables in `agg(Type x | ...)` scope over the guard,
  expression, and order-by of that aggregation.
- **Select:** `from` variables scope over `where` and `select` clauses.

### Phase 5: Type Checking

**Input:** Name-resolved AST
**Output:** `expr_types` map (Span → Type) per file, diagnostics

#### 5.1 Expression Typing Rules

| Expression | Type |
|---|---|
| `Literal(Int(_))` | `int` |
| `Literal(Float(_))` | `float` |
| `Literal(String(_))` | `string` |
| `Literal(Bool(_))` | `boolean` |
| `Variable(x)` | type of x's declaration |
| `This` | enclosing class type |
| `Result` | predicate's result type |
| `DontCare` | any type (unconstrained) |
| `BinaryOp(+,-,*,/,%)` | numeric (int if both int, else float); `+` on string → string |
| `UnaryOp(Neg)` | same numeric type as operand |
| `Call { name, args }` | result type of resolved predicate |
| `MemberCall { recv, name, args }` | result type of member predicate |
| `PostfixCast { expr, ty }` / `PrefixCast` | the cast target type |
| `Range [low..high]` | same type as low/high (must be compatible) |
| `SetLiteral [a,b,c]` | common type of elements |
| `Aggregation { kind, ... }` | depends on kind (count→int, min/max→element type, avg→float, sum→numeric, concat→string) |
| `RankExpr` | type of the expression being ranked |
| `AnyExpr` | type of the expression (or variable type if no expr) |
| `Super { type, name, args }` | result type of the specific supertype's method |
| `Paren(e)` | type of e |

#### 5.2 Formula Typing (Validation)

Formulas don't have types per se (they're boolean), but we validate:
- Comparison operands have compatible types
- `instanceof` target is a valid type
- Predicate calls match arity and argument types
- Quantifier variable types are valid (non-primitive types must exist)
- Negation doesn't introduce unbound variables

#### 5.3 Type Compatibility

Two types are compatible if they share the same "universe":
- `int` and `float` share the "number" universe
- Each database entity type is its own universe (but subtypes are compatible)
- Each class is compatible with its supertypes
- Primitive types are incompatible with each other (except int/float)

A class cannot extend types from different universes.

#### 5.4 Subtype Checking

`A <: B` (A is a subtype of B) if:
- A and B are the same type
- A is a class that extends B (directly or transitively)
- A is a database entity that is a variant of B (via union types in .dbscheme)
- A is a newtype branch and B is the newtype

### Phase 6: Class Hierarchy Analysis

**Input:** All class declarations with resolved supertypes
**Output:** `ClassHierarchy` with linearization, override info

#### 6.1 C3 Linearization

For each class, compute the Method Resolution Order (MRO) using C3:

```
L(C) = C + merge(L(B1), L(B2), ..., [B1, B2, ...])
```

where B1, B2, ... are the direct supertypes in declaration order.

The merge operation picks the first element from any list that doesn't appear
in the tail of any other list, removes it from all lists, and repeats.

If merge fails (no valid element), the hierarchy is inconsistent → error.

#### 6.2 Override Validation

For each member predicate with `override` annotation:
1. Check that the predicate exists in at least one supertype
2. Check that the overridden predicate is not `final`
3. Check that the signature matches (same name, arity, compatible param types)
4. Check that abstract predicates are overridden in non-abstract subclasses

#### 6.3 Abstract Class Validation

For each non-abstract class:
1. Check that all inherited abstract predicates are overridden
2. For abstract classes: ensure they have at least one non-abstract subclass
   (warning, not error — may be defined in a different file)

#### 6.4 Dispatch Table

For each class, compute which DefId handles each (name, arity) predicate:
1. Start from the class's own declarations
2. Walk the MRO; for each predicate not yet in the table, add it
3. Handle `final` supertypes: predicates inherited through `final` cannot
   be overridden (only shadowed)

```rust
pub struct ClassHierarchy {
    /// MRO for each class
    pub linearization: HashMap<DefId, Vec<DefId>>,

    /// For each class, the dispatch table: (pred_name, arity) → DefId
    pub dispatch: HashMap<DefId, HashMap<(String, usize), DefId>>,

    /// Direct subtypes for each class
    pub subtypes: HashMap<DefId, Vec<DefId>>,

    /// Which predicates each class overrides
    pub overrides: HashMap<DefId, Vec<(DefId, DefId)>>, // (class, overridden_pred → overriding_pred)
}
```

### Phase 7: Boundness Analysis

**Input:** Type-checked, name-resolved AST
**Output:** Per-predicate boundness status, diagnostics for unbound variables

A variable is **bound** if it is restricted to a finite set of values.

#### 7.1 Binding Sources

A variable `x` becomes bound in these contexts:
- `x` has a non-primitive type (class, database entity) → always bound
- `x = <constant>` → bound
- `x = [low..high]` where both are constants → bound
- `x = f(...)` where `f` is a predicate call and all args are bound → bound
- `pred(x, ...)` where `pred` binds that argument position → bound
- `x instanceof Type` → bound (if Type is non-primitive)
- Equality with a bound variable: `x = y` where y is bound → x is bound

A variable is **not** bound by:
- `x > y`, `x < y`, `x != y` — comparisons don't bind
- `not P(x)` — negation doesn't bind
- `forall(... | ... | uses(x))` — universal quantifier body doesn't bind outer x

#### 7.2 Binding Propagation Algorithm

For each predicate body / select clause:
1. Collect all free variables
2. Walk the formula top-down
3. For conjunctions (`and`): bindings from either side propagate
4. For disjunctions (`or`): only bindings present in ALL branches propagate
5. For negation (`not`): no bindings propagate outward
6. For quantifiers: internal variables are bound by their declarations;
   check that all internal variables are bound within the quantifier
7. Check that all free variables in the output are bound

#### 7.3 Bindingset Annotations

`bindingset[x, y]` on a predicate declares: "if x and y are bound by the
caller, then the predicate is finite." This means:
- The predicate itself may use `x` and `y` in unbounded ways internally
- But callers must bind those arguments

Multiple `bindingset` annotations on the same predicate are alternatives:
`bindingset[x] bindingset[y]` means either x OR y being bound suffices.

### Phase 8: Final Validation

Collect remaining checks:

1. **Annotation validation:**
   - `override` only on member predicates with matching supertype predicate
   - `abstract` only on classes and member predicates
   - `final` not on abstract entities
   - `query` only in query modules (.ql)
   - `pragma` kinds are valid for the context
   - `bindingset` params match predicate parameters

2. **Recursion monotonicity:**
   - Build the predicate call graph
   - For each cycle, check that recursion passes through an even number of
     negations (including aggregation expressions, which count as negation
     boundaries)

3. **Signature conformance:**
   - For parameterized modules with `implements`, check that all signature
     declarations are satisfied

4. **Select clause validation:**
   - At least one select expression
   - All select expressions have valid types
   - Order-by expressions reference available names

## 5. Desugaring (in HIR)

Partial desugaring is done in the HIR to simplify later phases. The MIR will
do further lowering.

| Sugar | Desugaring |
|---|---|
| `A implies B` | `not A or B` |
| `forex(vars \| guard \| body)` | `forall(vars \| guard \| body) and exists(vars \| guard)` |
| `if cond then t else e` | `(cond and t) or (not cond and e)` |
| `x in [a..b]` | `x >= a and x <= b` |
| `exists(expr)` | `exists(T _v \| _v = expr)` |
| `pred*(args)` | Introduce recursive helper: `predStar(x, y) :- x = y or exists(z \| pred(x, z) and predStar(z, y))` |
| `pred+(args)` | Introduce recursive helper: `predPlus(x, y) :- pred(x, y) or exists(z \| pred(x, z) and predPlus(z, y))` |

**Note:** Closure desugaring (`+`/`*`) is complex because it needs to create new
predicate DefIds. It may be deferred to MIR if the HIR complexity is too high.

## 6. Error Handling and Diagnostics

```rust
#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub file: FileId,
    pub notes: Vec<DiagnosticNote>,
}

pub enum Severity {
    Error,
    Warning,
    Info,
}

pub struct DiagnosticNote {
    pub message: String,
    pub span: Span,
    pub file: FileId,
}
```

Error recovery strategy: when encountering an error, produce a diagnostic and
insert a "poison" value (`Type::Error`, `ResolvedRef::Unresolved`) to prevent
cascading errors. Continue analysis on unaffected parts of the program.

## 7. Dependencies

```
ocql-common         (Span, Spanned)
ocql-ql-ast         (all AST types — we reference them, not duplicate)
ocql-ql-parser      (parse_source_file entry point)
ocql-schema         (DbScheme for database type resolution)
```

The HIR does NOT depend on `ocql-database` or `ocql-engine` (Track A).

## 8. Implementation Milestones

### Milestone 1: Single-File Foundation

**Goal:** Analyze a single .ql file with primitive types only.

Deliverables:
- DefId assignment for all declarations
- Name resolution for simple (non-qualified) references
- Type checking for primitive expressions and basic formulas
- Variable scoping (quantifiers, select, predicates)
- Bridge node resolution (ExprFormula → PredicateCall)
- Basic diagnostics (undefined variable, type mismatch)

Test target:
```ql
predicate isSmall(int x) { x >= 0 and x < 10 }
from int x
where isSmall(x) and x > 5
select x
```

### Milestone 2: Classes and Type Hierarchy

**Goal:** Handle class declarations, inheritance, member predicates.

Deliverables:
- Class declarations populate type namespace
- `this` and `result` resolution
- Member predicate resolution (receiver typing)
- C3 linearization
- Override validation
- Abstract class checking
- `instanceof` type checking
- Cast expression typing

Test target:
```ql
class SmallInt extends int {
    SmallInt() { this >= 0 and this < 100 }
    predicate isEven() { this % 2 = 0 }
}
from SmallInt x
where x.isEven()
select x
```

### Milestone 3: Multi-File and Imports

**Goal:** Handle projects with multiple .qll/.ql files.

Deliverables:
- Module graph construction from imports
- Import path resolution (qualified paths → files)
- Cross-file name resolution
- Visibility (private imports, private declarations)
- Re-export computation
- Explicit module declarations (nested modules)
- Module-qualified references (`Module::pred()`)

Test target: A .ql file importing a .qll library with class definitions.

### Milestone 4: Database Integration

**Goal:** Resolve `@`-prefixed database types from .dbscheme.

Deliverables:
- Parse .dbscheme via `ocql-schema`
- Register database entity types in global namespace
- Register table predicates in global predicate namespace
- Entity subtype hierarchy from union types
- Validate `@type` references in QL code

Test target:
```ql
import cpp
from @function f, string name
where functions(f, name, _)
select f, name
```

### Milestone 5: Boundness and Full Validation

**Goal:** Complete the analysis pipeline.

Deliverables:
- Boundness analysis with propagation
- `bindingset` annotation support
- Recursion monotonicity checking
- Annotation validation (all annotation kinds)
- Comprehensive error messages

### Milestone 6: Parameterized Modules and Signatures

**Goal:** Handle generic modules, the most complex feature.

Deliverables:
- Signature module/type/predicate declarations
- Parameterized module type parameters
- Module instantiation (applicative — same args → same instance)
- Signature conformance checking
- Qualified access through parameterized modules

This is the most complex milestone and may be split further.

## 9. What the HIR Does NOT Do (MIR's Job)

The following transformations are **out of scope** for the HIR:

- **Class elimination** — converting classes to characteristic predicates + dispatch
- **Module elimination** — flattening module structure, fully qualifying names
- **Aggregate lowering** — converting aggregations to grouped fixpoint operations
- **Negation stratification** — computing strata for evaluation ordering
- **Predicate specialization** — instantiating parameterized predicates

## 10. Testing Strategy

### Unit Tests
- Name resolution: test that each name in a snippet resolves correctly
- Type checking: test expression type inference on small examples
- Class hierarchy: test C3 linearization on known hierarchies
- Boundness: test that bound/unbound variables are correctly identified

### Snapshot Tests
- Parse a .ql file → run HIR analysis → dump resolution map and types → compare
  with expected snapshot

### Integration Tests
- Run HIR analysis on selected vendor/codeql files (start with simple ones)
- Measure "analysis rate" similar to parser's "parse rate" metric

### Error Tests
- Verify that known-bad programs produce the expected diagnostics
- Test error recovery (bad code in one function doesn't break analysis of others)

## 11. Open Questions

1. **Incremental computation:** Should we use a query framework (like salsa) for
   incremental re-analysis? Decision: **No, not initially.** Start with a simple
   batch pipeline. Add incrementality later if IDE support becomes a goal.

2. **Parameterized module monomorphization timing:** Should parameterized modules
   be monomorphized in HIR or MIR? Decision: **MIR.** The HIR keeps them
   polymorphic and just validates constraints. MIR monomorphizes when it
   flattens modules.

3. **Closure desugaring location:** Should `+`/`*` closures be desugared in HIR
   or MIR? Decision: **MIR.** Closures create new predicates, which is a MIR
   concern. The HIR just validates that the closure is applied to a valid
   member predicate with the right arity (unary — one `this` parameter, one
   result, forming a binary relation).

4. **How to handle files that fail to parse:** Decision: **Skip them.** Register
   the FileId but mark as unparseable. Other files that import them get an
   "import target has parse errors" diagnostic.
