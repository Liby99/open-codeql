# C++ Schema Alignment Report

Comparison of our extractor schema (`crates/ocql-extractor-cpp/src/schema.rs`, the `CPP_DBSCHEME` constant) against the vendor CodeQL schema (`vendor/codeql/cpp/ql/lib/semmlecode.cpp.dbscheme`).

**Our schema**: 33 tables
**Vendor schema**: ~235 tables

---

## Part 1: Table-by-Table Comparison

### Tables That Match Vendor Exactly

| Table | Status | Notes |
|-------|--------|-------|
| `files` | MATCH | `(unique int id: @file, string name: string ref)` |
| `folders` | MATCH | `(unique int id: @folder, string name: string ref)` |
| `locations_default` | MATCH | `(unique int id, int file: @file ref, int beginLine, int beginColumn, int endLine, int endColumn)` |
| `functions` | MATCH | `(unique int id: @function, string name: string ref, int kind: int ref)` |
| `function_entry_point` | MATCH | `(int id: @function ref, unique int entry_point: @stmt ref)` |
| `namespaces` | MATCH | `(unique int id: @namespace, string name: string ref)` |
| `usertypes` | MATCH | `(unique int id: @usertype, string name: string ref, int kind: int ref)` |
| `stmts` | MATCH | `(unique int id: @stmt, int kind: int ref, int location: @location_default ref)` |
| `exprs` | MATCH | `(unique int id: @expr, int kind: int ref, int location: @location_default ref)` |
| `comments` | MATCH | `(unique int id: @comment, string contents: string ref, int location: @location_default ref)` |
| `while_body` | MATCH | `(unique int while_stmt, int body_id: @stmt ref)` |
| `do_body` | MATCH | `(unique int do_stmt, int body_id: @stmt ref)` |
| `switch_body` | MATCH | `(unique int switch_stmt, int body_id: @stmt ref)` |
| `enclosingfunction` | PARTIAL | See differences below |

### Tables With Column Differences

#### `function_return_type`
- **Ours**: `(int id: @function ref, string return_type: string ref)`
- **Vendor**: `(int id: @function ref, int return_type: @type ref)`
- **Difference**: We store return type as a **string**, vendor stores it as a **@type entity reference**. This is a fundamental difference -- the vendor has a full type system with entity IDs. Our string-based approach works for extraction but will not be compatible with QL library code that expects `@type` entities.

#### `params`
- **Ours**: `#keyset[id, index] (int id: @function ref, int index: int ref, string name: string ref, string param_type: string ref)`
- **Vendor**: `(int id: @parameter, int function: @parameterized_element ref, int index: int ref, int type_id: @type ref)`
- **Differences**:
  1. Vendor uses a `@parameter` entity as first column (each param has its own entity ID); ours uses `@function ref`
  2. Vendor references `@parameterized_element` (functions + block stmts + requires_expr); ours only references `@function`
  3. Vendor stores type as `@type ref`; ours stores as `string`
  4. Vendor has no `name` column (parameter names are in a separate `var_decls` table); ours includes `name`

#### `globalvariables`
- **Ours**: `(unique int id: @globalvariable, string name: string ref, string global_type: string ref)`
- **Vendor**: `#keyset[id, type_id] (int id: @globalvariable, int type_id: @type ref, string name: string ref)`
- **Differences**: Vendor stores type as `@type ref`, ours uses `string`. Column name differs (`type_id` vs `global_type`). Vendor uses `#keyset[id, type_id]`, ours uses `unique int id`.

#### `localvariables`
- **Ours**: `(unique int id: @localvariable, string name: string ref, string local_type: string ref)`
- **Vendor**: `#keyset[id, type_id] (int id: @localvariable, int type_id: @type ref, string name: string ref)`
- **Differences**: Same as globalvariables -- type as string vs entity ref, different column names.

#### `membervariables`
- **Ours**: `(unique int id: @membervariable, string name: string ref, string member_type: string ref)`
- **Vendor**: `#keyset[id, type_id] (int id: @membervariable, int type_id: @type ref, string name: string ref)`
- **Differences**: Same pattern as other variable tables.

