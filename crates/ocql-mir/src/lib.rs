//! MIR (Mid-level IR) for open-cql.
//!
//! MIR is a flat, relational representation where all QL classes, modules, and
//! high-level constructs have been lowered to named predicates with explicit
//! rule bodies. MIR bridges HIR (name-resolved, typed QL) and the Datalog engine.
//!
//! # Pipeline
//!
//! ```text
//! QL Source → AST → HIR → MIR → Engine Rules → Evaluation
//! ```
//!
//! # Modules
//!
//! - `nodes` — MIR node types (program, predicate, atom, term, etc.)
//! - `sexpr` — S-expression printing and parsing for MIR
//! - `lower` — AST → MIR lowering
//! - `emit` — MIR → engine rule emission

pub mod nodes;
pub mod sexpr;
pub mod lower;
pub mod emit;

pub use nodes::*;
pub use lower::{LowerCtx, LowerError, lower_source_file};
pub use emit::{emit_program, emit_program_with_strings};
pub use sexpr::{print_program as print_mir, parse_program as parse_mir};

/// Parse a QL source string and compile it to a MIR program.
pub fn compile_ql(source: &str) -> Result<MirProgram, CompileError> {
    let ast = ocql_ql_parser::parse_source_file(source)
        .map_err(|e| CompileError::Parse(format!("{:?}", e)))?;

    let mir = lower_source_file(&ast)
        .map_err(CompileError::Lower)?;

    Ok(mir)
}

/// Parse QL and emit directly to engine rules (convenience for simple cases).
pub fn compile_ql_to_engine(source: &str) -> Result<ocql_engine::rule::Program, CompileError> {
    let mir = compile_ql(source)?;
    Ok(emit_program_with_strings(&mir))
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
