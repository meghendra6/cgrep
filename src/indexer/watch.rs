// SPDX-License-Identifier: MIT OR Apache-2.0

//! File watcher for incremental index updates with debouncing

use anyhow::Result;
use colored::Colorize;
use notify::{
    Config as NotifyConfig, Event, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher,
};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::time::{Duration, Instant};

use crate::indexer::scanner::is_indexable_extension;
use crate::indexer::IndexBuilder;
use cgrep::config::Config;

/// Default debounce interval in seconds
const DEFAULT_DEBOUNCE_SECS: u64 = 15;

/// Minimum time between reindex operations
const MIN_REINDEX_INTERVAL_SECS: u64 = 180;

/// Maximum time to keep batching before forcing a reindex attempt
const DEFAULT_MAX_BATCH_DELAY_SECS: u64 = 180;

/// Watcher polling interval for platforms that use polling fallback
const WATCH_POLL_INTERVAL_SECS: u64 = 15;

/// Lower background indexing thread usage in watch mode
const WATCH_IO_THREADS: usize = 2;

/// Safety cap for adaptive min interval scaling
const MAX_ADAPTIVE_MIN_INTERVAL_SECS: u64 = 600;

/// Safety cap for adaptive debounce scaling
const MAX_ADAPTIVE_DEBOUNCE_SECS: u64 = 120;

/// File system watcher with debouncing
pub struct Watcher {
    root: PathBuf,
    builder: IndexBuilder,
    exclude_patterns: Vec<String>,
    writer_budget_bytes: usize,
    debounce_duration: Duration,
    min_reindex_interval: Duration,
    max_batch_delay: Duration,
    adaptive: bool,
}