#### `enumconstants`
- **Ours**: `(unique int id: @enumconstant, int parent: @usertype ref, int index: int ref, string name: string ref, int location: @location_default ref)`
- **Vendor**: `(unique int id: @enumconstant, int parent: @usertype ref, int index: int ref, int type_id: @type ref, string name: string ref, int location: @location_default ref)`
- **Difference**: Vendor has an additional `type_id: @type ref` column that we are missing.

#### `derivations`
- **Ours**: `#keyset[derived, index] (int derived: @usertype ref, int index: int ref, string base_name: string ref)`
- **Vendor**: `(unique int derivation: @derivation, int sub: @type ref, int index: int ref, int super: @type ref, int location: @location_default ref)`
- **Differences**: Completely different structure:
  1. Vendor gives each derivation its own entity ID (`@derivation`)
  2. Vendor uses `sub`/`super` (both `@type ref`); ours uses `derived`/`base_name` (string)
  3. Vendor includes a location; ours does not
  4. Column names differ

#### `member`
- **Ours**: `#keyset[parent, index] (int parent: @usertype ref, int index: int ref, int child: @element ref)`
- **Vendor**: `(int parent: @type ref, int index: int ref, int child: @member ref)`
- **Differences**: Vendor uses `@type ref` for parent and `@member ref` for child; ours uses `@usertype ref` and `@element ref`.

#### `namespacembrs`
- **Ours**: `(int parentid: @namespace ref, unique int memberid: @element ref)`
- **Vendor**: `(int parentid: @namespace ref, unique int memberid: @namespacembr ref)`
- **Difference**: Vendor uses `@namespacembr ref` (= `@declaration | @namespace`); ours uses `@element ref`.

#### `stmtparents`
- **Ours**: `(unique int id: @stmt ref, int index: int ref, int parent_id: @element ref)`
- **Vendor**: `(unique int id: @stmt ref, int index: int ref, int parent: @stmtparent ref)`
- **Difference**: Vendor uses `@stmtparent` (= `@stmt | @expr_stmt`); ours uses `@element ref`. Column name differs (`parent_id` vs `parent`).

#### `exprparents`
- **Ours**: `(int expr_id: @expr ref, int child_index: int ref, int parent_id: @element ref)`
- **Vendor**: `(int expr_id: @expr ref, int child_index: int ref, int parent_id: @exprparent ref)`
- **Difference**: Vendor uses `@exprparent` (= `@element`); functionally equivalent.

#### `if_then` / `if_else`
- **Ours**: `(unique int if_stmt: @stmt ref, int then_id/else_id: @stmt ref)`
- **Vendor**: `(unique int if_stmt: @stmt_if ref, int then_id/else_id: @stmt ref)`
- **Difference**: Vendor uses the more specific `@stmt_if` type; ours uses generic `@stmt`.

#### `for_body`
- **Ours**: `#keyset[for_stmt] (int for_stmt: @stmt ref, int body_id: @stmt ref)`
- **Vendor**: `(unique int for_stmt: @stmt_for ref, int body_id: @stmt ref)`
- **Difference**: Vendor uses `@stmt_for`; ours uses generic `@stmt`.

#### `includes`
- **Ours**: `(unique int id: @include, int file: @file ref, string included: string ref)`
- **Vendor**: `(unique int id: @ppd_include ref, int included: @file ref)`
- **Differences**:
  1. Vendor's `@ppd_include` is a union of include directive types
  2. Vendor's `included` is `@file ref`; ours stores the included path as a `string`
  3. Ours has an extra `file` column; vendor has a separate `preprocdirects` table for file/line info

#### `valuetext`
- **Ours**: `(unique int id: @expr ref, string text: string ref)`
- **Vendor**: `(unique int id: @value ref, string text: string ref)`
- **Difference**: Vendor uses `@value ref` (values are separate from exprs, linked via `valuebind`); ours directly uses `@expr ref`.

#### `enclosingfunction`
- **Ours**: `(unique int child: @element ref, int parent: @function ref)`
- **Vendor**: `(unique int child: @enclosingfunction_child ref, int parent: @function ref)`
- **Difference**: Vendor constrains child to `@enclosingfunction_child` (= `@usertype | @variable | @namespace`); ours uses generic `@element`.

### Tables Only In Our Schema (Not In Vendor)

