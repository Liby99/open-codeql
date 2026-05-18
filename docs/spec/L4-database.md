# L4 — Database & Schema Contract

- **Layer:** L4
- **Implementation crates:** `ocql-database`, `ocql-schema`
- **Status:** Ratified for the on-disk format and value model; Provisional for the schema-extension rules in §6.
- **Version:** 0.1

## 1. Purpose

L4 specifies the **data model** that L3 evaluates against and that L4-X
extractors populate:

- The value space (§3).
- The relation model (§4).
- The on-disk format (§5).
- The `.dbscheme` schema language (§6).

L4 is the most stable layer in open-codeql: a change here ripples into
every extractor and every query.

## 2. Vocabulary

| Term | Meaning |
|------|---------|
| **Database** | An on-disk artifact produced by extraction; the input to L3. |
| **Relation** | A named, typed, set-valued collection of tuples. |
| **Tuple** | An ordered sequence of values. |
| **Schema** | A `.dbscheme` text describing relations, columns, and union types. |
| **Entity** | A value of an `@`-prefixed database type, identified by a 64-bit id. |
| **InternedString** | A handle into the database's per-database string table. |

## 3. Values

```
Value = Int(i64)
      | Float(f64)        // ordered-float total order
      | String(handle)
      | Bool(bool)
      | Entity(u64)
      | Null
```

Reference: `crates/ocql-database/src/value.rs`.

- All variants are `Copy`, `Hash`, `Ord`. `Float` uses the `ordered-float`
  total order so that NaN has a stable position.
- `String` values are *always* interned: a database with two identical
  string contents must have one handle for both.
- `Null` is the unit of "no value"; relations use it sparingly. Most
  tables are total (no nulls); columns that admit null must be declared
  so in the schema (§6).

## 4. Relations

A relation is `(name, columns, tuples)` where:

- `name` is a non-empty ASCII identifier matching `[a-z][a-zA-Z0-9_]*`.
- `columns` is a list of `(name, ColumnType)` pairs declared in the
  schema.
- `tuples` is a **set** of `Tuple`. Persisted as a sorted vector for
  cache-friendliness.

### 4.1 Column types

```
ColumnType = QlType + DbType + RefKind

QlType  = int | float | string | varchar(N) | boolean | date
DbType  = the QL type (primitive or @-entity)
RefKind = ref | (none)        // 'ref' means foreign-key reference
```

### 4.2 Indexes

The implementation lazily builds **multi-column indexes** on demand from
queries (`Database::lookup_each`, `lookup_any` in
`crates/ocql-database/src/database.rs`). Indexes are not persisted;
they are rebuilt on load.

Index keys are `SmallVec<[usize; 4]>` — a column-permutation. The same
relation may have multiple live indexes simultaneously.

### 4.3 Mutability

A database is **append-only during extraction** and **sealed for
evaluation**:

- An extractor may `insert` tuples into any relation it owns.
- L3 inserts new relations (one per evaluated predicate) but does not
  mutate base relations.
- After `save_to_file` the database is immutable.

## 5. On-disk format

The binding format is the custom binary in
`crates/ocql-database/src/serialize.rs`. The contract is:

- 4-byte magic `OCQL`.
- 1-byte format version (currently `1`). A loader **must** reject
  unknown versions.
- Little-endian throughout.
- Header → string table → relations.
- Each relation: name, column metadata, tuple count, tuples.
- Each tuple: `value-count`, then for each value a `tag` byte and the
  value bytes:

  | Tag | Variant | Payload |
  |-----|---------|---------|
  | 0   | Int     | i64 |
  | 1   | Float   | f64 |
  | 2   | String  | u32 handle |
  | 3   | Bool    | u8 |
  | 4   | Entity  | u64 |
  | 5   | Null    | – |

- The format is **not** stable across format-version bumps. A bump
  requires a migration tool in `scripts/`.

## 6. Schema language (`.dbscheme`)

Reference: `crates/ocql-schema/src/parse.rs`. The language is a
**subset** of CodeQL's `.dbscheme` syntax. The accepted productions are:

```
DbScheme = (Comment | Table | Union | Case)*

Table    = name '(' Column (',' Column)* ')'           Keyset*
Column   = ('unique')? QlType name ':' DbType ('ref')?
Keyset   = '#keyset' '[' name (',' name)* ']'

Union    = '@' name '=' DbType ('|' DbType)+
Case     = 'case' '@' name '.' name 'of' CaseArm+
CaseArm  = name '=' '@' name
```

Block comments nest. Line comments use `//`.

### 6.1 Conformance gap

The current parser supports:

- ✅ Tables, columns, keysets, primary-key uniqueness.
- ✅ Unions (`@container = @file | @folder`).
- ✅ Casetypes (`case @x.kind of …`).
- ⚠️ Multi-line keysets and complex foreign-key declarations are
  partially supported; see `crates/ocql-schema/src/parse.rs`.
- ❌ Annotations on relations (e.g. `@@deprecated`) — Provisional.

### 6.2 Schema as ground truth

The schema is the **single source of truth** for what an extractor must
emit. An extractor is conformant iff:

- It populates exactly the relations declared in its schema.
- Every emitted tuple satisfies the column types and `ref` constraints.
- Every `unique` column is unique across the relation's extent.
- Every `@`-entity id is assigned by exactly one relation that owns it
  (the "key relation" for that entity type).

Violations are extractor bugs, not query bugs.

## 7. Tying schema to extractor crates

Each language extractor crate (`ocql-extractor-*`) ships its schema as a
const string compiled into the binary (e.g.
`crates/ocql-extractor-java/src/schema.rs:11`). At database creation
time, `Database::from_schema` parses the string and seeds the database
with empty relations.

This means **the `.dbscheme` lives in Rust source, not on disk**, which
makes schema changes a Rust-rebuild rather than a config swap. The
trade-off is intentional: it keeps the schema and the emitter logic
co-located.

## 8. Migration policy

A change to L4 is governed by:

| Change | Requires |
|--------|----------|
| Add a relation to a schema | Extractor PR; no L4 bump. |
| Add a column to a relation | Extractor PR; databases must be re-extracted. |
| Change a column's type | Format-version bump; migration tool. |
| Change `Value` variants or tags | Format-version bump; migration tool. |
| Change the on-disk container format | Format-version bump. |

A format-version bump is announced by incrementing the byte after the
magic and shipping a `vN-to-vN+1` script under `scripts/migrations/`.

## 9. Conformance tests

`crates/ocql-database/tests/` (Provisional — to be created) holds:

- A round-trip test for every value variant.
- A round-trip test for a representative non-trivial schema (use
  `ocql-extractor-java`'s schema as the canonical fixture).
- A version-rejection test (loader must error on `magic + 0xff`).

## 10. Open questions

- **Persistent indexes (§4.2):** is the cost of rebuilding indexes on
  load worth the simpler on-disk format, or do we add an optional
  index sidecar?
- **`Null` discipline (§3):** should every nullable column be marked in
  the schema, or do we keep the current "infer from extractor" model?
- **Schema annotations (§6.1):** do we model `@@deprecated`,
  `@@cached`, `@@external` at L4 (extractor concern) or L1 (query
  concern)?
