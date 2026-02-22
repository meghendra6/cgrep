// SPDX-License-Identifier: MIT OR Apache-2.0

//! Background indexing daemon management.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const PID_FILE_NAME: &str = "watch.pid";
const LOG_FILE_NAME: &str = "watch.log";

fn resolve_root(path: Option<&str>) -> Result<PathBuf> {
    let root = path
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| anyhow::anyhow!("Cannot determine current directory"))?;
    Ok(root.canonicalize().unwrap_or(root))
}

fn state_dir(root: &Path) -> PathBuf {
    root.join(".cgrep")
}

fn pid_file(root: &Path) -> PathBuf {
    state_dir(root).join(PID_FILE_NAME)
}

fn log_file(root: &Path) -> PathBuf {
    state_dir(root).join(LOG_FILE_NAME)
}

fn read_pid(path: &Path) -> Result<Option<u32>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)?;
    let pid = raw.trim().parse::<u32>().ok();
    Ok(pid)
}

fn write_pid(path: &Path, pid: u32) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .with_context(|| format!("failed to open pid file {}", path.display()))?;
    writeln!(file, "{}", pid)?;
    Ok(())
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn process_alive(_pid: u32) -> bool {
    false
}

#[cfg(unix)]
fn terminate_process(pid: u32) -> bool {
    Command::new("kill")
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn terminate_process(_pid: u32) -> bool {
    false
}

pub fn start(
    path: Option<&str>,
    debounce: u64,
    min_interval: u64,
    max_batch_delay: u64,
    adaptive: bool,
) -> Result<()> {
    let root = resolve_root(path)?;
    let state = state_dir(&root);
    fs::create_dir_all(&state)?;

    let pid_path = pid_file(&root);
    if let Some(pid) = read_pid(&pid_path)? {
        if process_alive(pid) {
            println!(
                "{} Indexing daemon already running (pid={})",
                "✓".green(),
                pid
            );
            println!("  Root: {}", root.display());
            return Ok(());
        }
        let _ = fs::remove_file(&pid_path);
    }

    let log_path = log_file(&root);
    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("failed to open log file {}", log_path.display()))?;
    let stderr = stdout
        .try_clone()
        .context("failed to clone log file handle")?;

    let exe = std::env::current_exe().context("failed to resolve current executable")?;
    let mut cmd = Command::new(exe);
    cmd.current_dir(&root)
        .arg("daemon")
        .arg("__watch-worker")
        .arg("--path")
        .arg(root.as_os_str())
        .arg("--debounce")
        .arg(debounce.to_string())
        .arg("--min-interval")
        .arg(min_interval.to_string())
        .arg("--max-batch-delay")
        .arg(max_batch_delay.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    if !adaptive {
        cmd.arg("--no-adaptive");
    }

    let child = cmd.spawn().context("failed to start indexing daemon")?;
    let pid = child.id();
    write_pid(&pid_path, pid)?;

    println!(
        "{} Indexing daemon started (pid={})",
        "✓".green(),
        pid.to_string().cyan()
    );
    println!("  Root: {}", root.display());
    println!("  Log:  {}", log_path.display());
    println!("  PID:  {}", pid_path.display());

    Ok(())
}

pub fn stop(path: Option<&str>) -> Result<()> {
    let root = resolve_root(path)?;
    let pid_path = pid_file(&root);
    let Some(pid) = read_pid(&pid_path)? else {
        println!("{} Indexing daemon is not running", "✗".yellow());
        return Ok(());
    };

    if !process_alive(pid) {
        let _ = fs::remove_file(&pid_path);
        println!(
            "{} Indexing daemon was not running (stale pid removed)",
            "✗".yellow()
        );
        return Ok(());
    }

    if !terminate_process(pid) {
        anyhow::bail!("Failed to stop indexing daemon process {}", pid);
    }

    let _ = fs::remove_file(&pid_path);
    println!(
        "{} Indexing daemon stopped (pid={})",
        "✓".green(),
        pid.to_string().cyan()
    );
    Ok(())
}

pub fn status(path: Option<&str>) -> Result<()> {
    let root = resolve_root(path)?;
    let pid_path = pid_file(&root);
    let log_path = log_file(&root);

    let Some(pid) = read_pid(&pid_path)? else {
        println!("{} Indexing daemon: not running", "✗".yellow());
        println!("  Root: {}", root.display());
        return Ok(());
    };

    let alive = process_alive(pid);
    if alive {
        println!(
            "{} Indexing daemon: running (pid={})",
            "✓".green(),
            pid.to_string().cyan()
        );
    } else {
        println!(
            "{} Indexing daemon: stale pid file (pid={})",
            "✗".yellow(),
            pid
        );
    }
    println!("  Root: {}", root.display());
    println!("  Log:  {}", log_path.display());
    println!("  PID:  {}", pid_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pid_roundtrip() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("pid");
        write_pid(&path, 4242).expect("write pid");
        assert_eq!(read_pid(&path).expect("read pid"), Some(4242));
    }
}