| Table | Notes |
|-------|-------|
| `variables` | Combined variable table; vendor has no unified `variables` table, only `@variable` union type |
| `fields` | Struct/class fields; vendor handles this via `membervariables` + `member` |
| `element_location` | Location mapping for all elements; vendor uses inline locations per table |

### Tables In Vendor But NOT In Ours (Major Categories)

There are approximately 200 tables in the vendor schema that we do not implement. Below are the most important categories grouped by functionality:

#### Compilation Infrastructure (~15 tables)
- `compilations`, `compilation_args`, `compilation_expanded_args`, `compilation_finished`, `compilation_time`, `compilation_build_mode`, `compilation_compiling_files`
- `diagnostics`, `diagnostic_for`
- `extractor_version`, `trap_filename`, `databaseMetadata`

#### Type System (~25 tables)
- `builtintypes` -- fundamental types (int, float, char, etc.)
- `derivedtypes` -- pointers, references, arrays, etc.
- `routinetypes`, `routinetypeargs` -- function pointer types
- `ptrtomembers` -- pointer-to-member types
- `decltypes` -- decltype() expressions
- `typedefbase` -- typedef resolution
- `arraysizes` -- array dimensions
- `type_decls`, `type_def` -- type declarations/definitions
- `type_operators` -- type traits
- `type_mentions` -- where types are referenced
- `unspecifiedtype` -- type after stripping specifiers
- `usertypesize` -- sizeof for user types
- `is_complete` -- forward vs complete declarations
- `pointerishsize` -- pointer size info

#### Specifiers and Attributes (~8 tables)
- `specifiers`, `funspecifiers`, `varspecifiers`, `typespecifiers`, `derspecifiers`
- `attributes`, `attribute_args`, `attribute_arg_*` (constant, expr, name, type, value)
- `funcattributes`, `varattributes`, `typeattributes`, `stmtattributes`, `namespaceattributes`

#### Function Details (~20 tables)
- `fun_decls` -- function declaration entries
- `fun_def`, `fun_specialized`, `fun_implicit` -- function properties
- `fun_decl_specifiers`, `fun_decl_typedef_type`
- `fun_decl_throws`, `fun_decl_noexcept`, `fun_decl_empty_throws`, `fun_decl_empty_noexcept`
- `fun_requires` -- requires clauses
- `function_instantiation` -- template instantiation
- `function_deleted`, `function_prototyped`, `function_defaulted`
- `builtin_functions`, `purefunctions`
- `overrides` -- virtual function overrides
- `deduction_guide_for_class`
- `explicit_specifier_exprs`
- `coroutine`, `coroutine_*`
- `mangled_name`
- `lambdas`, `lambda_capture`

#### Variable Details (~10 tables)
- `var_decls`, `var_def`, `var_specialized`
- `var_decl_specifiers`, `var_requires`
- `varbind`, `variable_instantiation`
- `is_structured_binding`
- `autoderivation`
- `fieldoffsets`, `bitfield`

#### Expression Details (~30 tables)
- `expr_types` -- expression types
- `expr_reuse`, `expr_isload`
- `exprconv` -- implicit conversions
- `conversionkinds`
- `iscall` -- expression is a function call
- `funbind` -- call target binding
- `initialisers` -- initializer expressions
- `aggregate_field_init`, `aggregate_array_init`
- `braced_initialisers`
- `expr_cond_true`, `expr_cond_false`, `expr_cond_guard`
- `new_allocated_type`, `new_array_allocated_type`
- `expr_allocator`, `expr_deallocator`
- `sizeof_bind`, `typeid_bind`, `uuidof_bind`
- `fold` -- fold expressions
- `condition_decl_bind`
- `namequalifiers`
- `values`, `valuebind` -- literal values

#### Statement Details (~15 tables)
- `stmt_decl_bind`, `stmt_decl_entry_bind`
- `if_initialization`, `switch_initialization`, `for_initialization`, `for_condition`, `for_update`
- `constexpr_if_*` (then, else, initialization)
- `consteval_if_*`
- `switch_case`
- `ishandler`, `jumpinfo`
- `blockscope` -- block/function scope relationships
- `stmtattributes`
- `synthetic_destructor_call`

