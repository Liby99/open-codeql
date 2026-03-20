mod emitter;
mod extractor;
mod location;
pub mod source_roots;
mod tree_sitter_utils;

pub use emitter::FactEmitter;
pub use extractor::{Extractor, ExtractionResult, ProjectExtractionResult};
pub use location::LocationEmitter;
pub use source_roots::{BuildSystem, SourceRoot, discover_source_roots};
pub use tree_sitter_utils::{NodeExt, walk_tree};

// Re-export tree-sitter for use by language-specific extractors
pub use tree_sitter;
