//! Indexer module - handles file scanning, indexing, and watching

pub mod index;
pub mod scanner;
pub mod watch;

pub use index::IndexBuilder;
