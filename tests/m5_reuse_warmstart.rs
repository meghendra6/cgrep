// SPDX-License-Identifier: MIT OR Apache-2.0

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, content).expect("write file");
}

fn write_fixture(root: &Path, files: usize, marker: &str) {
    let src = root.join("src");
    fs::create_dir_all(&src).expect("create src");
    for i in 0..files {
        let tag = if i % 19 == 0 { marker } else { "noise" };
        let body = "    let value = input + 1;\n".repeat(20);
        let content = format!(
            "pub fn worker_{i}(input: i32) -> i32 {{\n    // {tag}\n{body}    input + {i}\n}}\n"
        );
        write_file(&src.join(format!("mod_{i}.rs")), &content);
    }
}

fn run_git(dir: &Path, args: &[&str]) -> String {
    let output = StdCommand::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("run git");
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("git {:?} failed: {}", args, stderr);
    }
    String::from_utf8(output.stdout).expect("git stdout utf8")
}

fn init_repo(path: &Path) {
    fs::create_dir_all(path).expect("mkdir repo");
    run_git(path, &["init"]);
    run_git(path, &["config", "user.email", "ci@example.com"]);
    run_git(path, &["config", "user.name", "CI"]);
}

fn commit_all(path: &Path, message: &str) -> String {
    run_git(path, &["add", "."]);
    run_git(path, &["commit", "-m", message]);
    run_git(path, &["rev-parse", "HEAD"]).trim().to_string()
}

fn setup_origin(seed: &Path, origin: &Path) {
    let origin_str = origin.to_string_lossy().to_string();
    run_git(seed, &["branch", "-M", "main"]);
    run_git(seed, &["init", "--bare", &origin_str]);
    run_git(seed, &["remote", "add", "origin", &origin_str]);
    run_git(seed, &["push", "-u", "origin", "main"]);
}

fn clone_origin(origin: &Path, destination: &Path) {
    let parent = destination.parent().expect("clone parent");
    fs::create_dir_all(parent).expect("mkdir parent");
    let origin_str = origin.to_string_lossy().to_string();
    let dest_str = destination.to_string_lossy().to_string();
    run_git(parent, &["clone", &origin_str, &dest_str]);
    run_git(destination, &["config", "user.email", "ci@example.com"]);
    run_git(destination, &["config", "user.name", "CI"]);
}

fn run_cgrep_success(dir: &Path, cache_root: &Path, args: &[&str]) -> (String, String) {
    run_cgrep_success_with_env(dir, cache_root, args, &[])
}

fn run_cgrep_success_with_env(
    dir: &Path,
    cache_root: &Path,
    args: &[&str],
    extra_env: &[(&str, &str)],
) -> (String, String) {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    cmd.current_dir(dir)
        .env("CGREP_REUSE_CACHE_DIR", cache_root)
        .args(args);
    for (key, value) in extra_env {
        cmd.env(key, value);
    }
    let output = cmd.assert().success().get_output().clone();
    (
        String::from_utf8(output.stdout).expect("stdout utf8"),
        String::from_utf8(output.stderr).expect("stderr utf8"),
    )
}

fn run_cgrep_json2(dir: &Path, cache_root: &Path, args: &[&str]) -> Value {
    let mut full = vec!["--format", "json2", "--compact"];
    full.extend_from_slice(args);
    let (stdout, _) = run_cgrep_success(dir, cache_root, &full);
    serde_json::from_str(&stdout).expect("json2 parse")
}

