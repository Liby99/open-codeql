use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use smallvec::SmallVec;

use crate::Value;
use ocql_schema::ColumnType;

/// A tuple is a fixed-size row of values.
/// SmallVec avoids heap allocation for small tuples (most are 2-6 columns).
pub type Tuple = SmallVec<[Value; 6]>;

/// Schema for a single column in a relation.
#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub col_type: ColumnType,
}

/// Schema describing a relation's structure.
#[derive(Debug, Clone)]
pub struct RelationSchema {
    pub name: String,
    pub columns: Vec<ColumnDef>,
}

/// Hash wrapper for Tuple (SmallVec doesn't implement Hash via HashSet,
/// but our Value type does implement Hash).
#[derive(Clone, Debug, PartialEq, Eq)]
struct TupleKey(Tuple);

impl Hash for TupleKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for v in &self.0 {
            v.hash(state);
        }
    }
}

/// An index on one or more columns for fast lookup.
struct Index {
    /// Which columns are indexed (by position).
    columns: Vec<usize>,
    /// Map from indexed column values → set of matching tuple indices.
    data: HashMap<SmallVec<[Value; 4]>, Vec<usize>>,
}

/// A relation is a named set of tuples with a schema.
///
/// Tuples are stored in a dense Vec for cache-friendly iteration.
/// A HashSet tracks membership for O(1) dedup on insert.
/// Indexes are lazily built via RefCell so that `lookup` works with `&self`.
pub struct Relation {
    pub schema: RelationSchema,
    /// Dense storage of all tuples (append-only).
    tuples: Vec<Tuple>,
    /// Membership set for O(1) dedup.
    membership: HashSet<TupleKey>,
    /// Lazily-built indexes. Uses RefCell so indexes can be built during
    /// immutable access (common pattern for caches).
    indexes: RefCell<Vec<Index>>,
}

impl Relation {
    /// Create an empty relation with the given schema.
    pub fn new(schema: RelationSchema) -> Self {
        Self {
            schema,
            tuples: Vec::new(),
            membership: HashSet::new(),
            indexes: RefCell::new(Vec::new()),
        }
    }

    /// Insert a tuple. Returns true if it was newly inserted (not a duplicate).
    pub fn insert(&mut self, tuple: Tuple) -> bool {
        if !self.membership.insert(TupleKey(tuple.clone())) {
            return false;
        }
        let idx = self.tuples.len();
        self.tuples.push(tuple.clone());
        // Update all existing indexes (get_mut avoids RefCell overhead)
        for index in self.indexes.get_mut().iter_mut() {
            let max_col = index.columns.iter().copied().max().unwrap_or(0);
            if tuple.len() <= max_col {
                continue; // skip indexes on columns wider than this tuple
            }
            let key: SmallVec<[Value; 4]> = index.columns.iter().map(|&c| tuple[c].clone()).collect();
            index.data.entry(key).or_default().push(idx);
        }
        true
    }

    /// Number of tuples in the relation.
    pub fn len(&self) -> usize {
        self.tuples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tuples.is_empty()
    }

    /// Iterate all tuples.
    pub fn scan(&self) -> impl Iterator<Item = &Tuple> {
        self.tuples.iter()
    }

    /// Look up tuples matching a key on the given columns.
    /// Lazily builds the index if needed (interior mutability via RefCell).
    ///
    /// For each matching tuple, calls `f(tuple)`. This closure-based API
    /// avoids lifetime issues with the RefCell borrow.
    pub fn lookup_each<F>(&self, columns: &[usize], key: &[Value], mut f: F)
    where
        F: FnMut(&Tuple),
    {
        self.ensure_index(columns);
        let indexes = self.indexes.borrow();
        let idx = indexes.iter().find(|i| i.columns == columns).unwrap();
        let search_key: SmallVec<[Value; 4]> = key.iter().cloned().collect();
        if let Some(tuple_indices) = idx.data.get(&search_key) {
            for &ti in tuple_indices {
                f(&self.tuples[ti]);
            }
        }
    }

    /// Check if any tuple matches the given key on the given columns.
    /// More efficient than `lookup_each` when you only need existence.
    pub fn lookup_any(&self, columns: &[usize], key: &[Value]) -> bool {
        self.ensure_index(columns);
        let indexes = self.indexes.borrow();
        let idx = indexes.iter().find(|i| i.columns == columns).unwrap();
        let search_key: SmallVec<[Value; 4]> = key.iter().cloned().collect();
        idx.data.contains_key(&search_key)
    }

