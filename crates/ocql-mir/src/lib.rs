//! MIR (Mid-level IR) — QL to Datalog lowering.
//!
//! Compiles QL source code (predicates, classes, select queries) into
//! flat Datalog rules that `ocql-engine` can evaluate.

pub mod lower;

pub use lower::{LowerCtx, LowerError, lower_source_file};

use ocql_engine::rule::Program;

/// Parse a QL source string and lower it to a Datalog program.
pub fn compile_ql(source: &str) -> Result<Program, CompileError> {
    let ast = ocql_ql_parser::parse_source_file(source)
        .map_err(|e| CompileError::Parse(format!("{:?}", e)))?;

    let program = lower_source_file(&ast)
        .map_err(CompileError::Lower)?;

    Ok(program)
}

/// Errors from QL compilation.
#[derive(Debug)]
pub enum CompileError {
    Parse(String),
    Lower(LowerError),
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::Parse(msg) => write!(f, "parse error: {}", msg),
            CompileError::Lower(err) => write!(f, "lowering error: {}", err),
        }
    }
}

impl std::error::Error for CompileError {}
