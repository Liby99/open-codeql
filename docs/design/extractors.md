# Extractor Design

This document details the design of language-specific extractors for open-cql.
Extractors transform source code into relational databases that QL queries run against.

## 1. Extraction Pipeline

```
Source Files
    │
    ▼
┌──────────────┐
│  tree-sitter  │ ←── Language grammar
│   parsing     │
└──────┬───────┘
       │ CST (Concrete Syntax Tree)
       ▼
┌──────────────┐
│   AST/Type    │ ←── Symbol table, type resolution
│   Analysis    │
└──────┬───────┘
       │ Typed AST
       ▼
┌──────────────┐
│    Fact       │ ←── .dbscheme (target schema)
│   Emission    │
└──────┬───────┘
       │ TRAP-like records
       ▼
┌──────────────┐
│   Database    │
│   Builder     │
└──────┬───────┘
       │
       ▼
  open-cql Database
```

## 2. Database Schema Compatibility

We parse CodeQL's `.dbscheme` files directly to understand the target schema.
This lets us:
- Validate that our extractors produce the right tables
- Potentially read CodeQL-generated databases for testing
- Stay aligned with the expected relational structure

### .dbscheme Format

```
// Entity type declarations
@file = @file_default;

// Table declarations
files(
    unique int id: @file,
    string name: string ref
);

locations_default(
    unique int id: @location_default,
    int file: @file ref,
    int beginLine: int ref,
    int beginColumn: int ref,
    int endLine: int ref,
    int endColumn: int ref
);

// Case/union types
@expr = @literal
      | @unary_expr
      | @binary_expr
      | @call_expr
      | ...;
```

### Schema Parser

```rust
struct DbScheme {
    entity_types: HashMap<String, EntityType>,
    tables: HashMap<String, Table>,
    unions: HashMap<String, Vec<String>>,
}

struct Table {
    name: String,
    columns: Vec<Column>,
    key_columns: Vec<usize>,
}

struct Column {
    name: String,
    col_type: ColumnType,
    is_unique: bool,
    is_ref: bool,
}

enum ColumnType {
    Int,
    Float,
    String,
    Boolean,
    Entity(String),  // Reference to @entity_type
}
```

## 3. C/C++ Extractor

### 3.1 Challenges

C/C++ is the most complex language to extract due to:
- **Preprocessor**: Macros, conditional compilation, includes
- **Templates**: Template instantiation, specialization, SFINAE
- **Overload resolution**: Complex rules for function overloading
- **Name lookup**: ADL, qualified lookup, dependent names
- **Multiple translation units**: Each .cpp file is compiled independently

### 3.2 Strategy

**Phase 1 (MVP):** tree-sitter for AST extraction, limited type resolution
- Parse with `tree-sitter-cpp`
- Extract AST structure (classes, functions, statements, expressions)
- Basic type resolution for simple cases
- Skip template instantiation initially

**Phase 2:** Integrate with clang for semantic analysis
- Use `libclang` or `clang-sys` for type checking and name resolution
- Clang provides accurate AST with full semantic information
- Map clang AST nodes to our relational schema

### 3.3 Key Tables to Populate

From `semmlecode.cpp.dbscheme`, the essential tables:

**File system:**
- `files(id, name)` — Source files
- `folders(id, name)` — Directories
- `locations_default(id, file, beginLine, beginColumn, endLine, endColumn)`

**Types:**
- `builtintypes(id, name, kind, size, sign, alignment)` — Primitive types
- `derivedtypes(id, name, kind, type)` — Pointers, references, arrays
- `usertypes(id, name, kind)` — Classes, structs, unions, enums

**Declarations:**
- `functions(id, name, type, kind)` — Function declarations
- `function_entry_point(id, stmt)` — Function body entry
- `variables(id, name, type, kind)` — Variable declarations
- `fieldoffsets(id, type, offset, bitoffset)` — Field layouts

**Expressions (89+ kinds):**
- `exprs(id, kind, type, location)` — All expressions
- `expr_types(expr, type, value_category)` — Expression type info

**Statements (26+ kinds):**
- `stmts(id, kind, location)` — All statements
- `stmt_parent(stmt, index, parent)` — Statement tree

**Relationships:**
- `derivations(id, sub, super, index)` — Inheritance
- `overrides(id, base, derived)` — Virtual override
- `funbind(expr, function)` — Call target binding

### 3.4 tree-sitter to Relations Mapping

Example: extracting a function declaration

```cpp
int factorial(int n) {
    if (n <= 1) return 1;
    return n * factorial(n - 1);
}
```

tree-sitter CST:
```
(function_definition
  type: (primitive_type)             → "int"
  declarator: (function_declarator
    declarator: (identifier)         → "factorial"
    parameters: (parameter_list
      (parameter_declaration
        type: (primitive_type)       → "int"
        declarator: (identifier))))  → "n"
  body: (compound_statement ...))
```

