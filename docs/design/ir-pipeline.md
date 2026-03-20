# IR Pipeline Design

This document details each intermediate representation in the open-cql compilation pipeline.

## 1. AST (Abstract Syntax Tree)

The untyped, direct representation of QL source code.

### Node Types

```
// Top-level
Module          = { kind: Query|Library, imports: [Import], members: [Member] }
Import          = { path: QualifiedName, alias: Option<Name>, is_private: bool }
Member          = Predicate | Class | Module | Alias | Select

// Declarations
Predicate       = { annotations: [Annotation], result_type: Option<TypeExpr>,
                    name: Name, params: [Param], body: Option<Formula> }
Class           = { annotations: [Annotation], name: Name,
                    supertypes: [TypeExpr], body: [ClassMember] }
ClassMember     = CharPredicate | MemberPredicate | Field
Alias           = ModuleAlias | TypeAlias | PredicateAlias
Select          = { from: [VarDecl], where: Option<Formula>,
                    select: [SelectExpr], order_by: [OrderBy] }

// Type expressions
TypeExpr        = PrimitiveTy | DatabaseTy | ClassTy | ModuleSelection

// Formulas
Formula         = Conjunction     { lhs, rhs }
                | Disjunction     { lhs, rhs }
                | Negation        { inner }
                | Comparison      { lhs, op, rhs }
                | InstanceOf      { expr, type }
                | InRange         { expr, range }
                | Exists          { vars, formula }
                | Forall          { vars, guard, body }
                | Forex           { vars, guard, body }
                | IfThenElse      { cond, then, else }
                | Implies         { lhs, rhs }
                | PredicateCall   { receiver, name, args, closure }
                | Any
                | None

// Expressions
Expr            = Literal         { kind: Int|Float|String|Bool|Date }
                | Variable        { name }
                | This
                | Result
                | DontCare                              // underscore _
                | Call            { receiver, name, args }
                | ClosureCall     { receiver, name, args, kind: Plus|Star }
                | Aggregation     { kind, vars, guard, expr, order_by }
                | UnaryOp         { op, operand }
                | BinaryOp        { lhs, op, rhs }
                | Cast            { expr, type }
                | Range           { low, high }
                | SetLiteral      { elements }
                | SuperExpr       { type, name }

// Aggregation kinds
AggregateKind   = Count | Min | Max | Avg | Sum | Concat | Rank | Unique
                | StrictCount | StrictSum | StrictConcat | Any_
```

### Source Locations

Every AST node carries a `Span`:
```rust
struct Span {
    file: FileId,
    start: Position,
    end: Position,
}

struct Position {
    line: u32,
    column: u32,
    offset: u32,  // byte offset from file start
}
```

## 2. HIR (High-level IR)

The typed, resolved version of the AST. All names are resolved to unique IDs.

### Key Differences from AST

1. **All names resolved** — Every identifier points to a unique `DefId`
2. **Types computed** — Every expression has a known type
3. **Imports resolved** — No import statements, all references are fully qualified
4. **Syntactic sugar lowered:**
   - `implies` → disjunction with negation
   - `forex` → `forall` + `exists`
   - `if-then-else` → conjunction/disjunction
   - Closure calls → explicit recursive predicate references

### HIR Data Structures

