use std::collections::{BTreeMap, BTreeSet};

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

/// An index on one or more columns for fast lookup.
struct Index {
    /// Which columns are indexed (by position).
    columns: Vec<usize>,
    /// Map from indexed column values → set of matching tuples.
    data: BTreeMap<SmallVec<[Value; 4]>, Vec<Tuple>>,
}

/// A relation is a named set of tuples with a schema.
pub struct Relation {
    pub schema: RelationSchema,
    tuples: BTreeSet<Tuple>,
    indexes: Vec<Index>,
}

impl Relation {
    /// Create an empty relation with the given schema.
    pub fn new(schema: RelationSchema) -> Self {
        Self {
            schema,
            tuples: BTreeSet::new(),
            indexes: Vec::new(),
        }
    }

    /// Insert a tuple. Returns true if it was newly inserted (not a duplicate).
    pub fn insert(&mut self, tuple: Tuple) -> bool {
        let is_new = self.tuples.insert(tuple.clone());
        if is_new {
            // Update all existing indexes
            for index in &mut self.indexes {
                let key: SmallVec<[Value; 4]> = index.columns.iter().map(|&c| tuple[c].clone()).collect();
                index.data.entry(key).or_default().push(tuple.clone());
            }
        }
        is_new
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

    /// Ensure an index exists on the given columns, then look up matching tuples.
    pub fn lookup(&mut self, columns: &[usize], key: &[Value]) -> Vec<&Tuple> {
        // Find or create the index
        let idx = self.ensure_index(columns);
        let search_key: SmallVec<[Value; 4]> = key.iter().cloned().collect();
        match self.indexes[idx].data.get(&search_key) {
            Some(tuples) => tuples.iter().collect(),
            None => Vec::new(),
        }
    }

    /// Ensure an index exists for the given columns, returning its position.
    fn ensure_index(&mut self, columns: &[usize]) -> usize {
        // Check if index already exists
        for (i, index) in self.indexes.iter().enumerate() {
            if index.columns == columns {
                return i;
            }
        }

        // Build new index from existing tuples
        let mut data: BTreeMap<SmallVec<[Value; 4]>, Vec<Tuple>> = BTreeMap::new();
        for tuple in &self.tuples {
            let key: SmallVec<[Value; 4]> = columns.iter().map(|&c| tuple[c].clone()).collect();
            data.entry(key).or_default().push(tuple.clone());
        }

        self.indexes.push(Index {
            columns: columns.to_vec(),
            data,
        });
        self.indexes.len() - 1
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
    fn test_lookup() {
        let mut rel = Relation::new(make_schema());
        rel.insert(smallvec![Value::Int(1), Value::Int(100)]);
        rel.insert(smallvec![Value::Int(2), Value::Int(200)]);
        rel.insert(smallvec![Value::Int(1), Value::Int(300)]);

        // Lookup by first column
        let results = rel.lookup(&[0], &[Value::Int(1)]);
        assert_eq!(results.len(), 2);

        let results = rel.lookup(&[0], &[Value::Int(2)]);
        assert_eq!(results.len(), 1);

        let results = rel.lookup(&[0], &[Value::Int(99)]);
        assert_eq!(results.len(), 0);
    }
}