impl Watcher {
    #[allow(clippy::too_many_arguments)]
    pub fn with_options(
        root: impl AsRef<Path>,
        builder: IndexBuilder,
        exclude_patterns: Vec<String>,
        writer_budget_bytes: usize,
        debounce_secs: u64,
        min_interval_secs: u64,
        max_batch_delay_secs: u64,
        adaptive: bool,
    ) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            builder,
            exclude_patterns,
            writer_budget_bytes,
            debounce_duration: Duration::from_secs(debounce_secs.max(1)),
            min_reindex_interval: Duration::from_secs(min_interval_secs.max(1)),
            max_batch_delay: Duration::from_secs(max_batch_delay_secs.max(1)),
            adaptive,
        }
    }

    /// Start watching for file changes with debouncing
    pub fn watch(&self) -> Result<()> {
        let (tx, rx) = channel();

        let config = NotifyConfig::default()
            .with_poll_interval(Duration::from_secs(WATCH_POLL_INTERVAL_SECS));

        let mut watcher = RecommendedWatcher::new(tx, config)?;
        watcher.watch(&self.root, RecursiveMode::Recursive)?;

        println!(
            "{} Watching {} for changes...",
            "👁".cyan(),
            self.root.display()
        );
        println!(
            "  Debounce: {}s, Min interval: {}s",
            self.debounce_duration.as_secs(),
            self.min_reindex_interval.as_secs()
        );
        println!(
            "  Max batch delay: {}s, Adaptive: {}",
            self.max_batch_delay.as_secs(),
            if self.adaptive { "on" } else { "off" }
        );
        println!("Press Ctrl+C to stop\n");

        // Track pending changes and last reindex time
        let mut pending_paths: HashSet<PathBuf> = HashSet::new();
        let mut pending_since: Option<Instant> = None;
        let mut last_event_time: Option<Instant> = None;
        // Treat startup as the first cycle boundary so background reindex runs
        // are naturally spaced out from initial index creation.
        let mut last_reindex_time: Option<Instant> = Some(Instant::now());
        let mut last_reindex_duration: Option<Duration> = None;

        loop {
            // Use timeout to implement debouncing
            let timeout = if pending_paths.is_empty() {
                Duration::from_secs(60) // Long timeout when idle
            } else {
                effective_debounce(
                    self.debounce_duration,
                    self.adaptive,
                    pending_paths.len(),
                    last_reindex_duration,
                )
            };

            match rx.recv_timeout(timeout) {
                Ok(Ok(event)) => {
                    if should_reindex(&event) {
                        let mut accepted = false;
                        // Collect changed paths
                        for path in &event.paths {
                            if !should_track_path(&self.root, path, &self.exclude_patterns) {
                                continue;
                            }
                            accepted |= pending_paths.insert(path.clone());
                        }
                        if accepted {
                            let now = Instant::now();
                            last_event_time = Some(now);
                            if pending_since.is_none() {
                                pending_since = Some(now);
                            }
                        }
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("{} Watch error: {}", "✗".red(), e);
                }
                Err(RecvTimeoutError::Timeout) => {
                    // Check if we should flush pending changes
                }
                Err(RecvTimeoutError::Disconnected) => {
                    break;
                }
            }

            // Check if we should trigger reindex
            if !pending_paths.is_empty() {
                let current_debounce = effective_debounce(
                    self.debounce_duration,
                    self.adaptive,
                    pending_paths.len(),
                    last_reindex_duration,
                );
                let should_reindex = if let Some(last_event) = last_event_time {
                    // Debounce: wait for debounce_duration since last event
                    last_event.elapsed() >= current_debounce
                } else {
                    false
                };
                let force_flush = pending_since
                    .map(|since| since.elapsed() >= self.max_batch_delay)
                    .unwrap_or(false);

                let current_min_interval = effective_min_interval(
                    self.min_reindex_interval,
                    self.adaptive,
                    last_reindex_duration,
                );

                let can_reindex = if let Some(last_reindex) = last_reindex_time {
                    // Rate limit: ensure minimum interval between reindexes
                    last_reindex.elapsed() >= current_min_interval
                } else {
                    true
                };

                if (should_reindex || force_flush) && can_reindex {
                    let changed_paths: Vec<PathBuf> = pending_paths.iter().cloned().collect();
                    let num_changes = changed_paths.len();
                    println!(
                        "{} {} file(s) changed, reindexing... (debounce={}s min_interval={}s)",
                        "🔄".yellow(),
                        num_changes,
                        current_debounce.as_secs(),
                        current_min_interval.as_secs()
                    );

                    // Clear pending before reindex to capture new events during reindex
                    pending_paths.clear();
                    pending_since = None;
                    last_event_time = None;

                    let start = Instant::now();
                    if let Err(e) = self.builder.update_paths_with_io_threads(
                        &changed_paths,
                        self.writer_budget_bytes,
                        Some(WATCH_IO_THREADS),
                    ) {
                        eprintln!("{} Reindex failed: {}", "✗".red(), e);
                    } else {
                        let elapsed = start.elapsed();
                        println!(
                            "{} Reindex complete in {:.1}s ({} paths)",
                            "✓".green(),
                            elapsed.as_secs_f64(),
                            num_changes
                        );
                        last_reindex_duration = Some(elapsed);
                    }

                    last_reindex_time = Some(Instant::now());
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

fn effective_min_interval(
    base: Duration,
    adaptive: bool,
    last_reindex_duration: Option<Duration>,
) -> Duration {
    if !adaptive {
        return base;
    }
    let Some(last_duration) = last_reindex_duration else {
        return base;
    };

    let scaled = scale_duration(
        last_duration,
        2.0,
        Duration::from_secs(MAX_ADAPTIVE_MIN_INTERVAL_SECS),
    );
    base.max(scaled)
}

fn effective_debounce(
    base: Duration,
    adaptive: bool,
    pending_count: usize,
    last_reindex_duration: Option<Duration>,
) -> Duration {
    if !adaptive {
        return base;
    }

    let mut effective = base;
    if pending_count >= 1000 {
        effective = effective.max(Duration::from_secs(30));
    } else if pending_count >= 200 {
        effective = effective.max(Duration::from_secs(15));
    }

    if let Some(last_duration) = last_reindex_duration {
        let scaled = scale_duration(
            last_duration,
            1.25,
            Duration::from_secs(MAX_ADAPTIVE_DEBOUNCE_SECS),
        );
        effective = effective.max(scaled);
    }

    effective
}

fn scale_duration(duration: Duration, factor: f64, max: Duration) -> Duration {
    let secs = duration.as_secs_f64() * factor;
    Duration::from_secs_f64(secs).min(max)
}

fn should_track_path(root: &Path, path: &Path, exclude_patterns: &[String]) -> bool {
    let relative = path.strip_prefix(root).unwrap_or(path);
    if relative.as_os_str().is_empty() {
        return false;
    }

    for component in relative.components() {
        if let Component::Normal(name) = component {
            let Some(name) = name.to_str() else { continue };
            if matches!(name, ".cgrep" | ".git" | ".hg" | ".svn") {
                return false;
            }
        }
    }

    let rel_str = relative.to_string_lossy();
    if exclude_patterns
        .iter()
        .any(|pattern| !pattern.is_empty() && rel_str.contains(pattern))
    {
        return false;
    }

    let file_name = relative.file_name().and_then(|f| f.to_str()).unwrap_or("");
    if file_name.starts_with(".#")
        || file_name.ends_with('~')
        || file_name.ends_with(".tmp")
        || file_name.ends_with(".swp")
        || file_name.ends_with(".swo")
    {
        return false;
    }

    let Some(ext) = relative.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    is_indexable_extension(ext)
}

/// Run the watch command
pub fn run(
    path: Option<&str>,
    debounce_secs: Option<u64>,
    min_interval_secs: Option<u64>,
    max_batch_delay_secs: Option<u64>,
    adaptive: bool,
) -> Result<()> {
    let root = path
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| anyhow::anyhow!("Cannot determine current directory"))?;
    let root = root.canonicalize().unwrap_or(root);

    let config = Config::load_for_dir(&root);
    let index_options = crate::indexer::index::resolve_index_options_for_watch(&root, &config);
    let excludes = index_options.exclude_paths.clone();
    let builder = IndexBuilder::with_excludes_and_symbols(
        &root,
        excludes.clone(),
        index_options.include_paths.clone(),
        index_options.respect_git_ignore,
        index_options.high_memory,
        config.embeddings.symbol_preview_lines(),
        config.embeddings.symbol_max_chars(),
        config.embeddings.max_symbols_per_file(),
        config
            .embeddings
            .symbol_kinds()
            .map(|kinds| kinds.into_iter().collect()),
    )?;
    let writer_budget_bytes = index_options.writer_budget_bytes();
    if index_options.high_memory {
        eprintln!("Using high-memory indexing in watch mode: writer budget = 1GiB");
    }

    // Build initial index
    builder.build_with_io_threads(false, writer_budget_bytes, Some(WATCH_IO_THREADS))?;

    let watcher = Watcher::with_options(
        &root,
        builder,
        excludes,
        writer_budget_bytes,
        debounce_secs.unwrap_or(DEFAULT_DEBOUNCE_SECS),
        min_interval_secs.unwrap_or(MIN_REINDEX_INTERVAL_SECS),
        max_batch_delay_secs.unwrap_or(DEFAULT_MAX_BATCH_DELAY_SECS),
        adaptive,
    );
    watcher.watch()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_path_filters_ignored_dirs_and_exts() {
        let root = PathBuf::from("/repo");
        assert!(!should_track_path(&root, Path::new("/repo/.git/HEAD"), &[]));
        assert!(!should_track_path(
            &root,
            Path::new("/repo/src/temp.py.swp"),
            &[]
        ));
        assert!(!should_track_path(
            &root,
            Path::new("/repo/docs/readme.adoc"),
            &[]
        ));
        assert!(should_track_path(&root, Path::new("/repo/src/lib.rs"), &[]));
    }

    #[test]
    fn track_path_respects_excludes() {
        let root = PathBuf::from("/repo");
        let excludes = vec!["vendor/".to_string(), "third_party".to_string()];
        assert!(!should_track_path(
            &root,
            Path::new("/repo/vendor/mod.rs"),
            &excludes
        ));
        assert!(!should_track_path(
            &root,
            Path::new("/repo/src/third_party/item.py"),
            &excludes
        ));
        assert!(should_track_path(
            &root,
            Path::new("/repo/src/main.rs"),
            &excludes
        ));
    }

    #[test]
    fn adaptive_intervals_scale_with_recent_cost() {
        let base_min = Duration::from_secs(5);
        let base_debounce = Duration::from_secs(2);
        let expensive = Duration::from_secs(20);

        let min_interval = effective_min_interval(base_min, true, Some(expensive));
        let debounce = effective_debounce(base_debounce, true, 500, Some(expensive));

        assert!(min_interval >= Duration::from_secs(40));
        assert!(debounce >= Duration::from_secs(25));
    }
}
