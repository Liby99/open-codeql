//! Parse query metadata from QLDoc comment blocks.
//!
//! CodeQL queries start with a `/** ... */` block containing `@key value` annotations:
//! ```ql
//! /**
//!  * @name Call to dangerous function
//!  * @kind problem
//!  * @id my/dangerous-call
//!  * @problem.severity error
//!  * @security-severity 9.0
//!  * @tags security
//!  *       external/cwe/cwe-120
//!  */
//! ```

/// The kind of query, parsed from `@kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryKind {
    /// `@kind problem` — alert-style results: (element, message)
    Problem,
    /// `@kind path-problem` — data-flow path results: (element, source, sink, message)
    PathProblem,
    /// `@kind table` — raw tabular output
    Table,
    /// `@kind metric` — numeric metric results
    Metric,
    /// `@kind diagnostic` — diagnostic results
    Diagnostic,
    /// Unknown or unrecognized kind
    Other(String),
}

impl QueryKind {
    fn parse(s: &str) -> Self {
        match s.trim() {
            "problem" => Self::Problem,
            "path-problem" => Self::PathProblem,
            "table" => Self::Table,
            "metric" => Self::Metric,
            "diagnostic" => Self::Diagnostic,
            other => Self::Other(other.to_string()),
        }
    }
}

/// Metadata parsed from a QLDoc comment block at the top of a `.ql` file.
#[derive(Debug, Clone, Default)]
pub struct QueryMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
    pub kind: Option<QueryKind>,
    pub id: Option<String>,
    pub problem_severity: Option<String>,
    pub security_severity: Option<String>,
    pub tags: Vec<String>,
}

/// Parse query metadata from the raw source text of a `.ql` file.
///
/// Extracts the first `/** ... */` block comment and parses `@key value` pairs.
pub fn parse_query_metadata(source: &str) -> QueryMetadata {
    let mut meta = QueryMetadata::default();

    // Find the first /** ... */ block
    let Some(start) = source.find("/**") else {
        return meta;
    };
    let search_from = start + 3;
    let Some(end_rel) = source[search_from..].find("*/") else {
        return meta;
    };
    let block = &source[search_from..search_from + end_rel];

    // Parse lines: strip leading ` * ` prefix, then look for @key
    let mut current_key: Option<String> = None;
    let mut current_value = String::new();

    let flush = |meta: &mut QueryMetadata, key: &str, value: &str| {
        let value = value.trim();
        match key {
            "name" => meta.name = Some(value.to_string()),
            "description" => meta.description = Some(value.to_string()),
            "kind" => meta.kind = Some(QueryKind::parse(value)),
            "id" => meta.id = Some(value.to_string()),
            "problem.severity" => meta.problem_severity = Some(value.to_string()),
            "security-severity" => meta.security_severity = Some(value.to_string()),
            "tags" => {
                // Tags can be multiline: "security\n       external/cwe/..."
                for part in value.split_whitespace() {
                    meta.tags.push(part.to_string());
                }
            }
            _ => {} // ignore unknown keys
        }
    };

    for line in block.lines() {
        // Strip leading whitespace + optional " * "
        let trimmed = line.trim_start();
        let trimmed = if let Some(rest) = trimmed.strip_prefix("* ") {
            rest
        } else if let Some(rest) = trimmed.strip_prefix("*") {
            rest
        } else {
            trimmed
        };

        if let Some(rest) = trimmed.strip_prefix('@') {
            // Flush previous key if any
            if let Some(ref key) = current_key {
                flush(&mut meta, key, &current_value);
            }

            // Parse new @key value
            let (key, value) = match rest.split_once(char::is_whitespace) {
                Some((k, v)) => (k.to_string(), v.trim().to_string()),
                None => (rest.to_string(), String::new()),
            };
            current_key = Some(key);
            current_value = value;
        } else if current_key.is_some() {
            // Continuation line for current key
            let t = trimmed.trim();
            if !t.is_empty() {
                current_value.push(' ');
                current_value.push_str(t);
            }
        }
    }

    // Flush last key
    if let Some(ref key) = current_key {
        flush(&mut meta, key, &current_value);
    }

    meta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_problem_query_metadata() {
        let source = r#"/**
 * @name Call to dangerous C function
 * @description Finds calls to known dangerous C functions that are common
 *              sources of buffer overflow and format string vulnerabilities.
 * @kind problem
 * @id ocql/dangerous-function-call
 * @problem.severity error
 * @security-severity 9.0
 * @tags security
 *       external/cwe/cwe-120
 *       external/cwe/cwe-134
 */

from string callee, string caller
where dangerousFinding(callee, caller)
select callee, caller
"#;
        let meta = parse_query_metadata(source);
        assert_eq!(meta.name.as_deref(), Some("Call to dangerous C function"));
        assert!(meta.description.as_ref().unwrap().starts_with("Finds calls"));
        assert_eq!(meta.kind, Some(QueryKind::Problem));
        assert_eq!(meta.id.as_deref(), Some("ocql/dangerous-function-call"));
        assert_eq!(meta.problem_severity.as_deref(), Some("error"));
        assert_eq!(meta.security_severity.as_deref(), Some("9.0"));
        assert_eq!(meta.tags, vec!["security", "external/cwe/cwe-120", "external/cwe/cwe-134"]);
    }

    #[test]
    fn parse_path_problem_metadata() {
        let source = r#"/**
 * @name Constant array overflow
 * @kind path-problem
 * @id cpp/constant-array-overflow
 */
"#;
        let meta = parse_query_metadata(source);
        assert_eq!(meta.kind, Some(QueryKind::PathProblem));
        assert_eq!(meta.id.as_deref(), Some("cpp/constant-array-overflow"));
    }

    #[test]
    fn no_metadata() {
        let source = "from int x select x";
        let meta = parse_query_metadata(source);
        assert!(meta.name.is_none());
        assert!(meta.kind.is_none());
    }

    #[test]
    fn table_kind() {
        let source = "/** @kind table */\nfrom int x select x";
        let meta = parse_query_metadata(source);
        assert_eq!(meta.kind, Some(QueryKind::Table));
    }
}
