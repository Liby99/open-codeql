pub mod relops;
pub mod rule;
pub mod stratify;
pub mod eval;

pub use eval::evaluate;
pub use rule::{Program, Rule, Atom, Term, BodyElement, Guard, CompOp, AggFunction};
pub use rule::{var, int, entity};
pub use stratify::{Stratum, stratify};