```
HirModule {
    id: ModuleId,
    predicates: Map<PredicateId, HirPredicate>,
    classes: Map<ClassId, HirClass>,
    submodules: Map<ModuleId, HirModule>,
}

HirPredicate {
    id: PredicateId,
    annotations: AnnotationSet,
    result_type: Option<TypeId>,
    params: Vec<(ParamId, TypeId)>,
    body: Option<HirFormula>,
}

HirClass {
    id: ClassId,
    annotations: AnnotationSet,
    supertypes: Vec<TypeId>,
    linearization: Vec<TypeId>,       // MRO (method resolution order)
    char_predicate: Option<HirFormula>,
    members: Vec<HirMemberPredicate>,
    fields: Vec<(FieldId, TypeId, HirFormula)>,
}

HirFormula = Conjunction(Box<HirFormula>, Box<HirFormula>)
           | Disjunction(Box<HirFormula>, Box<HirFormula>)
           | Negation(Box<HirFormula>)
           | Comparison(HirExpr, CompOp, HirExpr)
           | InstanceOf(HirExpr, TypeId)
           | Exists(Vec<(VarId, TypeId)>, Box<HirFormula>)
           | Forall(Vec<(VarId, TypeId)>, Box<HirFormula>, Box<HirFormula>)
           | Call(PredicateId, Vec<HirExpr>)
           | True
           | False

HirExpr = Literal(Value)
        | Var(VarId)
        | Call(PredicateId, Vec<HirExpr>)      // with result
        | Aggregation(AggKind, Vec<(VarId, TypeId)>,
                      Box<HirFormula>, Box<HirExpr>)
        | BinaryOp(Box<HirExpr>, BinOp, Box<HirExpr>)
        | UnaryOp(UnOp, Box<HirExpr>)
        | Cast(Box<HirExpr>, TypeId)
```

### Name Resolution Strategy

1. Build a scope tree from the module structure
2. For each identifier, walk up the scope tree checking each namespace
3. Handle shadowing: inner scopes shadow outer scopes
4. Handle qualified references: `Module::name` lookups
5. Handle parameterized module instantiation

### Type Checking

1. Compute types bottom-up from expressions
2. Check that predicate call arguments match parameter types
3. Verify class hierarchy constraints (no multi-universe inheritance)
4. Check that variables are bound (range restriction)

## 3. MIR (Mid-level IR)

A flat, Datalog-like representation. No classes, no modules, just predicates.

### Key Transformations

1. **Class elimination:**
   ```
   class SmallInt extends int {
     SmallInt() { this in [1..9] }
     int double() { result = this * 2 }
   }
   ```
   Becomes:
   ```
   pred SmallInt#char(this: int) { this in [1..9] }
   pred SmallInt#double(this: int, result: int) {
     SmallInt#char(this) and result = this * 2
   }
   ```

2. **Override dispatch:**
   For overridden predicates, generate dispatch:
   ```
   pred Base#foo(this, result) {
     (SubA#char(this) and SubA#foo(this, result)) or
     (SubB#char(this) and SubB#foo(this, result)) or
     (not SubA#char(this) and not SubB#char(this) and Base#foo_impl(this, result))
   }
   ```

3. **Aggregate lowering:**
   ```
   count(int x | x in [1..10] and x % 2 = 0 | x)
   ```
   Becomes:
   ```
   // Auxiliary predicate for the aggregate body
   pred agg_body_1(x: int) { x in [1..10] and x % 2 = 0 }
   // Aggregate computation
   pred agg_result_1(result: int) { result = COUNT(agg_body_1) }
   ```

4. **Negation stratification:**
   Build predicate dependency graph. Assign each predicate to a stratum
   such that negated dependencies are in strictly lower strata.

### MIR Data Structures

```
MirProgram {
    predicates: Vec<MirPredicate>,
    strata: Vec<Vec<PredicateId>>,      // Evaluation order
}

MirPredicate {
    id: PredicateId,
    name: QualifiedName,
    params: Vec<(Name, TypeId)>,
    body: MirBody,
    is_recursive: bool,
    stratum: usize,
}

MirBody = Conjunction(Vec<MirAtom>)     // A rule body: list of atoms
        | Disjunction(Vec<MirBody>)     // Union of conjunctions

MirAtom = Positive(PredicateId, Vec<MirTerm>)
        | Negative(PredicateId, Vec<MirTerm>)
        | Guard(MirTerm, CompOp, MirTerm)
        | Bind(VarId, MirTerm)
        | Aggregate(AggKind, PredicateId, Vec<GroupByVar>, ResultVar)

MirTerm = Var(VarId)
        | Const(Value)
        | BinOp(Box<MirTerm>, BinOp, Box<MirTerm>)
```