#### Template System (~15 tables)
- `is_class_template`, `is_function_template`, `is_variable_template`
- `class_template_argument`, `class_template_argument_value`
- `function_template_argument`, `function_template_argument_value`
- `variable_template_argument`, `variable_template_argument_value`
- `template_template_argument`, `template_template_argument_value`
- `template_template_instantiation`
- `class_instantiation`
- `numtemplatearguments`
- `concept_templates`, `concept_instantiation`, `concept_template_argument`, `concept_template_argument_value`
- `is_type_constraint`, `type_template_type_constraint`
- `nontype_template_parameters`

#### Preprocessor (~10 tables)
- `preprocdirects` -- preprocessor directives
- `preproctext`, `preprocpair`, `preproctrue`, `preprocfalse`
- `macroinvocations`, `macrolocationbind`, `macroparent`
- `macro_argument_expanded`, `macro_argument_unexpanded`
- `inmacroexpansion`, `affectedbymacroexpansion`
- `embeds`

#### Access Control and Linkage (~5 tables)
- `memberaccess` -- public/private/protected
- `frienddecls` -- friend declarations
- `link_targets`, `link_parent`
- `using_container`, `usings`
- `namespace_inline`, `namespace_decls`

#### Lines of Code (~1 table)
- `numlines` -- lines of code / comments / blank per element

#### Source Location (~3 tables)
- `containerparent`, `sourceLocationPrefix`, `fileannotations`

#### Misc (~5 tables)
- `commentbinding` -- comments attached to elements
- `compgenerated` -- compiler-generated elements
- `param_decl_bind` -- parameter declaration entries
- `static_asserts`
- `member_function_this_type`
- `externalData`

---

## Part 2: Database Tables Referenced by Core Library Classes

### Element.qll
Database tables directly referenced:
- `blockscope` -- MISSING from our schema
- `using_container` -- MISSING
- `affectedbymacroexpansion` -- MISSING
- `enclosingfunction` -- HAVE (minor type diff)
- `enumconstants` -- HAVE (missing type_id column)
- `derivations` -- HAVE (column structure differs significantly)
- `stmtparents` -- HAVE (minor type diff)
- `exprparents` -- HAVE (functionally equivalent)
- `namequalifiers` -- MISSING
- `initialisers` -- MISSING
- `exprconv` -- MISSING
- `param_decl_bind` -- MISSING
- `static_asserts` -- MISSING
- `var_decls` -- MISSING
- `is_class_template` -- MISSING
- `is_function_template` -- MISSING
- `is_variable_template` -- MISSING

**Missing for Element.qll**: 11 of 17 tables

### Function.qll
Database tables directly referenced:
- `functions` -- HAVE (match)
- `function_return_type` -- HAVE (type as string vs entity)
- `function_entry_point` -- HAVE (match)
- `params` -- HAVE (significant column differences)
- `funspecifiers` -- MISSING
- `funcattributes` -- MISSING
- `compgenerated` -- MISSING
- `function_deleted` -- MISSING
- `function_prototyped` -- MISSING
- `function_defaulted` -- MISSING
- `explicit_specifier_exprs` -- MISSING
- `fun_decls` -- MISSING
- `fun_def` -- MISSING
- `fun_specialized` -- MISSING
- `fun_implicit` -- MISSING
- `fun_decl_specifiers` -- MISSING
- `fun_decl_typedef_type` -- MISSING
- `fun_decl_throws` -- MISSING
- `fun_decl_noexcept` -- MISSING
- `fun_decl_empty_throws` -- MISSING
- `fun_decl_empty_noexcept` -- MISSING
- `fun_requires` -- MISSING
- `function_instantiation` -- MISSING
- `builtin_functions` -- MISSING
- `deduction_guide_for_class` -- MISSING
- `is_function_template` -- MISSING

**Missing for Function.qll**: 22 of 26 tables

### File.qll
Database tables directly referenced:
- `files` -- HAVE (match)
- `folders` -- HAVE (match)
- `containerparent` -- MISSING
- `fileannotations` -- MISSING
- `numlines` -- MISSING

**Missing for File.qll**: 3 of 5 tables

