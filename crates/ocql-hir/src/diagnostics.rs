use ocql_common::Span;

use crate::FileId;

/// A diagnostic message produced during HIR analysis.
#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub file: FileId,
    pub notes: Vec<DiagnosticNote>,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sev = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        write!(f, "{sev}: {}", self.message)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug)]
pub struct DiagnosticNote {
    pub message: String,
    pub span: Span,
    pub file: FileId,
}
