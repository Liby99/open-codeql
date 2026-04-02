//! Decode query results into human-readable formats: CSV, JSON, SARIF.

use crate::metadata::QueryKind;
use crate::result::{QueryResult, ResultValue};

// ============================================================
// CSV
// ============================================================

/// Convert query results to CSV format.
pub fn to_csv(result: &QueryResult) -> String {
    let mut out = String::new();

    // Header
    let headers: Vec<&str> = result.columns.iter().map(|c| c.name.as_str()).collect();
    out.push_str(&headers.join(","));
    out.push('\n');

    // Rows
    for row in &result.rows {
        let cells: Vec<String> = row.iter().map(csv_escape).collect();
        out.push_str(&cells.join(","));
        out.push('\n');
    }

    out
}

fn csv_escape(v: &ResultValue) -> String {
    match v {
        ResultValue::String(s) => {
            if s.contains(',') || s.contains('"') || s.contains('\n') {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.clone()
            }
        }
        ResultValue::Int(i) => i.to_string(),
        ResultValue::Float(f) => f.to_string(),
        ResultValue::Entity(id) => format!("#{}", id),
    }
}

// ============================================================
// JSON
// ============================================================

/// Convert query results to JSON format.
///
/// Produces a JSON object:
/// ```json
/// {
///   "metadata": { "name": "...", "kind": "...", ... },
///   "columns": ["col0", "col1"],
///   "rows": [["val0", "val1"], ...]
/// }
/// ```
pub fn to_json(result: &QueryResult) -> String {
    let columns: Vec<serde_json::Value> = result
        .columns
        .iter()
        .map(|c| serde_json::Value::String(c.name.clone()))
        .collect();

    let rows: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            let cells: Vec<serde_json::Value> = row.iter().map(json_value).collect();
            serde_json::Value::Array(cells)
        })
        .collect();

    let mut metadata = serde_json::Map::new();
    if let Some(ref name) = result.metadata.name {
        metadata.insert("name".into(), serde_json::Value::String(name.clone()));
    }
    if let Some(ref kind) = result.metadata.kind {
        metadata.insert("kind".into(), serde_json::Value::String(kind_to_string(kind)));
    }
    if let Some(ref id) = result.metadata.id {
        metadata.insert("id".into(), serde_json::Value::String(id.clone()));
    }
    if !result.metadata.tags.is_empty() {
        let tags: Vec<serde_json::Value> = result
            .metadata
            .tags
            .iter()
            .map(|t| serde_json::Value::String(t.clone()))
            .collect();
        metadata.insert("tags".into(), serde_json::Value::Array(tags));
    }

    let mut root = serde_json::Map::new();
    root.insert("metadata".into(), serde_json::Value::Object(metadata));
    root.insert("columns".into(), serde_json::Value::Array(columns));
    root.insert("rows".into(), serde_json::Value::Array(rows));

    serde_json::to_string_pretty(&serde_json::Value::Object(root)).unwrap()
}

fn json_value(v: &ResultValue) -> serde_json::Value {
    match v {
        ResultValue::String(s) => serde_json::Value::String(s.clone()),
        ResultValue::Int(i) => serde_json::json!(i),
        ResultValue::Float(f) => serde_json::json!(f),
        ResultValue::Entity(id) => serde_json::json!(format!("#{}", id)),
    }
}

fn kind_to_string(kind: &QueryKind) -> String {
    match kind {
        QueryKind::Problem => "problem".to_string(),
        QueryKind::PathProblem => "path-problem".to_string(),
        QueryKind::Table => "table".to_string(),
        QueryKind::Metric => "metric".to_string(),
        QueryKind::Diagnostic => "diagnostic".to_string(),
        QueryKind::Other(s) => s.clone(),
    }
}

// ============================================================
// SARIF 2.1.0
// ============================================================

