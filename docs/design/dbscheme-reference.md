# Database Schema Reference

This document summarizes the CodeQL database schemas for C/C++ and Java,
based on the public `.dbscheme` files in the github/codeql repository.

## Schema Sources

- C++: `github/codeql/cpp/ql/lib/semmlecode.cpp.dbscheme`
- Java: `github/codeql/java/ql/lib/config/semmlecode.dbscheme`

## C/C++ Schema Overview

### File & Location Tables

| Table | Columns | Purpose |
|-------|---------|---------|
| `files` | id, name | Source file registry |
| `folders` | id, name | Directory registry |
| `locations_default` | id, file, beginLine, beginColumn, endLine, endColumn | Source spans |
| `sourceLocationPrefix` | prefix | Snapshot location root |

### Compilation Tables

| Table | Columns | Purpose |
|-------|---------|---------|
| `compilations` | id, cwd | Compiler invocations |
| `compilation_args` | compilation, num, arg | Compiler arguments |
| `compilation_compiling_files` | compilation, num, file | Files per compilation |
| `compilation_time` | compilation, num, kind, seconds | Timing metrics |
| `diagnostic_for` | diagnostic, compilation, fileNum, fileNumOk | Extraction diagnostics |

### Type Tables

| Table | Columns | Purpose |
|-------|---------|---------|
| `builtintypes` | id, name, kind, size, sign, alignment | void, int, float, char, etc. |
| `derivedtypes` | id, name, kind, type | Pointers, references, arrays |
| `usertypes` | id, name, kind | Classes, structs, unions, enums |
| `typedefbase` | id, type | Typedef targets |
| `decltypes` | id, expr, baseType | decltype() types |
| `type_operators` | id, arg, kind, baseType | Type trait operators |

#### Built-in Type Kinds
```
1: error_type, 2: unknown_type, 3: void,
4: boolean, 5: char, 6: unsigned_char, 7: signed_char,
8: short, 9: unsigned_short, 10: signed_short,
11: int, 12: unsigned_int, 13: signed_int,
14: long, 15: unsigned_long, 16: signed_long,
17: long_long, 18: unsigned_long_long, 19: signed_long_long,
20: __int8, 21: __int16, 22: __int32, 23: __int64, 24: __int128,
25: float, 26: double, 27: long_double,
28: complex_float, 29: complex_double, 30: complex_long_double,
31: imaginary_float, 32: imaginary_double, 33: imaginary_long_double,
34: wchar_t, 35: decltype_nullptr, 36: __int128,
37: __float128, 38: char16_t, 39: char32_t, 40: char8_t,
41: _Float32, 42: _Float32x, 43: _Float64, 44: _Float64x, 45: _Float128,
46: __float16, 47: _Float16, 48: __bf16, 49: std_float32_t, 50: std_float64_t
```

#### Derived Type Kinds
```
1: pointer, 2: reference (lvalue), 3: type_with_specifiers,
4: array, 5: gnu_vector, 6: routineptr (function pointer),
7: fnptr, 8: block (Objective-C), 9: decltype,
10: rvalue_reference
```

#### User Type Kinds
```
1: struct, 2: class, 3: union, 4: enum,
5: typedef, 6: template_parameter,
7: template_template_parameter,
8: proxy_class, 9: scoped_enum, 10: using_alias, 11: decltype
```

### Declaration Tables

| Table | Columns | Purpose |
|-------|---------|---------|
| `functions` | id, name, type | Function declarations |
| `function_entry_point` | id, entry_point | Function body entry |
| `function_return_type` | id, return_type | Return type |
| `purefunctions` | id | Pure virtual functions |
| `function_deleted` | id | = delete functions |
| `function_defaulted` | id | = default functions |
| `function_prototyped` | id | Has prototype |
| `deduction_guide_for_class` | id, class_template | Deduction guides |
| `coroutine` | id | Coroutine functions |
| `variables` | id, name, type | All variables |
| `enumconstants` | id, name, parent, index, type, value | Enum values |