### Declaration.qll
Database tables directly referenced:
- `class_template_argument` -- MISSING
- `function_template_argument` -- MISSING
- `variable_template_argument` -- MISSING
- `template_template_argument` -- MISSING
- `concept_template_argument` -- MISSING
- `class_template_argument_value` -- MISSING
- `function_template_argument_value` -- MISSING
- `variable_template_argument_value` -- MISSING
- `template_template_argument_value` -- MISSING
- `concept_template_argument_value` -- MISSING

**Missing for Declaration.qll**: 10 of 10 tables

---

## Part 3: Table Name Mapping / Mismatches

### Names that Match
The vendor uses table names like `functions(...)`, `files(...)`, `stmts(...)` and entity types like `@function`, `@file`, `@stmt`. Our schema uses **exactly the same names** for tables and entity types. There is no translation layer needed for table/entity names.

### Structural Mismatches (Not Name Mismatches)
The mismatches are not in naming but in **column structure**:

1. **Type references**: The vendor's type system is entity-based (`@type ref`). Our schema uses `string ref` for type columns (e.g., `function_return_type`, `params`, `*variables`). This means:
   - The vendor's `function_return_type` stores an entity ID pointing to the `@type` hierarchy
   - Ours stores a human-readable type string like `"int"` or `"std::string"`
   - QL library code like `Function.getType()` expects `@type ref`, not `string`

2. **Parameter entity IDs**: The vendor gives each parameter its own entity ID (`@parameter`), while ours uses the function ID as the key.

3. **Derivation entity IDs**: The vendor gives each derivation its own entity ID (`@derivation`), while ours does not.

4. **Value indirection**: The vendor separates values from expressions (`@value` + `valuebind`), while we bind `valuetext` directly to `@expr`.

5. **Include paths**: The vendor resolves includes to `@file ref`, while we store the include path as `string`.

### Tables We Have That Vendor Does Not
- `variables` -- our unified variable table (vendor only has the subtypes)
- `fields` -- our struct field table (vendor uses `membervariables` + `member`)
- `element_location` -- our global location mapping (vendor has per-table locations)

---

## Part 4: String/Int Builtin Methods

### Current Engine State
The engine (`crates/ocql-engine/src/eval.rs` and `rule.rs`) has **no builtin predicate/method support**. It evaluates:
- Relational atoms (positive and negated)
- Guards (comparison operators: =, !=, <, <=, >, >=)
- Arithmetic assignments (add, sub, mul, div, mod)
- Aggregates (count, sum, min, max)

There is no mechanism for calling methods on values (e.g., `x.toString()`, `s.length()`).

### Builtins Needed for Basic Queries

Real CodeQL queries extensively use builtin methods on primitive types. The key ones are:

#### int builtins
| Method | Signature | Description |
|--------|-----------|-------------|
| `toString()` | `int -> string` | Convert integer to string representation |
| `abs()` | `int -> int` | Absolute value |
| `maximum(other)` | `(int, int) -> int` | Max of two integers |
| `minimum(other)` | `(int, int) -> int` | Min of two integers |

#### float builtins
| Method | Signature | Description |
|--------|-----------|-------------|
| `toString()` | `float -> string` | Convert float to string |
| `abs()` | `float -> float` | Absolute value |
| `floor()` | `float -> int` | Floor |
| `ceil()` | `float -> int` | Ceiling |

#### string builtins (most commonly used)
| Method | Signature | Description |
|--------|-----------|-------------|
| `length()` | `string -> int` | String length |
| `charAt(i)` | `(string, int) -> string` | Character at index |
| `indexOf(sub)` | `(string, string) -> int` | Find substring index |
| `substring(start, end)` | `(string, int, int) -> string` | Substring extraction |
| `prefix(n)` | `(string, int) -> string` | First n characters |
| `suffix(n)` | `(string, int) -> string` | Last n characters |
| `matches(pattern)` | `(string, string) -> boolean` | Glob-style matching (`%` = wildcard) |
| `regexpMatch(pattern)` | `(string, string) -> boolean` | Regex matching |
| `regexpFind(pattern, i, n)` | `(string, string, int, int) -> string` | Regex find |
| `regexpReplaceAll(pattern, rep)` | `(string, string, string) -> string` | Regex replace |
| `splitAt(delim, n)` | `(string, string, int) -> string` | Split and get nth part |
| `trim()` | `string -> string` | Trim whitespace |
| `toLowerCase()` | `string -> string` | To lowercase |
| `toUpperCase()` | `string -> string` | To uppercase |
| `toInt()` | `string -> int` | Parse integer |
| `toFloat()` | `string -> float` | Parse float |
| `replaceAll(old, new)` | `(string, string, string) -> string` | Replace all occurrences |

