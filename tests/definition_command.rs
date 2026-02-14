// SPDX-License-Identifier: MIT OR Apache-2.0

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use tempfile::TempDir;

fn write_file(path: &std::path::Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, content).expect("write file");
}

#[test]
fn definition_prefers_exact_match_over_partial() {
    let dir = TempDir::new().expect("tempdir");
    write_file(&dir.path().join("run.rs"), "pub fn run() {}\n");
    write_file(&dir.path().join("runner.rs"), "pub fn runner() {}\n");

    let mut index_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    index_cmd
        .current_dir(dir.path())
        .args(["index"])
        .assert()
        .success();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = cmd
        .current_dir(dir.path())
        .args(["--format", "json", "--compact", "definition", "run"])
        .assert()
        .success();

    let out = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let results: Vec<Value> = serde_json::from_str(&out).expect("json");
    assert!(results.iter().any(|r| r["name"] == "run"));
    assert!(!results.iter().any(|r| r["name"] == "runner"));
}

#[test]
fn definition_falls_back_to_partial_when_exact_missing() {
    let dir = TempDir::new().expect("tempdir");
    write_file(&dir.path().join("runner.rs"), "pub fn runner() {}\n");

    let mut index_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    index_cmd
        .current_dir(dir.path())
        .args(["index"])
        .assert()
        .success();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = cmd
        .current_dir(dir.path())
        .args(["--format", "json", "--compact", "definition", "runn"])
        .assert()
        .success();

    let out = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let results: Vec<Value> = serde_json::from_str(&out).expect("json");
    assert!(results.iter().any(|r| r["name"] == "runner"));
}
