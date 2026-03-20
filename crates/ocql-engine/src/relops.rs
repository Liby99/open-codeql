//! Relational algebra operators.
//!
//! Stateless functions that take relation(s) and produce a new relation.
//! These form the primitive building blocks that the Datalog evaluator
//! composes to evaluate rules.

use std::collections::{HashMap, HashSet};

use ocql_database::{Relation, RelationSchema, ColumnDef, Tuple, Value};
use ocql_schema::ColumnType;
use smallvec::SmallVec;

/// Filter rows matching a predicate.
pub fn filter(rel: &Relation, pred: &dyn Fn(&Tuple) -> bool) -> Relation {
    let mut result = Relation::new(rel.schema.clone());
    for tuple in rel.scan() {
        if pred(tuple) {
            result.insert(tuple.clone());
        }
    }
    result
}

/// Project to selected columns, deduplicating.
pub fn project(rel: &Relation, columns: &[usize]) -> Relation {
    let schema = RelationSchema {
        name: format!("{}_proj", rel.schema.name),
        columns: columns.iter().map(|&c| rel.schema.columns[c].clone()).collect(),
    };
    let mut result = Relation::new(schema);
    for tuple in rel.scan() {
        let projected: Tuple = columns.iter().map(|&c| tuple[c].clone()).collect();
        result.insert(projected);
    }
    result
}

/// Equi-join two relations on specified column pairs.
///
/// For each pair `(left_cols[i], right_cols[i])`, the values must match.
/// The result contains all columns from `left` followed by all columns from `right`.
pub fn join(
    left: &Relation,
    right: &Relation,
    left_cols: &[usize],
    right_cols: &[usize],
) -> Relation {
    assert_eq!(left_cols.len(), right_cols.len(), "join column counts must match");

    let schema = RelationSchema {
        name: format!("{}_{}_join", left.schema.name, right.schema.name),
        columns: left.schema.columns.iter()
            .chain(right.schema.columns.iter())
            .cloned()
            .collect(),
    };
    let mut result = Relation::new(schema);

    // Build hash table on the smaller side (right by default)
    let mut hash_table: HashMap<SmallVec<[Value; 4]>, Vec<&Tuple>> = HashMap::new();
    for tuple in right.scan() {
        let key: SmallVec<[Value; 4]> = right_cols.iter().map(|&c| tuple[c].clone()).collect();
        hash_table.entry(key).or_default().push(tuple);
    }

    // Probe from left
    for left_tuple in left.scan() {
        let key: SmallVec<[Value; 4]> = left_cols.iter().map(|&c| left_tuple[c].clone()).collect();
        if let Some(matches) = hash_table.get(&key) {
            for right_tuple in matches {
                let mut combined: Tuple = SmallVec::new();
                combined.extend(left_tuple.iter().cloned());
                combined.extend(right_tuple.iter().cloned());
                result.insert(combined);
            }
        }
    }

    result
}

/// Set union of two relations (must have same schema shape).
pub fn union(a: &Relation, b: &Relation) -> Relation {
    let mut result = Relation::new(a.schema.clone());
    for tuple in a.scan() {
        result.insert(tuple.clone());
    }
    for tuple in b.scan() {
        result.insert(tuple.clone());
    }
    result
}

/// Set difference: tuples in `a` not in `b`.
pub fn difference(a: &Relation, b: &Relation) -> Relation {
    let b_set: HashSet<&Tuple> = b.scan().collect();
    let mut result = Relation::new(a.schema.clone());
    for tuple in a.scan() {
        if !b_set.contains(tuple) {
            result.insert(tuple.clone());
        }
    }
    result
}

/// Rename a relation (creates a new relation with the given name, same tuples).
pub fn rename(rel: &Relation, new_name: &str) -> Relation {
    let schema = RelationSchema {
        name: new_name.to_string(),
        columns: rel.schema.columns.clone(),
    };
    let mut result = Relation::new(schema);
    for tuple in rel.scan() {
        result.insert(tuple.clone());
    }
    result
}

/// Aggregate function types.
#[derive(Debug, Clone)]
pub enum AggregateFunction {
    Count,
    Sum,
    Min,
    Max,
}