/// Convert query results to SARIF 2.1.0 format.
///
/// SARIF output is only meaningful for `@kind problem` and `@kind path-problem` queries.
/// For other kinds, returns a valid SARIF with no results.
///
/// For `@kind problem`:
///   select element, message — maps to SARIF result with message
///   Each row becomes one SARIF result entry.
///
/// For `@kind path-problem`:
///   select element, source, sink, message — maps to SARIF result with codeFlows
///   (path data would require additional info; we emit what we have)
pub fn to_sarif(result: &QueryResult) -> String {
    let tool_name = "open-codeql";
    let tool_version = env!("CARGO_PKG_VERSION");

    let query_id = result
        .metadata
        .id
        .as_deref()
        .unwrap_or("unknown");
    let query_name = result
        .metadata
        .name
        .as_deref()
        .unwrap_or("Unknown query");

    // Build the rule descriptor
    let mut rule = serde_json::Map::new();
    rule.insert("id".into(), serde_json::Value::String(query_id.to_string()));
    rule.insert(
        "name".into(),
        serde_json::Value::String(query_name.to_string()),
    );
    if let Some(ref desc) = result.metadata.description {
        let mut short_desc = serde_json::Map::new();
        short_desc.insert("text".into(), serde_json::Value::String(desc.clone()));
        rule.insert(
            "shortDescription".into(),
            serde_json::Value::Object(short_desc),
        );
    }
    if let Some(ref sev) = result.metadata.security_severity {
        let mut props = serde_json::Map::new();
        props.insert(
            "security-severity".into(),
            serde_json::Value::String(sev.clone()),
        );
        if !result.metadata.tags.is_empty() {
            let tags: Vec<serde_json::Value> = result
                .metadata
                .tags
                .iter()
                .map(|t| serde_json::Value::String(t.clone()))
                .collect();
            props.insert("tags".into(), serde_json::Value::Array(tags));
        }
        rule.insert("properties".into(), serde_json::Value::Object(props));
    }

    // Map severity
    let level = match result.metadata.problem_severity.as_deref() {
        Some("error") => "error",
        Some("warning") => "warning",
        Some("recommendation") => "note",
        _ => "warning",
    };

    // Build results
    let sarif_results: Vec<serde_json::Value> = match result.metadata.kind {
        Some(QueryKind::Problem) => result
            .rows
            .iter()
            .map(|row| build_problem_result(row, query_id, level))
            .collect(),
        Some(QueryKind::PathProblem) => result
            .rows
            .iter()
            .map(|row| build_path_problem_result(row, query_id, level))
            .collect(),
        _ => {
            // For table/metric/diagnostic, emit each row as a basic result
            result
                .rows
                .iter()
                .map(|row| build_table_result(row, query_id, level))
                .collect()
        }
    };

    // Assemble SARIF
    let sarif = serde_json::json!({
        "$schema": "https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/schemas/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": tool_name,
                    "version": tool_version,
                    "rules": [serde_json::Value::Object(rule)]
                }
            },
            "results": sarif_results
        }]
    });

    serde_json::to_string_pretty(&sarif).unwrap()
}

/// Build a SARIF result for a `@kind problem` row.
/// Expected columns: (element, message) or more generally (col0, col1, ...).
fn build_problem_result(
    row: &[ResultValue],
    rule_id: &str,
    level: &str,
) -> serde_json::Value {
    // Use the last column as message, first column as the element identifier
    let message = if row.len() >= 2 {
        format!("{}", row.last().unwrap())
    } else if row.len() == 1 {
        format!("{}", row[0])
    } else {
        "finding".to_string()
    };

    let element = if !row.is_empty() {
        format!("{}", row[0])
    } else {
        "unknown".to_string()
    };

    let mut result = serde_json::Map::new();
    result.insert("ruleId".into(), serde_json::Value::String(rule_id.to_string()));
    result.insert("level".into(), serde_json::Value::String(level.to_string()));
    result.insert(
        "message".into(),
        serde_json::json!({ "text": message }),
    );

    // Add partial location info from available data
    let mut location = serde_json::Map::new();
    let mut physical = serde_json::Map::new();
    let mut artifact = serde_json::Map::new();
    artifact.insert(
        "uri".into(),
        serde_json::Value::String(element),
    );
    physical.insert(
        "artifactLocation".into(),
        serde_json::Value::Object(artifact),
    );
    location.insert(
        "physicalLocation".into(),
        serde_json::Value::Object(physical),
    );
    result.insert(
        "locations".into(),
        serde_json::Value::Array(vec![serde_json::Value::Object(location)]),
    );

    serde_json::Value::Object(result)
}

/// Build a SARIF result for a `@kind path-problem` row.
/// Expected columns: (element, source, sink, message).
fn build_path_problem_result(
    row: &[ResultValue],
    rule_id: &str,
    level: &str,
) -> serde_json::Value {
    // For path-problem, row is typically: (element, source, sink, message)
    let message = if row.len() >= 4 {
        format!("{}", row[3])
    } else if !row.is_empty() {
        format!("{}", row.last().unwrap())
    } else {
        "finding".to_string()
    };

    let mut result = serde_json::Map::new();
    result.insert("ruleId".into(), serde_json::Value::String(rule_id.to_string()));
    result.insert("level".into(), serde_json::Value::String(level.to_string()));
    result.insert("message".into(), serde_json::json!({ "text": message }));

    // For path-problem, include source and sink in relatedLocations
    let mut related = Vec::new();
    if row.len() >= 3 {
        related.push(serde_json::json!({
            "id": 0,
            "message": { "text": format!("source: {}", row[1]) }
        }));
        related.push(serde_json::json!({
            "id": 1,
            "message": { "text": format!("sink: {}", row[2]) }
        }));
    }
    if !related.is_empty() {
        result.insert(
            "relatedLocations".into(),
            serde_json::Value::Array(related),
        );
    }

    // codeFlows placeholder — real path data would go here
    result.insert(
        "codeFlows".into(),
        serde_json::Value::Array(vec![serde_json::json!({
            "threadFlows": [{
                "locations": []
            }]
        })]),
    );

    serde_json::Value::Object(result)
}