#### boolean builtins
| Method | Signature | Description |
|--------|-----------|-------------|
| `toString()` | `boolean -> string` | "true" or "false" |

### What Simple Queries Need

For `ExtractedFiles.ql` (select files from database):
- `string.toString()` -- for result display
- `File.getAbsolutePath()` references `files(id, name)` -- just returns `name`

For `DeadCodeFunction.ql` (functions never called):
- `Function.getName()` references `functions(id, name, kind)` -- just returns `name`
- `Function.hasEntryPoint()` references `function_entry_point`
- Needs negation (already supported) and basic join logic

For more realistic queries:
- `string.matches(pattern)` is very common for filtering paths, names
- `int.toString()` is needed whenever displaying numeric results
- `string.length()` for string length checks
- `string.indexOf(sub)` and `string.prefix/suffix` for path manipulation

### How the Engine Would Need to Change

#### Option A: Builtin Function Table (Recommended)
Add a `BuiltinFunction` concept to the rule/eval layer:

```
enum BuiltinCall {
    ToString(Term),           // x.toString()
    StringLength(Term),       // s.length()
    StringMatches(Term, Term), // s.matches(pattern)
    StringIndexOf(Term, Term), // s.indexOf(sub)
    StringPrefix(Term, Term),  // s.prefix(n)
    StringSuffix(Term, Term),  // s.suffix(n)
    IntAbs(Term),             // i.abs()
    // ...
}
```

Add a new `BodyElement::BuiltinAssign` variant:
```
BodyElement::BuiltinAssign {
    result_var: String,
    call: BuiltinCall,
}
```

Or for predicate-style builtins (like `matches` which returns boolean):
```
BodyElement::BuiltinFilter {
    call: BuiltinCall,
}
```

During evaluation, handle these by computing the result from the bound values.

#### Option B: Virtual Relations (Alternative)
Register builtin operations as virtual relations that are computed on-demand rather than stored. For example, `string_length(s, n)` would be a virtual relation where `n = s.length()`. This fits naturally into the Datalog model but requires lazy evaluation.

#### Recommendation
Option A is simpler and sufficient for the near term. The QL compiler (MIR/LIR) would lower method calls like `s.length()` into `BuiltinAssign` body elements during rule generation. This keeps the engine simple while supporting the most common builtins.

---

## Summary

### Key Findings

1. **Table names match** -- there is no naming translation needed between our schema and the vendor's.

2. **Column types diverge on types** -- the biggest gap is that our schema stores types as strings while the vendor uses entity references into a rich `@type` hierarchy. This affects `function_return_type`, `params`, all `*variables` tables, `enumconstants`, and `derivations`.

3. **~200 tables missing** -- we implement about 33 tables out of ~235. The most impactful missing categories for running real queries are:
   - Type system (`builtintypes`, `derivedtypes`, `typedefbase`, etc.) -- needed for any type-aware query
   - Function details (`fun_decls`, `overrides`, `function_instantiation`) -- needed for Function.qll
   - Expression details (`expr_types`, `exprconv`, `iscall`, `funbind`) -- needed for call graph analysis
   - Preprocessor (`macroinvocations`, `preprocdirects`) -- needed for macro-aware queries

4. **No builtin method support** in the engine -- `toString()`, `matches()`, `length()` etc. are not yet supported. These are needed for virtually all real CodeQL queries.

5. **Our custom tables** (`variables`, `fields`, `element_location`) are not in the vendor schema and would need to either be kept as internal conveniences or mapped onto vendor-compatible structures.

### Priority Recommendations

For running basic queries against our extracted databases:
1. The engine builtin methods are the most critical gap (especially `toString` and `matches`)
2. The `containerparent` and `numlines` tables for File.qll support
3. The type system entity model (converting string types to `@type` entities) for any type-aware query
4. `fun_decls` and related tables for full Function class support