/// Aggregate: group by some columns, apply aggregate function to another column.
///
/// Result has the group-by columns followed by the aggregate result column.
pub fn aggregate(
    rel: &Relation,
    group_by: &[usize],
    agg_col: usize,
    agg_fn: &AggregateFunction,
) -> Relation {
    let mut schema_cols: Vec<ColumnDef> = group_by.iter()
        .map(|&c| rel.schema.columns[c].clone())
        .collect();
    schema_cols.push(ColumnDef {
        name: "agg_result".to_string(),
        col_type: ColumnType::Int,
    });
    let schema = RelationSchema {
        name: format!("{}_agg", rel.schema.name),
        columns: schema_cols,
    };

    // Group tuples
    let mut groups: HashMap<SmallVec<[Value; 4]>, Vec<Value>> = HashMap::new();
    for tuple in rel.scan() {
        let key: SmallVec<[Value; 4]> = group_by.iter().map(|&c| tuple[c].clone()).collect();
        groups.entry(key).or_default().push(tuple[agg_col].clone());
    }

    let mut result = Relation::new(schema);
    for (key, values) in &groups {
        let agg_result = apply_aggregate(agg_fn, values);
        let mut tuple: Tuple = SmallVec::new();
        tuple.extend(key.iter().cloned());
        tuple.push(agg_result);
        result.insert(tuple);
    }

    result
}

/// Count all rows (no grouping).
pub fn count(rel: &Relation) -> usize {
    rel.len()
}

