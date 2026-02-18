// SPDX-License-Identifier: MIT OR Apache-2.0

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command as ProcessCommand;
use tempfile::TempDir;

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, content).expect("write file");
}

fn run_search(dir: &Path, query: &str) -> Value {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = cmd
        .current_dir(dir)
        .args(["--format", "json", "search", query])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    serde_json::from_str(&stdout).expect("json")
}

fn run_git(dir: &Path, args: &[&str]) {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn init_git_repo(dir: &Path) {
    run_git(dir, &["init", "-q"]);
    run_git(dir, &["config", "user.email", "test@example.com"]);
    run_git(dir, &["config", "user.name", "test"]);
}

#[test]
fn index_respects_gitignore_by_default() {
    let dir = TempDir::new().expect("tempdir");
    init_git_repo(dir.path());
    write_file(&dir.path().join(".gitignore"), "target/\n");
    write_file(
        &dir.path().join("src/lib.rs"),
        "pub fn src_marker_index_policy() {}\n",
    );
    write_file(
        &dir.path().join("target/noise.rs"),
        "pub fn ignored_marker_index_policy() {}\n",
    );

    let mut index_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    index_cmd
        .current_dir(dir.path())
        .args(["index", "--embeddings", "off"])
        .assert()
        .success();

    let src_json = run_search(dir.path(), "src_marker_index_policy");
    let src_results = src_json.as_array().expect("results");
    assert!(src_results.iter().any(|r| r["path"] == "src/lib.rs"));

    let ignored_json = run_search(dir.path(), "ignored_marker_index_policy");
    let ignored_results = ignored_json.as_array().expect("results");
    assert!(ignored_results.is_empty());
}

#[test]
fn index_include_ignored_opt_out_includes_gitignored_files() {
    let dir = TempDir::new().expect("tempdir");
    init_git_repo(dir.path());
    write_file(&dir.path().join(".gitignore"), "target/\n");
    write_file(
        &dir.path().join("target/noise.rs"),
        "pub fn ignored_marker_include_optout() {}\n",
    );

    let mut index_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    index_cmd
        .current_dir(dir.path())
        .args(["index", "--include-ignored", "--embeddings", "off"])
        .assert()
        .success();

    let ignored_json = run_search(dir.path(), "ignored_marker_include_optout");
    let ignored_results = ignored_json.as_array().expect("results");
    assert!(!ignored_results.is_empty());
    assert!(ignored_results.iter().any(|r| {
        r["path"]
            .as_str()
            .map(|p| p.contains("target/noise.rs"))
            .unwrap_or(false)
    }));
}