Emitted relations:
```
files(1, "example.cpp")
locations_default(1, 1, 1, 1, 4, 1)
functions(1, "factorial", 100, "normal")  -- type 100 = int(int)
variables(1, "n", 101, "parameter")       -- type 101 = int
stmts(1, "if_stmt", ...)
stmts(2, "return_stmt", ...)
stmts(3, "return_stmt", ...)
exprs(1, "le_expr", ...)                  -- n <= 1
exprs(2, "literal", ...)                  -- 1
exprs(3, "call_expr", ...)               -- factorial(n - 1)
...
```

## 4. Java Extractor

### 4.1 Advantages over C++

- No preprocessor
- Clear package/import structure
- Generics are simpler than templates (type erasure)
- Well-defined name resolution
- Single compilation unit per file (mostly)

### 4.2 Strategy

**Phase 1 (MVP):** tree-sitter for AST extraction
- Parse with `tree-sitter-java`
- Extract full AST including type declarations, methods, fields
- Basic type resolution using import declarations
- Handle generic types at the surface level

**Phase 2:** Deeper semantic analysis
- Full type resolution including generics
- Method overload resolution
- Annotation processing
- Lambda expression desugaring

### 4.3 Key Tables to Populate

From `semmlecode.dbscheme` (Java):

**Packages and types:**
- `packages(id, name)` — Java packages
- `classes_or_interfaces(id, name, package, kind)` — Type declarations
- `primitives(id, name)` — Primitive types
- `arrays(id, name, component_type)` — Array types

**Members:**
- `fields(id, name, type, parent)` — Field declarations
- `methods(id, name, signature, type, parent)` — Method declarations
- `constructors(id, name, signature, parent)` — Constructor declarations
- `params(id, type, index, callable)` — Method/constructor parameters

**Expressions and statements:**
- `exprs(id, kind, type, parent, index)` — 89 expression kinds
- `stmts(id, kind, parent, index)` — 26 statement kinds
- `variableBinding(expr, variable)` — Variable reference resolution
- `callableBinding(expr, callable)` — Method call resolution

**Generics:**
- `typeVars(id, name, pos, parent)` — Type parameters
- `typeBounds(id, type, pos, typevar)` — Type bounds
- `wildcards(id, name, kind)` — Wildcard types

### 4.4 Control Flow Graph Construction

Both extractors need to construct control flow graphs for the database.
The CFG is essential for data flow analysis.

```rust
struct CfgBuilder {
    nodes: Vec<CfgNode>,
    edges: Vec<(CfgNodeId, CfgNodeId, CfgEdgeKind)>,
}

enum CfgNode {
    Entry(FunctionId),
    Exit(FunctionId),
    Expr(ExprId),
    Stmt(StmtId),
    Guard(ExprId, bool),    // Branch condition (true/false arm)
}

enum CfgEdgeKind {
    Normal,
    True,        // Condition is true
    False,       // Condition is false
    Exception,   // Exception thrown
    Break,
    Continue,
    Return,
}
```

CFG construction rules:
- Sequential statements: edge from end of S1 to start of S2
- If statement: edges from condition to true/false branches, both converge after
- While/for: edge from condition to body (true) and exit (false), back-edge from body end to condition
- Try/catch: edges from any statement to catch handlers
- Return/break/continue: edge to function exit / loop exit / loop header

## 5. Shared Extractor Infrastructure

### 5.1 Entity ID Allocation

All extractors share a common ID allocation scheme:
```rust
struct IdAllocator {
    next_id: AtomicU64,
}

impl IdAllocator {
    fn alloc(&self) -> EntityId {
        EntityId(self.next_id.fetch_add(1, Ordering::Relaxed))
    }
}
```

### 5.2 Fact Emission

Extractors emit facts in a common intermediate format:
```rust
struct Fact {
    table: TableName,
    values: Vec<Value>,
}

trait FactEmitter {
    fn emit(&mut self, fact: Fact);
    fn emit_location(&mut self, entity: EntityId, file: FileId,
                     start_line: u32, start_col: u32,
                     end_line: u32, end_col: u32);
}
```

### 5.3 Source Archive

The database includes a copy of the analyzed source files for displaying
results in context:
```
database/source/
├── src/
│   ├── main.cpp
│   ├── utils.h
│   └── ...
└── lib/
    └── ...
```

## 6. Testing Extractors

### 6.1 Small test programs
Create minimal C++/Java programs that exercise specific language features.
Extract them and verify the resulting database has the expected relations.

### 6.2 Comparison with CodeQL
For programs where we can run both CodeQL and open-cql:
1. Extract with both tools
2. Compare the resulting databases (modulo ID differences)
3. Run the same queries and compare results

### 6.3 tree-sitter coverage
Verify that tree-sitter can parse all our test programs correctly.
Track any parsing failures or inaccuracies.
