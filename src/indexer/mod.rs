// SPDX-License-Identifier: MIT OR Apache-2.0

//! Indexer module - handles file scanning, indexing, and watching

pub mod daemon;
pub mod index;
pub mod scanner;
pub mod watch;

pub use index::IndexBuilder;
