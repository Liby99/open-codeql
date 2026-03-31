//! LIR — Low-level Intermediate Representation.
//!
//! The LIR is the final IR before execution. All QL-level abstractions
//! (classes, modules, overrides) have been eliminated. What remains is:
//!
//! - **Concrete named relations** (monomorphized predicates)
//! - **Relational algebra operators**: WCO join, project, filter, anti-join, union, aggregate
//! - **Stratified evaluation plan**: strata ordered by negation/aggregation dependencies
//! - **Worst-case optimal join** as the core join primitive
//!
//! ## Pipeline position
//!
//! ```text
//! QL source → AST → HIR → MIR → LIR → Execution
//!                                  ↑ you are here
//! ```
//!
//! ## Design principles
//!
//! 1. **No variables** in the Datalog sense. All bindings are expressed as
//!    column references within relational operators.
//! 2. **WCO join** (worst-case optimal join) is the primary join strategy.
//!    Each join specifies the participating atoms and a variable ordering.
//! 3. **Stratification is explicit**: the program is a sequence of strata,
//!    each containing rules that can be evaluated together.
//! 4. **Recursion is explicit**: recursive strata are marked and require
//!    semi-naive fixpoint evaluation.

pub mod nodes;
pub mod lower;
pub mod pretty;

pub use nodes::*;
pub use lower::{lower_mir, LowerError};
pub use pretty::pretty_print;
