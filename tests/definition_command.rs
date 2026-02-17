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

#[test]
fn definition_limit_and_path_scope_work() {
    let dir = TempDir::new().expect("tempdir");
    write_file(
        &dir.path().join("core/foo.rs"),
        "pub fn foo() {}\npub fn foo_core_only() {}\n",
    );
    write_file(
        &dir.path().join("nested/foo.rs"),
        "pub fn foo() {}\npub fn foo_extra() {}\n",
    );
    write_file(&dir.path().join("nested/bar.rs"), "pub fn foo() {}\n");

    let mut index_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    index_cmd
        .current_dir(dir.path())
        .args(["index"])
        .assert()
        .success();

    let mut scoped_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let scoped_assert = scoped_cmd
        .current_dir(dir.path())
        .args([
            "--format",
            "json",
            "--compact",
            "definition",
            "foo",
            "-p",
            "core",
            "-m",
            "1",
        ])
        .assert()
        .success();
    let scoped_out = String::from_utf8(scoped_assert.get_output().stdout.clone()).expect("utf8");
    let scoped: Vec<Value> = serde_json::from_str(&scoped_out).expect("json");
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0]["path"], "foo.rs");

    let mut limited_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let limited_assert = limited_cmd
        .current_dir(dir.path())
        .args([
            "--format",
            "json",
            "--compact",
            "definition",
            "foo",
            "-m",
            "2",
        ])
        .assert()
        .success();
    let limited_out = String::from_utf8(limited_assert.get_output().stdout.clone()).expect("utf8");
    let limited: Vec<Value> = serde_json::from_str(&limited_out).expect("json");
    assert_eq!(limited.len(), 2);
}

#[test]
fn definition_skips_cpp_forward_declarations() {
    let dir = TempDir::new().expect("tempdir");
    write_file(
        &dir.path().join("forward.h"),
        "struct TensorIteratorBase;\n",
    );
    write_file(
        &dir.path().join("impl.h"),
        "struct TensorIteratorBase { int x; };\n",
    );

    let mut index_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    index_cmd
        .current_dir(dir.path())
        .args(["index"])
        .assert()
        .success();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = cmd
        .current_dir(dir.path())
        .args([
            "--format",
            "json",
            "--compact",
            "definition",
            "TensorIteratorBase",
        ])
        .assert()
        .success();
    let out = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let results: Vec<Value> = serde_json::from_str(&out).expect("json");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["path"], "impl.h");
}