#### Function Kinds
```
1: normal, 2: constructor, 3: destructor,
4: conversion, 5: operator,
6: builtin (compiler intrinsic)
```

### Expression Tables

| Table | Columns | Purpose |
|-------|---------|---------|
| `exprs` | id, kind, location | All expressions |
| `expr_types` | id, type, value_category | Expression types |
| `expr_reuse` | reuse, original, value_category | CSE info |

Selected expression kinds (89+ total):
```
1: literal, 2: unary_plus, 3: unary_minus,
4: ref_deref (dereference), 5: address_of,
6: complement (~), 7: logical_not (!),
8: prefix_incr, 9: prefix_decr,
10: postfix_incr, 11: postfix_decr,
20: add, 21: sub, 22: mul, 23: div, 24: rem,
25: lshift, 26: rshift,
27: bitwise_and, 28: bitwise_or, 29: bitwise_xor,
30: logical_and, 31: logical_or,
40: eq, 41: ne, 42: lt, 43: gt, 44: le, 45: ge,
50: assign, 51: assign_add, 52: assign_sub,
53: assign_mul, 54: assign_div, 55: assign_rem,
60: comma, 61: conditional (?:),
70: call, 71: member_call, 72: static_call,
80: static_cast, 81: reinterpret_cast,
82: const_cast, 83: dynamic_cast, 84: c_style_cast,
90: new, 91: new_array, 92: delete, 93: delete_array,
100: sizeof, 101: alignof,
110: this, 111: lambda,
120: field_access, 121: array_access,
```

### Statement Tables

| Table | Columns | Purpose |
|-------|---------|---------|
| `stmts` | id, kind, location | All statements |
| `stmt_parent` | stmt, index, parent | Statement tree |

Statement kinds:
```
1: expr_stmt, 2: block,
3: if_stmt, 4: while_stmt, 5: do_stmt,
6: for_stmt, 7: switch_stmt,
8: case_stmt, 9: default_stmt,
10: break, 11: continue, 12: return,
13: goto, 14: label,
15: try, 16: catch,
17: decl_stmt,
18: range_for (C++11),
19: handler (function-try),
20: constexpr_if (if constexpr),
```

### Relationship Tables

| Table | Columns | Purpose |
|-------|---------|---------|
| `derivations` | id, sub, super, index | Inheritance hierarchy |
| `overrides` | id, memberFunction | Virtual override |
| `member` | parent, index, child | Type membership |
| `enclosingfunction` | id, function | Nested scope |
| `funbind` | expr, fun | Call target binding |
| `varbind` | expr, var | Variable access binding |

---

## Java Schema Overview

### File & Location Tables
Same structure as C++ (files, folders, locations_default).

### Package & Type Tables

| Table | Columns | Purpose |
|-------|---------|---------|
| `packages` | id, name | Java packages |
| `classes_or_interfaces` | id, name, package, parent | Type declarations |
| `primitives` | id, name | boolean, byte, char, double, float, int, long, short |
| `arrays` | id, name, component, dimension | Array types |
| `typeVars` | id, name, pos, parent | Generic type parameters |
| `typeBounds` | id, type, pos, typevar | Type bounds (extends/super) |
| `wildcards` | id, name, kind | Wildcard types |

#### Type Kinds for classes_or_interfaces
```
interface, class, record, enum, annotation_type
```

### Member Tables

| Table | Columns | Purpose |
|-------|---------|---------|
| `fields` | id, name, type, parent | Field declarations |
| `methods` | id, name, result, type, parent | Method declarations |
| `constructors` | id, name, type, parent | Constructor declarations |
| `params` | id, type, index, callable, sourceDecl | Parameters |
| `exceptions` | id, type, callable | Thrown exceptions |
| `isVarargs` | callable | Varargs methods |

### Expression Table

89 expression kinds. Key categories:

```
// Literals (1-19)
1: array_init, 2: array_creation,
3: boolean_literal, 4: cast,
5: char_literal, 6: class_instance_creation,
7: conditional (?:), 8: double_literal,
9: float_literal, 10: int_literal,
11: long_literal, 12: null_literal,
13: string_literal, 14: this, 15: super,
16: type_literal (Foo.class), 17: lambda,
18: member_ref (method reference),

// Operators (20-39)
20: add, 21: and_bitwise, 22: and_logical,
23: assign, 24: div, 25: eq,
26: ge, 27: gt, 28: instanceof,
29: le, 30: lshift, 31: lt,
32: mod, 33: mul, 34: ne,
35: neg, 36: not_bitwise, 37: not_logical,
38: or_bitwise, 39: or_logical,

// More operators (40-59)
40: plus (unary +), 41: postdecr, 42: postincr,
43: predecr, 44: preincr, 45: rshift,
46: sub, 47: urshift, 48: xor,

// Compound assignments (49-60)
49: assign_add, 50: assign_and, 51: assign_div,
52: assign_lshift, 53: assign_mod, 54: assign_mul,
55: assign_or, 56: assign_rshift, 57: assign_sub,
58: assign_urshift, 59: assign_xor,

// Access (60-79)
60: array_access, 61: field_access,
62: method_access, 63: type_access,
64: variable_access, 65: package_access,
66: wildcard_type_access, 67: constructor_access,

// Special
70: class_expr (anonymous class),
80: when_expr (Kotlin), 81: when_branch,
85: not_null_expr (Kotlin !!),
86: safe_cast (Kotlin as?),
87: implicit_coercion,
88: implicit_not_null,
89: property_access (Kotlin),
```

### Statement Table

26 statement kinds:
```
1: block, 2: if, 3: for,
4: enhanced_for (for-each), 5: while,
6: do, 7: try, 8: switch,
9: synchronized, 10: return,
11: throw, 12: break, 13: continue,
14: empty, 15: expr_stmt,
16: local_var_decl, 17: local_type_decl,
18: assert, 19: labeled,
20: catch_clause, 21: case (switch case),
22: default_case,
23: yield, 24: switch_expr,
25: when_stmt (Kotlin), 26: when_branch_stmt (Kotlin),
```

### Binding Tables

| Table | Columns | Purpose |
|-------|---------|---------|
| `variableBinding` | expr, variable | Variable access → declaration |
| `callableBinding` | expr, callable | Method call → method declaration |

### Modifier Tables

| Table | Columns | Purpose |
|-------|---------|---------|
| `hasModifier` | element, modifier | Access modifiers |
| `isInterface` | type | Interface marker |
| `isRecord` | type | Record marker |
| `isEnumType` | type | Enum marker |
| `isAnnotType` | type | Annotation type marker |

### Kotlin-Specific Tables

| Table | Columns | Purpose |
|-------|---------|---------|
| `kt_nullable_types` | id, name, component | T? types |
| `kt_notnull_types` | id, name, component | T!! types |
| `kt_type_alias` | id, name, type | Type aliases |
| `ktProperties` | id, getter, setter, backingField, parameter | Properties |
| `ktExtensionFunctions` | id, type, receiverParameter | Extension functions |

## Key Observations for open-cql

1. **Expression kinds are language-specific**: C++ has ~130 kinds, Java has 89.
   Our extractors need complete coverage of all kinds.

2. **The schema defines the analysis surface**: If a table isn't populated,
   queries depending on it will produce no results.

3. **Binding tables are critical**: `funbind`/`callableBinding` and
   `varbind`/`variableBinding` connect references to declarations.
   Without these, data flow analysis cannot work.

4. **Control flow is implicit**: The schema doesn't have explicit CFG tables
   in the base schema. CFG is typically computed by QL library predicates
   from the statement/expression structure. Our engine needs to support
   this efficiently.

5. **Schema compatibility**: By parsing .dbscheme files directly, we can
   validate our extractors against the expected schema and potentially
   interop with CodeQL databases for testing.