fn apply_aggregate(func: &AggregateFunction, values: &[Value]) -> Value {
    match func {
        AggregateFunction::Count => Value::Int(values.len() as i64),
        AggregateFunction::Sum => {
            let sum: i64 = values.iter().filter_map(|v| v.as_int()).sum();
            Value::Int(sum)
        }
        AggregateFunction::Min => {
            values.iter().min().cloned().unwrap_or(Value::Null)
        }
        AggregateFunction::Max => {
            values.iter().max().cloned().unwrap_or(Value::Null)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smallvec::smallvec;

    fn make_rel(name: &str, col_names: &[&str]) -> Relation {
        let schema = RelationSchema {
            name: name.to_string(),
            columns: col_names.iter().map(|n| ColumnDef {
                name: n.to_string(),
                col_type: ColumnType::Int,
            }).collect(),
        };
        Relation::new(schema)
    }

    #[test]
    fn test_filter() {
        let mut rel = make_rel("r", &["x", "y"]);
        rel.insert(smallvec![Value::Int(1), Value::Int(10)]);
        rel.insert(smallvec![Value::Int(2), Value::Int(20)]);
        rel.insert(smallvec![Value::Int(3), Value::Int(30)]);

        let result = filter(&rel, &|t| t[0].as_int().unwrap() > 1);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_project() {
        let mut rel = make_rel("r", &["x", "y", "z"]);
        rel.insert(smallvec![Value::Int(1), Value::Int(10), Value::Int(100)]);
        rel.insert(smallvec![Value::Int(2), Value::Int(20), Value::Int(200)]);

        let result = project(&rel, &[0, 2]);
        assert_eq!(result.len(), 2);
        assert_eq!(result.schema.columns.len(), 2);

        let tuples: Vec<_> = result.scan().collect();
        assert_eq!(tuples[0][0], Value::Int(1));
        assert_eq!(tuples[0][1], Value::Int(100));
    }

    #[test]
    fn test_project_dedup() {
        let mut rel = make_rel("r", &["x", "y"]);
        rel.insert(smallvec![Value::Int(1), Value::Int(10)]);
        rel.insert(smallvec![Value::Int(1), Value::Int(20)]);
        rel.insert(smallvec![Value::Int(2), Value::Int(30)]);

        // Project on first column only — should dedup
        let result = project(&rel, &[0]);
        assert_eq!(result.len(), 2); // 1 and 2
    }

    #[test]
    fn test_join() {
        // edge(src, dst) JOIN node(id, name) ON edge.dst = node.id
        let mut edges = make_rel("edge", &["src", "dst"]);
        edges.insert(smallvec![Value::Int(1), Value::Int(2)]);
        edges.insert(smallvec![Value::Int(1), Value::Int(3)]);
        edges.insert(smallvec![Value::Int(2), Value::Int(3)]);

        let mut nodes = make_rel("node", &["id", "name"]);
        nodes.insert(smallvec![Value::Int(1), Value::Int(100)]);
        nodes.insert(smallvec![Value::Int(2), Value::Int(200)]);
        nodes.insert(smallvec![Value::Int(3), Value::Int(300)]);

        let result = join(&edges, &nodes, &[1], &[0]); // edge.dst = node.id
        assert_eq!(result.len(), 3);
        assert_eq!(result.schema.columns.len(), 4); // src, dst, id, name
    }

    #[test]
    fn test_join_no_matches() {
        let mut a = make_rel("a", &["x"]);
        a.insert(smallvec![Value::Int(1)]);

        let mut b = make_rel("b", &["y"]);
        b.insert(smallvec![Value::Int(2)]);

        let result = join(&a, &b, &[0], &[0]);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_union() {
        let mut a = make_rel("a", &["x"]);
        a.insert(smallvec![Value::Int(1)]);
        a.insert(smallvec![Value::Int(2)]);

        let mut b = make_rel("b", &["x"]);
        b.insert(smallvec![Value::Int(2)]);
        b.insert(smallvec![Value::Int(3)]);

        let result = union(&a, &b);
        assert_eq!(result.len(), 3); // {1, 2, 3}
    }

    #[test]
    fn test_difference() {
        let mut a = make_rel("a", &["x"]);
        a.insert(smallvec![Value::Int(1)]);
        a.insert(smallvec![Value::Int(2)]);
        a.insert(smallvec![Value::Int(3)]);

        let mut b = make_rel("b", &["x"]);
        b.insert(smallvec![Value::Int(2)]);

        let result = difference(&a, &b);
        assert_eq!(result.len(), 2); // {1, 3}
    }

    #[test]
    fn test_aggregate_count() {
        let mut rel = make_rel("r", &["group", "val"]);
        rel.insert(smallvec![Value::Int(1), Value::Int(10)]);
        rel.insert(smallvec![Value::Int(1), Value::Int(20)]);
        rel.insert(smallvec![Value::Int(2), Value::Int(30)]);

        let result = aggregate(&rel, &[0], 1, &AggregateFunction::Count);
        assert_eq!(result.len(), 2);

        let tuples: Vec<_> = result.scan().collect();
        // Group 1 has 2 values, group 2 has 1
        let g1 = tuples.iter().find(|t| t[0] == Value::Int(1)).unwrap();
        let g2 = tuples.iter().find(|t| t[0] == Value::Int(2)).unwrap();
        assert_eq!(g1[1], Value::Int(2));
        assert_eq!(g2[1], Value::Int(1));
    }

    #[test]
    fn test_aggregate_sum() {
        let mut rel = make_rel("r", &["group", "val"]);
        rel.insert(smallvec![Value::Int(1), Value::Int(10)]);
        rel.insert(smallvec![Value::Int(1), Value::Int(20)]);
        rel.insert(smallvec![Value::Int(2), Value::Int(30)]);

        let result = aggregate(&rel, &[0], 1, &AggregateFunction::Sum);
        let tuples: Vec<_> = result.scan().collect();
        let g1 = tuples.iter().find(|t| t[0] == Value::Int(1)).unwrap();
        assert_eq!(g1[1], Value::Int(30));
    }

    #[test]
    fn test_aggregate_min_max() {
        let mut rel = make_rel("r", &["group", "val"]);
        rel.insert(smallvec![Value::Int(1), Value::Int(10)]);
        rel.insert(smallvec![Value::Int(1), Value::Int(20)]);
        rel.insert(smallvec![Value::Int(1), Value::Int(5)]);

        let min_result = aggregate(&rel, &[0], 1, &AggregateFunction::Min);
        let min_tuples: Vec<_> = min_result.scan().collect();
        assert_eq!(min_tuples[0][1], Value::Int(5));

        let max_result = aggregate(&rel, &[0], 1, &AggregateFunction::Max);
        let max_tuples: Vec<_> = max_result.scan().collect();
        assert_eq!(max_tuples[0][1], Value::Int(20));
    }

    #[test]
    fn test_self_join_transitive_step() {
        // edge(a, b) self-join to get 2-hop paths: edge(a, x) JOIN edge(x, b)
        let mut edges = make_rel("edge", &["src", "dst"]);
        edges.insert(smallvec![Value::Int(1), Value::Int(2)]);
        edges.insert(smallvec![Value::Int(2), Value::Int(3)]);
        edges.insert(smallvec![Value::Int(3), Value::Int(4)]);

        // Join edge(a, x) with edge(x, b) on edge.dst = edge.src
        let result = join(&edges, &edges, &[1], &[0]);
        // Result: (1,2,2,3), (2,3,3,4) — project to (col0, col3) for 2-hop paths
        let two_hop = project(&result, &[0, 3]);
        assert_eq!(two_hop.len(), 2);

        let paths: Vec<_> = two_hop.scan().collect();
        assert!(paths.contains(&&smallvec![Value::Int(1), Value::Int(3)]));
        assert!(paths.contains(&&smallvec![Value::Int(2), Value::Int(4)]));
    }
}