/// Build a basic SARIF result for table/metric queries.
fn build_table_result(
    row: &[ResultValue],
    rule_id: &str,
    level: &str,
) -> serde_json::Value {
    let message = row
        .iter()
        .map(|v| format!("{}", v))
        .collect::<Vec<_>>()
        .join(", ");

    serde_json::json!({
        "ruleId": rule_id,
        "level": level,
        "message": { "text": message }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::QueryMetadata;
    use crate::result::{ColumnDef, ColumnType};

    fn sample_problem_result() -> QueryResult {
        QueryResult::new(
            QueryMetadata {
                name: Some("Dangerous call".to_string()),
                description: Some("Finds dangerous calls".to_string()),
                kind: Some(QueryKind::Problem),
                id: Some("test/dangerous".to_string()),
                problem_severity: Some("error".to_string()),
                security_severity: Some("9.0".to_string()),
                tags: vec!["security".to_string()],
            },
            vec![
                ColumnDef { name: "callee".to_string(), col_type: ColumnType::String },
                ColumnDef { name: "caller".to_string(), col_type: ColumnType::String },
            ],
            vec![
                vec![
                    ResultValue::String("gets".to_string()),
                    ResultValue::String("dangerous_gets".to_string()),
                ],
                vec![
                    ResultValue::String("strcpy".to_string()),
                    ResultValue::String("dangerous_strcpy".to_string()),
                ],
            ],
        )
    }

    #[test]
    fn csv_output() {
        let result = sample_problem_result();
        let csv = to_csv(&result);
        assert!(csv.starts_with("callee,caller\n"));
        assert!(csv.contains("gets,dangerous_gets"));
        assert!(csv.contains("strcpy,dangerous_strcpy"));
    }

    #[test]
    fn csv_escape_commas() {
        let result = QueryResult::new(
            QueryMetadata::default(),
            vec![ColumnDef { name: "msg".to_string(), col_type: ColumnType::String }],
            vec![vec![ResultValue::String("hello, world".to_string())]],
        );
        let csv = to_csv(&result);
        assert!(csv.contains("\"hello, world\""));
    }

    #[test]
    fn json_output() {
        let result = sample_problem_result();
        let json = to_json(&result);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["metadata"]["kind"], "problem");
        assert_eq!(parsed["rows"][0][0], "gets");
        assert_eq!(parsed["rows"][1][0], "strcpy");
    }

    #[test]
    fn sarif_output() {
        let result = sample_problem_result();
        let sarif = to_sarif(&result);
        let parsed: serde_json::Value = serde_json::from_str(&sarif).unwrap();
        assert_eq!(parsed["version"], "2.1.0");
        let results = &parsed["runs"][0]["results"];
        assert_eq!(results.as_array().unwrap().len(), 2);
        assert_eq!(results[0]["ruleId"], "test/dangerous");
        assert_eq!(results[0]["level"], "error");
    }

    #[test]
    fn sarif_path_problem() {
        let result = QueryResult::new(
            QueryMetadata {
                kind: Some(QueryKind::PathProblem),
                id: Some("test/path".to_string()),
                ..QueryMetadata::default()
            },
            vec![
                ColumnDef { name: "element".to_string(), col_type: ColumnType::String },
                ColumnDef { name: "source".to_string(), col_type: ColumnType::String },
                ColumnDef { name: "sink".to_string(), col_type: ColumnType::String },
                ColumnDef { name: "message".to_string(), col_type: ColumnType::String },
            ],
            vec![vec![
                ResultValue::String("call".to_string()),
                ResultValue::String("input".to_string()),
                ResultValue::String("output".to_string()),
                ResultValue::String("tainted data flows here".to_string()),
            ]],
        );
        let sarif = to_sarif(&result);
        let parsed: serde_json::Value = serde_json::from_str(&sarif).unwrap();
        let r = &parsed["runs"][0]["results"][0];
        assert!(r["codeFlows"].is_array());
        assert!(r["relatedLocations"].is_array());
    }
}
