// SPDX-License-Identifier: MIT OR Apache-2.0

//! Index readiness and background index build status.

use anyhow::Result;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cli::OutputFormat;
use crate::indexer::manifest;
use crate::indexer::reuse;
use cgrep::output::print_json;

const STATUS_FILE_NAME: &str = "status.json";
const BACKGROUND_LOG_FILE_NAME: &str = "index-background.log";
const WATCH_PID_FILE_NAME: &str = "watch.pid";
const WATCH_LOG_FILE_NAME: &str = "watch.log";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct BuildProgress {
    pub total: usize,
    pub processed: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuildStatus {
    pub schema_version: String,
    pub phase: String,
    pub started_at: u64,
    pub updated_at: u64,
    pub basic_ready: bool,
    pub full_ready: bool,
    pub progress: BuildProgress,
    pub pid: Option<u32>,
    pub message: String,
}

impl BuildStatus {
    pub fn idle(root: &Path) -> Self {
        let now = now_unix_ms();
        Self {
            schema_version: "1".to_string(),
            phase: "idle".to_string(),
            started_at: now,
            updated_at: now,
            basic_ready: basic_ready(root),
            full_ready: full_index_ready(root),
            progress: BuildProgress::default(),
            pid: None,
            message: String::new(),
        }
    }
}

#[derive(Debug, Serialize)]
struct DaemonStatus {
    running: bool,
    stale: bool,
    pid: Option<u32>,
    pid_file: String,
    log_file: String,
}

#[derive(Debug, Serialize)]
struct StatusResult {
    root: String,
    phase: String,
    started_at: u64,
    updated_at: u64,
    basic_ready: bool,
    full_ready: bool,
    progress: BuildProgress,
    pid: Option<u32>,
    message: String,
    daemon: DaemonStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    reuse: Option<reuse::ReuseRuntimeState>,
}

#[derive(Debug, Serialize)]
struct StatusJson2Meta {
    schema_version: &'static str,
}

#[derive(Debug, Serialize)]
struct StatusJson2Payload {
    meta: StatusJson2Meta,
    result: StatusResult,
}

fn state_dir(root: &Path) -> PathBuf {
    root.join(".cgrep")
}

pub fn status_file_path(root: &Path) -> PathBuf {
    state_dir(root).join(STATUS_FILE_NAME)
}

pub fn background_log_path(root: &Path) -> PathBuf {
    state_dir(root).join(BACKGROUND_LOG_FILE_NAME)
}

fn watch_pid_file(root: &Path) -> PathBuf {
    state_dir(root).join(WATCH_PID_FILE_NAME)
}

fn watch_log_file(root: &Path) -> PathBuf {
    state_dir(root).join(WATCH_LOG_FILE_NAME)
}

pub fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn basic_ready(root: &Path) -> bool {
    root.exists()
}

pub fn full_index_ready(root: &Path) -> bool {
    state_dir(root).join("meta.json").is_file()
}

pub fn load_build_status(root: &Path) -> Option<BuildStatus> {
    let path = status_file_path(root);
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn save_build_status(root: &Path, status: &BuildStatus) -> Result<()> {
    let path = status_file_path(root);
    let content = serde_json::to_string_pretty(status)?;
    manifest::atomic_write_bytes(&path, content.as_bytes())
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn process_alive(_pid: u32) -> bool {
    false
}

fn should_recover_stale(status: &BuildStatus) -> bool {
    matches!(status.phase.as_str(), "starting" | "indexing" | "embedding")
}

pub fn recover_stale_status(root: &Path, status: &mut BuildStatus) -> bool {
    if !should_recover_stale(status) {
        return false;
    }

    let Some(pid) = status.pid else {
        return false;
    };
    if process_alive(pid) {
        return false;
    }

    status.phase = if full_index_ready(root) {
        "complete".to_string()
    } else {
        "interrupted".to_string()
    };
    status.full_ready = full_index_ready(root);
    status.basic_ready = basic_ready(root);
    status.updated_at = now_unix_ms();
    status.pid = None;
    if status.message.is_empty() {
        status.message = "background index process is not running".to_string();
    }
    true
}

pub fn read_status_with_recovery(root: &Path) -> Result<BuildStatus> {
    let mut status = match load_build_status(root) {
        Some(status) => status,
        None => {
            let initial = BuildStatus::idle(root);
            save_build_status(root, &initial)?;
            initial
        }
    };
    if recover_stale_status(root, &mut status) {
        save_build_status(root, &status)?;
    }
    Ok(status)
}

pub fn mark_build_start(
    root: &Path,
    phase: &str,
    pid: Option<u32>,
    total: usize,
    message: impl Into<String>,
) -> Result<BuildStatus> {
    let now = now_unix_ms();
    let mut status = BuildStatus::idle(root);
    status.phase = phase.to_string();
    status.started_at = now;
    status.updated_at = now;
    status.basic_ready = true;
    status.full_ready = full_index_ready(root);
    status.progress = BuildProgress {
        total,
        processed: 0,
        failed: 0,
    };
    status.pid = pid;
    status.message = message.into();
    save_build_status(root, &status)?;
    Ok(status)
}

pub fn mark_build_phase(
    root: &Path,
    status: &mut BuildStatus,
    phase: &str,
    processed: usize,
    failed: usize,
    message: impl Into<String>,
) -> Result<()> {
    status.phase = phase.to_string();
    status.updated_at = now_unix_ms();
    status.basic_ready = true;
    status.full_ready = full_index_ready(root);
    status.progress.processed = processed.min(status.progress.total);
    status.progress.failed = failed;
    status.message = message.into();
    save_build_status(root, status)
}

pub fn mark_build_complete(
    root: &Path,
    status: &mut BuildStatus,
    message: impl Into<String>,
) -> Result<()> {
    let total = status.progress.total;
    status.phase = "complete".to_string();
    status.updated_at = now_unix_ms();
    status.basic_ready = true;
    status.full_ready = full_index_ready(root);
    status.progress.processed = total;
    status.pid = None;
    status.message = message.into();
    save_build_status(root, status)
}

pub fn mark_build_failed(
    root: &Path,
    status: &mut BuildStatus,
    message: impl Into<String>,
) -> Result<()> {
    status.phase = "failed".to_string();
    status.updated_at = now_unix_ms();
    status.basic_ready = true;
    status.full_ready = full_index_ready(root);
    status.progress.failed = status.progress.failed.saturating_add(1);
    status.pid = None;
    status.message = message.into();
    save_build_status(root, status)
}

fn read_pid(path: &Path) -> Option<u32> {
    let raw = fs::read_to_string(path).ok()?;
    raw.trim().parse::<u32>().ok()
}

fn daemon_status(root: &Path) -> DaemonStatus {
    let pid_file = watch_pid_file(root);
    let log_file = watch_log_file(root);
    let pid = read_pid(&pid_file);
    let running = pid.map(process_alive).unwrap_or(false);
    let stale = pid.is_some() && !running;
    DaemonStatus {
        running,
        stale,
        pid,
        pid_file: pid_file.display().to_string(),
        log_file: log_file.display().to_string(),
    }
}

fn resolve_root(path: Option<&str>) -> Result<PathBuf> {
    let root = path
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| anyhow::anyhow!("Cannot determine current directory"))?;
    Ok(root.canonicalize().unwrap_or(root))
}

pub fn run(path: Option<&str>, format: OutputFormat, compact: bool) -> Result<()> {
    let root = resolve_root(path)?;
    let status = read_status_with_recovery(&root)?;
    let daemon = daemon_status(&root);
    let reuse_state = reuse::load_runtime_state(&root);
    let result = StatusResult {
        root: root.display().to_string(),
        phase: status.phase.clone(),
        started_at: status.started_at,
        updated_at: status.updated_at,
        basic_ready: status.basic_ready,
        full_ready: status.full_ready,
        progress: status.progress.clone(),
        pid: status.pid,
        message: status.message.clone(),
        daemon,
        reuse: reuse_state,
    };

    match format {
        OutputFormat::Text => {
            println!("Index root: {}", result.root);
            println!(
                "Basic readiness: {}",
                if result.basic_ready {
                    "ready".green().to_string()
                } else {
                    "not-ready".yellow().to_string()
                }
            );
            println!(
                "Full readiness: {}",
                if result.full_ready {
                    "ready".green().to_string()
                } else {
                    "not-ready".yellow().to_string()
                }
            );
            println!("Background phase: {}", result.phase);
            println!(
                "Progress: {}/{} (failed: {})",
                result.progress.processed, result.progress.total, result.progress.failed
            );
            if let Some(pid) = result.pid {
                println!("Background pid: {}", pid);
            }
            if !result.message.is_empty() {
                println!("Message: {}", result.message);
            }
            if let Some(reuse) = result.reuse.as_ref() {
                let mut detail = format!("decision={}", reuse.decision);
                if let Some(source) = reuse.source.as_ref() {
                    detail.push_str(&format!(", source={source}"));
                }
                if let Some(snapshot) = reuse.snapshot_key.as_ref() {
                    detail.push_str(&format!(", snapshot={snapshot}"));
                }
                if let Some(reason) = reuse.reason.as_ref() {
                    detail.push_str(&format!(", reason={reason}"));
                }
                detail.push_str(&format!(", active={}", reuse.active));
                println!("Reuse: {}", detail);
            }
            if result.daemon.running {
                println!(
                    "Watch daemon: running (pid={})",
                    result.daemon.pid.unwrap_or(0)
                );
            } else if result.daemon.stale {
                println!(
                    "Watch daemon: stale pid file (pid={})",
                    result.daemon.pid.unwrap_or(0)
                );
            } else {
                println!("Watch daemon: not running");
            }
            println!("Watch pid file: {}", result.daemon.pid_file);
            println!("Watch log file: {}", result.daemon.log_file);
        }
        OutputFormat::Json => {
            print_json(&result, compact)?;
        }
        OutputFormat::Json2 => {
            let payload = StatusJson2Payload {
                meta: StatusJson2Meta {
                    schema_version: "1",
                },
                result,
            };
            print_json(&payload, compact)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_status_tracks_full_index_readiness() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let root = dir.path();
        let idle = BuildStatus::idle(root);
        assert!(!idle.full_ready);
        fs::create_dir_all(root.join(".cgrep")).expect("mkdir .cgrep");
        fs::write(root.join(".cgrep/meta.json"), "{}").expect("write meta");
        let with_index = BuildStatus::idle(root);
        assert!(with_index.full_ready);
    }
}