    /// Ensure an index exists for the given columns (lazy, idempotent).
    fn ensure_index(&self, columns: &[usize]) {
        // Fast path: check if index already exists
        {
            let indexes = self.indexes.borrow();
            if indexes.iter().any(|i| i.columns == columns) {
                return;
            }
        }

        // Build new index from all existing tuples
        let max_col = columns.iter().copied().max().unwrap_or(0);
        let mut data: HashMap<SmallVec<[Value; 4]>, Vec<usize>> = HashMap::new();
        for (i, tuple) in self.tuples.iter().enumerate() {
            if tuple.len() <= max_col {
                continue; // skip tuples that are too short for this index
            }
            let key: SmallVec<[Value; 4]> = columns.iter().map(|&c| tuple[c].clone()).collect();
            data.entry(key).or_default().push(i);
        }

        self.indexes.borrow_mut().push(Index {
            columns: columns.to_vec(),
            data,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smallvec::smallvec;

    fn make_schema() -> RelationSchema {
        RelationSchema {
            name: "test".to_string(),
            columns: vec![
                ColumnDef { name: "id".to_string(), col_type: ColumnType::Int },
                ColumnDef { name: "name".to_string(), col_type: ColumnType::String },
            ],
        }
    }

    #[test]
    fn test_insert_and_scan() {
        let mut rel = Relation::new(make_schema());
        assert!(rel.insert(smallvec![Value::Int(1), Value::Int(100)]));
        assert!(rel.insert(smallvec![Value::Int(2), Value::Int(200)]));
        // Duplicate
        assert!(!rel.insert(smallvec![Value::Int(1), Value::Int(100)]));

        assert_eq!(rel.len(), 2);
    }

    #[test]
    fn test_lookup_each() {
        let mut rel = Relation::new(make_schema());
        rel.insert(smallvec![Value::Int(1), Value::Int(100)]);
        rel.insert(smallvec![Value::Int(2), Value::Int(200)]);
        rel.insert(smallvec![Value::Int(1), Value::Int(300)]);

        // Lookup by first column — should work with &self
        let mut results = Vec::new();
        rel.lookup_each(&[0], &[Value::Int(1)], |t| results.push(t.clone()));
        assert_eq!(results.len(), 2);

        let mut results = Vec::new();
        rel.lookup_each(&[0], &[Value::Int(2)], |t| results.push(t.clone()));
        assert_eq!(results.len(), 1);

        let mut results = Vec::new();
        rel.lookup_each(&[0], &[Value::Int(99)], |t| results.push(t.clone()));
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_lookup_any() {
        let mut rel = Relation::new(make_schema());
        rel.insert(smallvec![Value::Int(1), Value::Int(100)]);
        rel.insert(smallvec![Value::Int(2), Value::Int(200)]);

        assert!(rel.lookup_any(&[0], &[Value::Int(1)]));
        assert!(rel.lookup_any(&[0], &[Value::Int(2)]));
        assert!(!rel.lookup_any(&[0], &[Value::Int(99)]));
    }

    #[test]
    fn test_index_survives_insert() {
        let mut rel = Relation::new(make_schema());
        rel.insert(smallvec![Value::Int(1), Value::Int(100)]);

        // Build the index by looking up
        assert!(rel.lookup_any(&[0], &[Value::Int(1)]));

        // Insert more data — should be reflected in the index
        rel.insert(smallvec![Value::Int(1), Value::Int(200)]);
        let mut results = Vec::new();
        rel.lookup_each(&[0], &[Value::Int(1)], |t| results.push(t.clone()));
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_multi_column_index() {
        let mut rel = Relation::new(RelationSchema {
            name: "test".to_string(),
            columns: vec![
                ColumnDef { name: "a".to_string(), col_type: ColumnType::Int },
                ColumnDef { name: "b".to_string(), col_type: ColumnType::Int },
                ColumnDef { name: "c".to_string(), col_type: ColumnType::Int },
            ],
        });
        rel.insert(smallvec![Value::Int(1), Value::Int(10), Value::Int(100)]);
        rel.insert(smallvec![Value::Int(1), Value::Int(20), Value::Int(200)]);
        rel.insert(smallvec![Value::Int(2), Value::Int(10), Value::Int(300)]);

        // Index on columns [0, 1]
        let mut results = Vec::new();
        rel.lookup_each(&[0, 1], &[Value::Int(1), Value::Int(10)], |t| results.push(t.clone()));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0][2], Value::Int(100));
    }
}