fn result_paths(payload: &Value) -> Vec<String> {
    let mut paths = payload["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| {
            row.get("path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn reuse_state(root: &Path) -> Value {
    let path = root.join(".cgrep").join("reuse-state.json");
    let raw = fs::read_to_string(path).expect("read reuse state");
    serde_json::from_str(&raw).expect("parse reuse state")
}

fn wait_for_reuse_active(root: &Path, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        let path = root.join(".cgrep").join("reuse-state.json");
        if path.exists() {
            if let Ok(raw) = fs::read_to_string(&path) {
                if let Ok(json) = serde_json::from_str::<Value>(&raw) {
                    if json.get("active").and_then(|v| v.as_bool()) == Some(true) {
                        return true;
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(40));
    }
    false
}

#[cfg(unix)]
fn kill_pid(pid: u32) {
    let _ = StdCommand::new("kill")
        .args(["-9", &pid.to_string()])
        .status();
}

#[cfg(not(unix))]
fn kill_pid(_pid: u32) {}

fn cleanup_background(root: &Path) {
    let status_path = root.join(".cgrep").join("status.json");
    if !status_path.exists() {
        return;
    }
    let Ok(raw) = fs::read_to_string(status_path) else {
        return;
    };
    let Ok(json) = serde_json::from_str::<Value>(&raw) else {
        return;
    };
    if let Some(pid) = json.get("pid").and_then(|v| v.as_u64()) {
        kill_pid(pid as u32);
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
fn reuse_strict_hits_exact_commit_and_is_faster_than_off() {
    let dir = TempDir::new().expect("tempdir");
    let cache = dir.path().join("cache");
    let seed = dir.path().join("seed");
    let origin = dir.path().join("origin.git");
    init_repo(&seed);
    write_fixture(&seed, 500, "m5_strict_seed");
    let head = commit_all(&seed, "seed");
    setup_origin(&seed, &origin);

    let _ = run_cgrep_success(
        &seed,
        &cache,
        &["index", "--reuse", "strict", "--embeddings", "off"],
    );

    let clone_off = dir.path().join("clone_off");
    let clone_strict = dir.path().join("clone_strict");
    clone_origin(&origin, &clone_off);
    clone_origin(&origin, &clone_strict);

    let off_start = Instant::now();
    let _ = run_cgrep_success(
        &clone_off,
        &cache,
        &["index", "--reuse", "off", "--embeddings", "off"],
    );
    let off_elapsed = off_start.elapsed();

    let strict_start = Instant::now();
    let _ = run_cgrep_success(
        &clone_strict,
        &cache,
        &["index", "--reuse", "strict", "--embeddings", "off"],
    );
    let strict_elapsed = strict_start.elapsed();

    assert!(
        strict_elapsed < off_elapsed,
        "strict reuse should be faster than off: strict={strict_elapsed:?}, off={off_elapsed:?}"
    );

    let state = reuse_state(&clone_strict);
    assert_eq!(state["decision"], "hit");
    assert_eq!(state["source"], "strict");
    assert_eq!(state["snapshot_key"], head);
}

#[test]
fn reuse_auto_selects_nearest_snapshot_deterministically() {
    let dir = TempDir::new().expect("tempdir");
    let cache = dir.path().join("cache");
    let seed = dir.path().join("seed");
    let origin = dir.path().join("origin.git");
    init_repo(&seed);
    write_file(
        &seed.join("src/core.rs"),
        "pub fn nearest_marker() { let tag = \"nearest_core_a\"; }\n",
    );
    write_file(
        &seed.join("src/legacy.rs"),
        "pub fn legacy_marker() { let tag = \"nearest_legacy_a\"; }\n",
    );
    let _commit_a = commit_all(&seed, "commit a");
    setup_origin(&seed, &origin);
    let _ = run_cgrep_success(
        &seed,
        &cache,
        &["index", "--reuse", "strict", "--embeddings", "off"],
    );

    write_file(
        &seed.join("src/core.rs"),
        "pub fn nearest_marker() { let tag = \"nearest_core_b\"; }\n",
    );
    write_file(
        &seed.join("src/legacy.rs"),
        "pub fn legacy_marker() { let tag = \"nearest_legacy_b\"; }\n",
    );
    let commit_b = commit_all(&seed, "commit b");
    run_git(&seed, &["push", "origin", "main"]);
    let _ = run_cgrep_success(
        &seed,
        &cache,
        &["index", "--reuse", "strict", "--embeddings", "off"],
    );

    write_file(
        &seed.join("src/core.rs"),
        "pub fn nearest_marker() { let tag = \"nearest_core_c\"; }\n",
    );
    fs::remove_file(seed.join("src/legacy.rs")).expect("remove legacy");
    let _commit_c = commit_all(&seed, "commit c");
    run_git(&seed, &["push", "origin", "main"]);

    let clone_auto = dir.path().join("clone_auto");
    clone_origin(&origin, &clone_auto);
    let _ = run_cgrep_success(
        &clone_auto,
        &cache,
        &["index", "--reuse", "auto", "--embeddings", "off"],
    );
    let state = reuse_state(&clone_auto);
    assert_eq!(state["decision"], "hit");
    assert_eq!(state["source"], "auto");
    assert_eq!(state["snapshot_key"], commit_b);

    let latest = run_cgrep_json2(
        &clone_auto,
        &cache,
        &["search", "nearest_core_c", "--limit", "20"],
    );
    assert!(latest["results"]
        .as_array()
        .map(|r| !r.is_empty())
        .unwrap_or(false));

    let stale = run_cgrep_json2(
        &clone_auto,
        &cache,
        &["search", "nearest_legacy_b", "--limit", "20"],
    );
    assert_eq!(stale["results"].as_array().map(|r| r.len()).unwrap_or(0), 0);
}

#[test]
fn stale_deleted_files_do_not_leak_during_reuse_window() {
    let dir = TempDir::new().expect("tempdir");
    let cache = dir.path().join("cache");
    let seed = dir.path().join("seed");
    let origin = dir.path().join("origin.git");
    init_repo(&seed);
    write_fixture(&seed, 1200, "m5_bg_noise");
    write_file(
        &seed.join("src/stale.rs"),
        "pub fn stale() { let token = \"m5_stale_deleted_probe\"; }\n",
    );
    let _ = commit_all(&seed, "stale commit");
    setup_origin(&seed, &origin);
    let _ = run_cgrep_success(
        &seed,
        &cache,
        &["index", "--reuse", "strict", "--embeddings", "off"],
    );

    fs::remove_file(seed.join("src/stale.rs")).expect("delete stale file");
    write_file(
        &seed.join("src/live.rs"),
        "pub fn live() { let token = \"m5_live_probe\"; }\n",
    );
    let _ = commit_all(&seed, "post-stale");
    run_git(&seed, &["push", "origin", "main"]);

    let clone = dir.path().join("clone_bg");
    clone_origin(&origin, &clone);
    let _guard = BackgroundGuard::new(&clone);
    let _ = run_cgrep_success_with_env(
        &clone,
        &cache,
        &[
            "index",
            "--reuse",
            "auto",
            "--background",
            "--embeddings",
            "off",
        ],
        &[("CGREP_REUSE_HOLD_MS", "1500")],
    );

    assert!(
        wait_for_reuse_active(&clone, Duration::from_secs(10)),
        "expected active reuse window"
    );

    let stale = run_cgrep_json2(
        &clone,
        &cache,
        &["search", "m5_stale_deleted_probe", "--limit", "20"],
    );
    assert_eq!(stale["results"].as_array().map(|r| r.len()).unwrap_or(0), 0);
}

#[test]
fn reuse_off_matches_default_behavior() {
    let dir = TempDir::new().expect("tempdir");
    let cache = dir.path().join("cache");
    let seed = dir.path().join("seed");
    let origin = dir.path().join("origin.git");
    init_repo(&seed);
    write_fixture(&seed, 120, "m5_off_probe");
    let _ = commit_all(&seed, "seed");
    setup_origin(&seed, &origin);

    let clone_default = dir.path().join("clone_default");
    let clone_off = dir.path().join("clone_off");
    clone_origin(&origin, &clone_default);
    clone_origin(&origin, &clone_off);

    let _ = run_cgrep_success(&clone_default, &cache, &["index", "--embeddings", "off"]);
    let _ = run_cgrep_success(
        &clone_off,
        &cache,
        &["index", "--reuse", "off", "--embeddings", "off"],
    );

    let default_payload = run_cgrep_json2(
        &clone_default,
        &cache,
        &["search", "m5_off_probe", "--limit", "30"],
    );
    let off_payload = run_cgrep_json2(
        &clone_off,
        &cache,
        &["search", "m5_off_probe", "--limit", "30"],
    );
    assert_eq!(
        result_paths(&default_payload),
        result_paths(&off_payload),
        "reuse off should preserve result set"
    );
}

#[test]
fn corrupt_snapshot_falls_back_safely_with_reason() {
    let dir = TempDir::new().expect("tempdir");
    let cache = dir.path().join("cache");
    let seed = dir.path().join("seed");
    let origin = dir.path().join("origin.git");
    init_repo(&seed);
    write_file(
        &seed.join("src/lib.rs"),
        "pub fn fallback_probe() { let token = \"m5_corrupt_fallback\"; }\n",
    );
    let head = commit_all(&seed, "seed");
    setup_origin(&seed, &origin);
    let _ = run_cgrep_success(
        &seed,
        &cache,
        &["index", "--reuse", "strict", "--embeddings", "off"],
    );

    let seed_state = reuse_state(&seed);
    let repo_key = seed_state["repo_key"].as_str().expect("repo key");
    let snapshot_key = seed_state["snapshot_key"].as_str().unwrap_or(&head);
    let corrupt_meta = cache
        .join(repo_key)
        .join(snapshot_key)
        .join("tantivy")
        .join("meta.json");
    fs::remove_file(&corrupt_meta).expect("remove meta.json");

    let clone = dir.path().join("clone_corrupt");
    clone_origin(&origin, &clone);
    let _ = run_cgrep_success(
        &clone,
        &cache,
        &["index", "--reuse", "strict", "--embeddings", "off"],
    );
    let state = reuse_state(&clone);
    assert_eq!(state["decision"], "fallback");
    assert_eq!(state["reason"], "snapshot_corrupt");

    let payload = run_cgrep_json2(
        &clone,
        &cache,
        &["search", "m5_corrupt_fallback", "--limit", "20"],
    );
    assert!(payload["results"]
        .as_array()
        .map(|r| !r.is_empty())
        .unwrap_or(false));
}

#[test]
fn json2_outputs_are_deterministic_with_reuse_state() {
    let dir = TempDir::new().expect("tempdir");
    let cache = dir.path().join("cache");
    let seed = dir.path().join("seed");
    let origin = dir.path().join("origin.git");
    init_repo(&seed);
    write_file(
        &seed.join("src/lib.rs"),
        "pub fn det_probe() { let token = \"m5_determinism_probe\"; }\n",
    );
    let _ = commit_all(&seed, "seed");
    setup_origin(&seed, &origin);
    let _ = run_cgrep_success(
        &seed,
        &cache,
        &["index", "--reuse", "strict", "--embeddings", "off"],
    );

    let clone = dir.path().join("clone_det");
    clone_origin(&origin, &clone);
    let _ = run_cgrep_success(
        &clone,
        &cache,
        &["index", "--reuse", "strict", "--embeddings", "off"],
    );

    let status_a = run_cgrep_json2(&clone, &cache, &["status"]);
    let status_b = run_cgrep_json2(&clone, &cache, &["status"]);
    assert_eq!(status_a, status_b);

    let search_a = run_cgrep_json2(
        &clone,
        &cache,
        &["search", "m5_determinism_probe", "--limit", "20"],
    );
    let search_b = run_cgrep_json2(
        &clone,
        &cache,
        &["search", "m5_determinism_probe", "--limit", "20"],
    );
    assert_eq!(search_a["results"], search_b["results"]);
}
