// SPDX-License-Identifier: MIT OR Apache-2.0

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, content).expect("write file");
}

fn write_fixture(root: &Path, files: usize, needle: &str) {
    let src = root.join("src");
    fs::create_dir_all(&src).expect("create src");
    for i in 0..files {
        let marker = if i % 11 == 0 { needle } else { "noise" };
        let body = "    let value = 1 + 2 + 3 + 4 + 5;\n".repeat(40);
        let content =
            format!("pub fn worker_{i}() -> i32 {{\n    // {marker}\n{body}    {i} as i32\n}}\n");
        write_file(&src.join(format!("mod_{i}.rs")), &content);
    }
}

fn run_success(dir: &Path, args: &[&str]) -> (String, String) {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let output = cmd
        .current_dir(dir)
        .args(args)
        .assert()
        .success()
        .get_output()
        .clone();
    (
        String::from_utf8(output.stdout).expect("stdout utf8"),
        String::from_utf8(output.stderr).expect("stderr utf8"),
    )
}

fn run_json2(dir: &Path, args: &[&str]) -> Value {
    let mut full_args = vec!["--format", "json2", "--compact"];
    full_args.extend_from_slice(args);
    let (stdout, _) = run_success(dir, &full_args);
    serde_json::from_str(&stdout).expect("json2 output")
}

fn status_file(root: &Path) -> PathBuf {
    root.join(".cgrep").join("status.json")
}

fn wait_for_status_file(root: &Path, timeout: Duration) -> Value {
    let start = Instant::now();
    while start.elapsed() < timeout {
        let path = status_file(root);
        if path.exists() {
            let raw = fs::read_to_string(path).expect("read status file");
            return serde_json::from_str(&raw).expect("parse status file json");
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("timed out waiting for status file");
}

#[cfg(unix)]
fn kill_pid(pid: u32) {
    let _ = std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .status();
}

#[cfg(not(unix))]
fn kill_pid(_pid: u32) {}

fn cleanup_background(root: &Path) {
    let path = status_file(root);
    if !path.exists() {
        return;
    }
    if let Ok(raw) = fs::read_to_string(path) {
        if let Ok(json) = serde_json::from_str::<Value>(&raw) {
            if let Some(pid) = json.get("pid").and_then(|v| v.as_u64()) {
                kill_pid(pid as u32);
            }
        }
    }
}

struct BackgroundGuard {
    root: PathBuf,
}

impl BackgroundGuard {
    fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }
}

impl Drop for BackgroundGuard {
    fn drop(&mut self) {
        cleanup_background(&self.root);
    }
}

#[test]
fn keyword_search_without_full_index_is_functional_and_deterministic() {
    let dir = TempDir::new().expect("tempdir");
    write_fixture(dir.path(), 30, "m3_missing_index_probe");

    let first = run_json2(
        dir.path(),
        &["search", "m3_missing_index_probe", "--limit", "20"],
    );
    let second = run_json2(
        dir.path(),
        &["search", "m3_missing_index_probe", "--limit", "20"],
    );

    assert_eq!(first["meta"]["index_mode"], "scan");
    assert!(first["results"]
        .as_array()
        .map(|v| !v.is_empty())
        .unwrap_or(false));
    assert_eq!(first["results"], second["results"]);
}

#[test]
fn background_indexing_active_search_remains_responsive_and_correct() {
    let dir = TempDir::new().expect("tempdir");
    write_fixture(dir.path(), 600, "m3_background_probe");
    let _guard = BackgroundGuard::new(dir.path());

    let start = Instant::now();
    let _ = run_success(
        dir.path(),
        &["index", "--background", "--embeddings", "off"],
    );
    let payload = run_json2(
        dir.path(),
        &["search", "m3_background_probe", "--limit", "20"],
    );
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(8),
        "search took too long: {elapsed:?}"
    );
    assert!(payload["results"]
        .as_array()
        .map(|v| !v.is_empty())
        .unwrap_or(false));
    let mode = payload["meta"]["index_mode"].as_str().unwrap_or("");
    assert!(mode == "scan" || mode == "index", "unexpected mode: {mode}");
}

#[test]
fn index_background_returns_immediately_and_updates_status_state() {
    let dir = TempDir::new().expect("tempdir");
    write_fixture(dir.path(), 400, "m3_status_probe");
    let _guard = BackgroundGuard::new(dir.path());

    let start = Instant::now();
    let _ = run_success(
        dir.path(),
        &["index", "--background", "--embeddings", "off"],
    );
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(4),
        "background command was not immediate: {elapsed:?}"
    );

    let status = wait_for_status_file(dir.path(), Duration::from_secs(5));
    assert!(status["phase"].is_string());
    assert!(status["started_at"].is_number());
    assert!(status["updated_at"].is_number());
    assert!(status["basic_ready"].is_boolean());
    assert!(status["full_ready"].is_boolean());
    assert!(status["progress"]["total"].is_number());
    assert!(status["progress"]["processed"].is_number());
    assert!(status["progress"]["failed"].is_number());
}

#[test]
fn status_recovers_after_interruption_without_corruption() {
    let dir = TempDir::new().expect("tempdir");
    write_fixture(dir.path(), 20, "m3_interrupt_probe");

    let stale_status = serde_json::json!({
        "schema_version": "1",
        "phase": "indexing",
        "started_at": 1,
        "updated_at": 1,
        "basic_ready": true,
        "full_ready": false,
        "progress": {
            "total": 20,
            "processed": 10,
            "failed": 0
        },
        "pid": 999999,
        "message": "simulated stale worker"
    });
    let path = status_file(dir.path());
    fs::create_dir_all(path.parent().expect("status parent")).expect("mkdir .cgrep");
    fs::write(
        &path,
        serde_json::to_string_pretty(&stale_status).expect("serialize stale"),
    )
    .expect("write stale status");

    let payload = run_json2(dir.path(), &["status"]);
    let phase = payload["result"]["phase"].as_str().unwrap_or("");
    assert!(
        matches!(phase, "interrupted" | "failed" | "complete"),
        "unexpected recovered phase: {phase}"
    );
    assert!(payload["result"]["pid"].is_null());
    assert!(payload["result"]["progress"]["total"].is_number());
    assert!(payload["result"]["progress"]["processed"].is_number());
    assert!(payload["result"]["progress"]["failed"].is_number());
}

#[test]
fn default_index_behavior_is_unchanged_without_background_flag() {
    let dir = TempDir::new().expect("tempdir");
    write_fixture(dir.path(), 80, "m3_default_behavior_probe");

    let _ = run_success(dir.path(), &["index", "--embeddings", "off"]);
    let payload = run_json2(
        dir.path(),
        &["search", "m3_default_behavior_probe", "--limit", "20"],
    );

    assert_eq!(payload["meta"]["index_mode"], "index");
    assert!(payload["results"]
        .as_array()
        .map(|v| !v.is_empty())
        .unwrap_or(false));
}

#[test]
fn json2_compact_status_output_is_deterministic_for_stable_state() {
    let dir = TempDir::new().expect("tempdir");
    write_fixture(dir.path(), 50, "m3_status_determinism_probe");

    let _ = run_success(dir.path(), &["index", "--embeddings", "off"]);
    let first = run_json2(dir.path(), &["status"]);
    let second = run_json2(dir.path(), &["status"]);
    assert_eq!(first, second);
}