## 4. LIR (Low-level IR)

Pure relational algebra with extensions.

### Core Operators

```
LirOp =
    // Base
    | Scan(RelationId)                          // Read base table
    | Constant(Vec<Vec<Value>>)                 // Inline constant relation

    // Standard relational algebra
    | Project(Box<LirOp>, Vec<ColId>)           // Column selection
    | Select(Box<LirOp>, Condition)             // Row filter
    | Join(Box<LirOp>, Box<LirOp>, JoinCond)    // Equi-join
    | Union(Vec<LirOp>)                         // Set union
    | Difference(Box<LirOp>, Box<LirOp>)        // Set difference
    | Rename(Box<LirOp>, Map<ColId, ColId>)     // Column rename
    | Distinct(Box<LirOp>)                      // Deduplicate

    // Extensions
    | Fixpoint(FixpointOp)                      // Recursive computation
    | Aggregate(Box<LirOp>, Vec<ColId>,         // Group-by + aggregate
                AggFunc, ColId)
    | Sort(Box<LirOp>, Vec<(ColId, Dir)>)       // Ordering
    | Limit(Box<LirOp>, usize)                  // Top-N

    // Flow-tracking extensions
    | FlowStep(Box<LirOp>, FlowConfig)          // Single flow step
    | FlowFixpoint(FlowFixpointOp)              // Flow-aware fixpoint

FixpointOp {
    base: Box<LirOp>,          // Initial (non-recursive) part
    step: Box<LirOp>,          // Recursive step (references delta)
    delta_ref: RelationId,     // Name of the delta relation
    max_iterations: Option<u32>,
}

FlowFixpointOp {
    sources: Box<LirOp>,       // Source nodes
    sinks: Box<LirOp>,         // Sink nodes
    steps: Vec<FlowStepDef>,   // Flow step definitions
    config: FlowConfig,
}

FlowConfig {
    track_paths: bool,          // Whether to record exact paths
    context_sensitivity: ContextKind,
    max_path_length: Option<u32>,
}
```

## 5. Query Plan

The physical execution plan, produced by the optimizer from LIR.

### Plan Operators

```
Plan =
    | TableScan(TableId, Option<IndexId>)       // Full/index scan
    | IndexLookup(TableId, IndexId, Vec<Value>) // Point lookup
    | HashJoin(Box<Plan>, Box<Plan>, JoinKeys)  // Hash join
    | MergeJoin(Box<Plan>, Box<Plan>, JoinKeys) // Merge join (sorted)
    | NestedLoop(Box<Plan>, Box<Plan>, Cond)    // Nested loop join
    | Filter(Box<Plan>, Condition)
    | ProjectPlan(Box<Plan>, Vec<ColId>)
    | UnionAll(Vec<Plan>)
    | Sort(Box<Plan>, Vec<(ColId, Dir)>)
    | Materialize(Box<Plan>, MaterializeId)     // Cache result
    | MaterializeRead(MaterializeId)            // Read cached
    | SemiNaive(SemiNaivePlan)                  // Recursive eval
    | FlowAnalysis(FlowPlan)                    // Flow computation
```

## 6. Dumping and Debugging

Each IR stage supports textual dump format for debugging and snapshot testing:

```
// AST dump (S-expression style)
(select
  (from (var x int) (var y int))
  (where (and (= x 3) (in y (range 0 2))))
  (select x y (* x y)))

// HIR dump (typed, resolved)
query#1(x: int, y: int, _result: int) {
  x = 3 AND y IN [0..2] AND _result = x * y
}

// MIR dump (Datalog-style)
query#1(x, y, r) :- x = 3, y >= 0, y <= 2, r = x * y.

// LIR dump (relational algebra)
Project[0,1,2](
  Select[col0 = 3](
    Join[](
      Scan(int_range(0, 2)) AS [col1],
      Constant([[3]]) AS [col0]
    )
  ) ⨝ Compute[col2 = col0 * col1]
)
```
