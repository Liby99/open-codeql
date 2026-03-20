# Datalog+Flow Engine Design

This document details the evaluation engine for open-cql, which extends traditional
Datalog evaluation with exact flow tracking capabilities.

## 1. Why Traditional Datalog Is Insufficient

### What Datalog Gives You
Standard Datalog evaluates recursive rules to a least fixed point. For data flow:

```
reaches(x, y) :- edge(x, y).
reaches(x, z) :- reaches(x, y), edge(y, z).
```

This tells you **whether** x can reach z, but not **how**.

### What CodeQL Needs

CodeQL's data flow analysis requires:

1. **Exact paths**: "Tainted data flows from `getUserInput()` at line 5 through
   `buffer` at line 8 to `sql.execute()` at line 12"

2. **Access paths**: Tracking data through object fields:
   ```java
   obj.field = tainted;     // taint enters obj.field
   x = obj.field;           // taint exits to x
   sink(x);                 // x reaches sink
   ```

3. **Context sensitivity**: Distinguishing different call sites:
   ```java
   String clean = sanitize(safe_data);    // call site 1
   String dirty = identity(user_input);   // call site 2
   sink(dirty);  // only this should be flagged
   ```

4. **Partial flow**: Showing how far tainted data propagates even if it
   doesn't reach a known sink.

## 2. Our Approach: Provenance-Enriched Datalog

### 2.1 Core Idea

Every tuple in the database carries optional **provenance** — metadata about
how it was derived. For flow-related predicates, provenance tracks the exact path.

### 2.2 Tuple Representation

```rust
/// A single value in the database
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Value {
    Int(i64),
    Float(OrderedFloat<f64>),
    String(InternedString),
    Bool(bool),
    Entity(EntityId),       // Reference to a database entity
    Null,
}

/// A tuple is a row in a relation
struct Tuple {
    values: SmallVec<[Value; 4]>,  // Most tuples are small
}

/// A relation is a set of tuples
struct Relation {
    schema: RelationSchema,
    tuples: BTreeSet<Tuple>,
    indexes: Vec<BTreeMap<Value, Vec<TupleId>>>,
}
```

### 2.3 Flow-Aware Relations

For flow analysis, we use specialized relation types:

```rust
/// A flow fact: data flows from source to node via a specific path
struct FlowFact {
    source: EntityId,
    node: EntityId,
    access_path: AccessPath,
    context: CallContext,
    path: FlowPath,
}

/// An access path tracks field-level data flow
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum AccessPath {
    Empty,                              // The value itself
    Field(FieldId, Box<AccessPath>),    // obj.field...
    Element(Box<AccessPath>),           // array[i]...
    Truncated(usize),                   // Approximated after N steps
}

/// Call context for context-sensitive analysis
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum CallContext {
    Empty,
    Push(CallSiteId, Box<CallContext>),
}

/// A flow path records the exact derivation
struct FlowPath {
    steps: Vec<FlowStep>,
}

struct FlowStep {
    from_node: EntityId,
    to_node: EntityId,
    kind: StepKind,
    location: LocationId,
}

enum StepKind {
    LocalAssignment,     // x = y
    FieldStore,          // obj.f = x
    FieldLoad,           // x = obj.f
    ArrayStore,          // arr[i] = x
    ArrayLoad,           // x = arr[i]
    Call,                // f(x) — argument passing
    Return,              // return x — return value
    TaintStep,           // Non-value-preserving (e.g., string concat)
    Cast,                // (Type)x
    ImplicitRead,        // Reading from unknown code
}
```

## 3. Evaluation Algorithm

### 3.1 Overall Structure

```
1. Load base facts from database
2. Compute negation stratification
3. For each stratum (bottom-up):
   a. If non-recursive: evaluate rules once
   b. If recursive: semi-naive fixpoint
   c. If flow-related: flow-aware fixpoint
4. Evaluate query predicates
5. Format and output results
```

### 3.2 Semi-Naive Evaluation

For standard recursive predicates:

```rust
fn evaluate_stratum(rules: &[Rule], base: &mut Database) {
    // Initialize delta relations with base facts
    let mut deltas: HashMap<RelId, Relation> = initialize_deltas(rules, base);

    loop {
        let mut new_deltas: HashMap<RelId, Relation> = HashMap::new();

        for rule in rules {
            // Evaluate rule body, using delta for at least one recursive atom
            let new_tuples = evaluate_rule_with_delta(rule, base, &deltas);

            // Filter out already-known tuples
            let truly_new = new_tuples.difference(&base.get(rule.head));

            new_deltas.entry(rule.head)
                .or_default()
                .extend(truly_new);
        }

        if new_deltas.values().all(|r| r.is_empty()) {
            break;  // Fixed point reached
        }

        // Merge new deltas into base, update deltas for next iteration
        for (rel_id, delta) in new_deltas {
            base.extend(rel_id, &delta);
            deltas.insert(rel_id, delta);
        }
    }
}
```

### 3.3 Flow-Aware Fixpoint

For data flow predicates, we use a worklist algorithm:

```rust
struct FlowWorklist {
    pending: BinaryHeap<FlowWorkItem>,
    seen: HashSet<(EntityId, AccessPath, CallContext)>,
}

struct FlowWorkItem {
    source: EntityId,
    node: EntityId,
    access_path: AccessPath,
    context: CallContext,
    path: FlowPath,
    priority: u32,  // Based on topological order
}

fn compute_flow(
    sources: &[EntityId],
    sinks: &[EntityId],
    flow_steps: &FlowStepDatabase,
    config: &FlowConfig,
) -> Vec<FlowResult> {
    let mut worklist = FlowWorklist::new();
    let mut results = Vec::new();

    // Initialize worklist with sources
    for &source in sources {
        worklist.push(FlowWorkItem {
            source,
            node: source,
            access_path: AccessPath::Empty,
            context: CallContext::Empty,
            path: FlowPath::new(source),
            priority: 0,
        });
    }

    while let Some(item) = worklist.pop() {
        let state = (item.node, item.access_path.clone(), item.context.clone());
        if !worklist.seen.insert(state) {
            continue;  // Already processed this state
        }

        // Check if we reached a sink
        if sinks.contains(&item.node) {
            results.push(FlowResult {
                source: item.source,
                sink: item.node,
                path: item.path.clone(),
            });
        }

        // Propagate flow through local steps
        for step in flow_steps.local_steps_from(item.node) {
            let new_ap = step.transform_access_path(&item.access_path);
            if let Some(new_ap) = new_ap {
                worklist.push(FlowWorkItem {
                    source: item.source,
                    node: step.target,
                    access_path: new_ap,
                    context: item.context.clone(),
                    path: item.path.extend(step),
                    priority: item.priority + 1,
                });
            }
        }

        // Propagate through call edges
        for call_step in flow_steps.call_steps_from(item.node) {
            let new_ctx = CallContext::Push(call_step.call_site,
                                           Box::new(item.context.clone()));
            worklist.push(FlowWorkItem {
                source: item.source,
                node: call_step.callee_param,
                access_path: item.access_path.clone(),
                context: new_ctx,
                path: item.path.extend(&call_step),
                priority: item.priority + 1,
            });
        }

        // Propagate through return edges
        for ret_step in flow_steps.return_steps_from(item.node) {
            if let CallContext::Push(call_site, outer_ctx) = &item.context {
                if ret_step.matches_call_site(*call_site) {
                    worklist.push(FlowWorkItem {
                        source: item.source,
                        node: ret_step.return_site,
                        access_path: item.access_path.clone(),
                        context: *outer_ctx.clone(),
                        path: item.path.extend(&ret_step),
                        priority: item.priority + 1,
                    });
                }
            }
        }
    }

    results
}
```

## 4. Integration with QL Queries

### 4.1 How DataFlow::ConfigSig Maps to Engine Operations

When a QL query uses `DataFlow::Global<MyConfig>`:

```ql
module MyConfig implements DataFlow::ConfigSig {
  predicate isSource(DataFlow::Node node) { ... }
  predicate isSink(DataFlow::Node node) { ... }
  predicate isBarrier(DataFlow::Node node) { ... }
}

module MyFlow = DataFlow::Global<MyConfig>;

from MyFlow::PathNode source, MyFlow::PathNode sink
where MyFlow::flowPath(source, sink)
select sink, "Data flows from $@", source, "here"
```

The compiler recognizes this pattern and generates a FlowFixpoint LIR operation:

```
FlowFixpoint {
    sources: Evaluate(MyConfig::isSource),
    sinks: Evaluate(MyConfig::isSink),
    barriers: Evaluate(MyConfig::isBarrier),
    steps: [LocalFlowSteps, CallFlowSteps, ReturnFlowSteps],
    config: FlowConfig { track_paths: true, ... },
}
```

### 4.2 DataFlow::Node

The `DataFlow::Node` type is a union of:
- Expression nodes (values of expressions)
- Parameter nodes (function parameters)
- Indirect nodes (for pointer dereferencing in C/C++)

In the database, these are represented as entity references into the
expression/parameter tables.

## 5. Performance Strategies

### 5.1 Indexing

Maintain indexes on columns used in joins:
```rust
struct IndexedRelation {
    primary: BTreeSet<Tuple>,                    // Full sorted set
    indexes: HashMap<Vec<ColId>, BTreeMultiMap>,  // Secondary indexes
}
```

Automatically create indexes based on query plan analysis.

### 5.2 Join Strategies

Choose join algorithm based on relation sizes:
- **Hash join**: Default for large relations
- **Merge join**: When both inputs are sorted on join keys
- **Index nested loop**: When one side is small and the other has an index

### 5.3 Parallelism

- Independent strata can be evaluated in parallel
- Within a stratum, independent rules can be parallelized
- Join evaluation can use work-stealing parallelism (rayon)

### 5.4 Memory Management

- Arena allocation per stratum evaluation
- Intern all strings into a global string table
- Use entity IDs (u32/u64) instead of copying values
- Lazy path materialization: only construct full paths for final results

## 6. Comparison with CodeQL's Approach

| Aspect | CodeQL | open-cql |
|--------|--------|----------|
| Core engine | Proprietary Datalog evaluator | Custom Datalog+flow engine |
| Flow tracking | QL library predicates walking flow graph | Native engine support |
| Path tracking | Reconstructed from flow graph | Tracked during evaluation |
| Storage | Custom column store | Sorted arrays + B-tree indexes |
| Compilation | QL → DIL → RA → native code | QL → HIR → MIR → LIR → Plan |
| Parallelism | Unknown (proprietary) | Rayon-based work stealing |

## 7. Open Research Questions

1. **Optimal context sensitivity depth**: How many call-site contexts to track
   before approximating? CodeQL uses a bounded approach.

2. **Access path length**: How many field dereferences to track precisely?
   After some depth, approximate with "any field of type T."

3. **Incremental analysis**: Can we re-evaluate queries when only part of
   the database changes? This requires understanding which strata are affected.

4. **Path deduplication**: Multiple paths may connect the same source-sink pair.
   Which paths to report? Shortest? Most representative?

5. **Monotonic aggregates**: How to handle aggregates inside recursive
   predicates (the `language[monotonicAggregates]` annotation)?
