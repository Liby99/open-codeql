//! Data types for CodeQL source/sink/summary models.

use crate::access_path::AccessPath;

/// Identifies a callable (method/function) in a model.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallableSpec {
    /// Package or namespace (e.g., "java.sql", "android.content", "boost::asio")
    pub package: String,
    /// Type/class name; empty for free functions
    pub type_name: String,
    /// Whether the model applies to subtypes/overrides
    pub subtypes: bool,
    /// Method/function name
    pub name: String,
    /// Type signature for disambiguation (e.g., "(String)", "(Intent,int,Bundle)")
    pub signature: String,
}

impl CallableSpec {
    /// Returns a qualified name like "java.sql.Statement.execute"
    pub fn qualified_name(&self) -> String {
        if self.type_name.is_empty() {
            if self.package.is_empty() {
                self.name.clone()
            } else {
                format!("{}.{}", self.package, self.name)
            }
        } else if self.package.is_empty() {
            format!("{}.{}", self.type_name, self.name)
        } else {
            format!("{}.{}.{}", self.package, self.type_name, self.name)
        }
    }
}

/// A source model: where tainted data enters.
#[derive(Debug, Clone)]
pub struct SourceModel {
    pub callable: CallableSpec,
    /// Where the source value appears (e.g., ReturnValue, Argument[0])
    pub output: AccessPath,
    /// Threat model kind (e.g., "remote", "local", "file", "android-external-storage-dir")
    pub kind: String,
    /// Provenance (e.g., "manual", "ai-manual", "df-generated")
    pub provenance: String,
}

/// A sink model: where tainted data reaches a vulnerability.
#[derive(Debug, Clone)]
pub struct SinkModel {
    pub callable: CallableSpec,
    /// Where the sink receives tainted data (e.g., Argument[0])
    pub input: AccessPath,
    /// Vulnerability kind (e.g., "sql-injection", "path-injection", "command-injection")
    pub kind: String,
    pub provenance: String,
}

/// A summary model: how data flows through a library method.
#[derive(Debug, Clone)]
pub struct SummaryModel {
    pub callable: CallableSpec,
    /// Where data comes from
    pub input: AccessPath,
    /// Where data flows to
    pub output: AccessPath,
    /// Flow kind: "taint" (whole-value propagation) or "value" (structural)
    pub kind: String,
    pub provenance: String,
}

/// A neutral model: marks a method as having no taint effect.
#[derive(Debug, Clone)]
pub struct NeutralModel {
    pub package: String,
    pub type_name: String,
    pub name: String,
    pub signature: String,
    pub kind: String,
    pub provenance: String,
}

/// An unrecognized extension — stored as raw string tuples.
#[derive(Debug, Clone)]
pub struct RawExtension {
    pub pack: String,
    pub extensible: String,
    pub data: Vec<String>,
}

/// A collection of all models from one or more .model.yml files.
#[derive(Debug, Clone, Default)]
pub struct ModelStore {
    pub sources: Vec<SourceModel>,
    pub sinks: Vec<SinkModel>,
    pub summaries: Vec<SummaryModel>,
    pub neutrals: Vec<NeutralModel>,
    pub raw_extensions: Vec<RawExtension>,
}

impl ModelStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Merge another store into this one.
    pub fn merge(&mut self, other: ModelStore) {
        self.sources.extend(other.sources);
        self.sinks.extend(other.sinks);
        self.summaries.extend(other.summaries);
        self.neutrals.extend(other.neutrals);
        self.raw_extensions.extend(other.raw_extensions);
    }

    /// Total number of models across all categories.
    pub fn total_models(&self) -> usize {
        self.sources.len() + self.sinks.len() + self.summaries.len() + self.neutrals.len()
    }

    /// Look up sources by vulnerability kind.
    pub fn sources_by_kind(&self, kind: &str) -> Vec<&SourceModel> {
        self.sources.iter().filter(|s| s.kind == kind).collect()
    }

    /// Look up sinks by vulnerability kind.
    pub fn sinks_by_kind(&self, kind: &str) -> Vec<&SinkModel> {
        self.sinks.iter().filter(|s| s.kind == kind).collect()
    }

    /// Look up summaries by flow kind.
    pub fn summaries_by_kind(&self, kind: &str) -> Vec<&SummaryModel> {
        self.summaries.iter().filter(|s| s.kind == kind).collect()
    }

    /// Look up all models for a given callable name.
    pub fn summaries_for(&self, package: &str, type_name: &str, name: &str) -> Vec<&SummaryModel> {
        self.summaries.iter()
            .filter(|s| s.callable.package == package
                && s.callable.type_name == type_name
                && s.callable.name == name)
            .collect()
    }
}
