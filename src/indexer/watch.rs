//! File watcher for incremental index updates

use anyhow::Result;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher, Event};
use std::path::Path;
use std::sync::mpsc::channel;
use std::time::Duration;
use colored::Colorize;

use crate::indexer::IndexBuilder;

/// File system watcher
pub struct Watcher {
    root: std::path::PathBuf,
}

impl Watcher {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    /// Start watching for file changes
    pub fn watch(&self) -> Result<()> {
        let (tx, rx) = channel();

        let config = Config::default()
            .with_poll_interval(Duration::from_secs(2));

        let mut watcher = RecommendedWatcher::new(tx, config)?;

        watcher.watch(&self.root, RecursiveMode::Recursive)?;

        println!("{} Watching {} for changes...", "👁".cyan(), self.root.display());
        println!("Press Ctrl+C to stop\n");

        let builder = IndexBuilder::new(&self.root)?;

        for res in rx {
            match res {
                Ok(event) => {
                    if should_reindex(&event) {
                        println!("{} Change detected, reindexing...", "🔄".yellow());
                        if let Err(e) = builder.build(false) {
                            eprintln!("{} Reindex failed: {}", "✗".red(), e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("{} Watch error: {}", "✗".red(), e);
                }
            }
        }

        Ok(())
    }
}

/// Check if event should trigger reindex
fn should_reindex(event: &Event) -> bool {
    use notify::EventKind::*;

    matches!(event.kind, Create(_) | Modify(_) | Remove(_))
}

/// Run the watch command
pub fn run(path: Option<&str>) -> Result<()> {
    let root = path.map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    // Build initial index
    let builder = IndexBuilder::new(&root)?;
    builder.build(false)?;

    // Start watching
    let watcher = Watcher::new(&root);
    watcher.watch()
}
