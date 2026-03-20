/// A parsed .dbscheme file.
#[derive(Debug, Clone, PartialEq)]
pub struct DbScheme {
    pub entries: Vec<Entry>,
}

impl DbScheme {
    pub fn tables(&self) -> impl Iterator<Item = &Table> {
        self.entries.iter().filter_map(|e| match e {
            Entry::Table(t) => Some(t),
            _ => None,
        })
    }

    pub fn unions(&self) -> impl Iterator<Item = &UnionType> {
        self.entries.iter().filter_map(|e| match e {
            Entry::Union(u) => Some(u),
            _ => None,
        })
    }

    pub fn cases(&self) -> impl Iterator<Item = &CaseType> {
        self.entries.iter().filter_map(|e| match e {
            Entry::Case(c) => Some(c),
            _ => None,
        })
    }
}

/// A top-level entry in a .dbscheme file.
#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    Table(Table),
    Union(UnionType),
    Case(CaseType),
}

/// A table definition, e.g.:
/// ```text
/// #keyset[id, num]
/// compilation_args(
///     int id : @compilation ref,
///     int num : int ref,
///     string arg : string ref
/// );
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
    pub keysets: Vec<Vec<String>>,
}

/// A single column in a table definition.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub name: String,
    pub col_type: ColumnType,
    pub db_type: DbType,
    pub is_unique: bool,
    pub is_ref: bool,
}

/// The QL-level type of a column (the left side of `:`).
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnType {
    Int,
    Float,
    String,
    Varchar(u32),
    Boolean,
    Date,
}

/// The database-level type of a column (the right side of `:`).
#[derive(Debug, Clone, PartialEq)]
pub enum DbType {
    Int,
    Float,
    String,
    Boolean,
    Date,
    Entity(String),
}

/// A union type definition, e.g.:
/// ```text
/// @container = @file | @folder
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct UnionType {
    pub name: String,
    pub variants: Vec<String>,
}

/// A case/enum block, e.g.:
/// ```text
/// case @macroinvocation.kind of
///   1 = @macro_expansion
/// | 2 = @other_macro_reference
/// ;
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CaseType {
    pub entity: String,
    pub column: String,
    pub variants: Vec<CaseVariant>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaseVariant {
    pub value: i64,
    pub entity_type: String,
}
