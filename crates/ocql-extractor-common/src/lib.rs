mod emitter;
mod extractor;
mod location;
mod tree_sitter_utils;

pub use emitter::FactEmitter;
pub use extractor::{Extractor, ExtractionResult};
pub use location::LocationEmitter;
pub use tree_sitter_utils::{NodeExt, walk_tree};

// Re-export tree-sitter for use by language-specific extractors
pub use tree_sitter;
