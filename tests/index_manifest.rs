// SPDX-License-Identifier: MIT OR Apache-2.0

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, content).expect("write file");
}

fn run_index(dir: &Path, args: &[&str]) -> String {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = cmd.current_dir(dir).args(args).assert().success();
    String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout")
}

fn run_search_json2_compact(dir: &Path, query: &str) -> String {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = cmd
        .current_dir(dir)
        .args(["--format", "json2", "--compact", "search", query])
        .assert()
        .success();
    String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout")
}

#[test]
fn manifest_only_writes_manifest_artifacts() {
    let dir = TempDir::new().expect("tempdir");
    write_file(
        &dir.path().join("src/lib.rs"),
        "pub fn manifest_marker() {}\n",
    );

    let _ = run_index(
        dir.path(),
        &[
            "index",
            "--manifest-only",
            "--print-diff",
            "--embeddings",
            "off",
        ],
    );

    assert!(dir.path().join(".cgrep/manifest/version").exists());
    assert!(dir.path().join(".cgrep/manifest/v1.json").exists());
    assert!(dir.path().join(".cgrep/manifest/root.hash").exists());
    assert!(dir.path().join(".cgrep/metadata.json").exists());
}

#[test]
fn print_diff_lists_paths_in_sorted_order() {
    let dir = TempDir::new().expect("tempdir");
    write_file(&dir.path().join("src/a.rs"), "pub fn a() {}\n");
    write_file(&dir.path().join("src/c.rs"), "pub fn c() {}\n");
    write_file(&dir.path().join("src/e.rs"), "pub fn e() {}\n");

    let _ = run_index(dir.path(), &["index", "--embeddings", "off"]);

    write_file(&dir.path().join("src/c.rs"), "pub fn c_changed() {}\n");
    fs::remove_file(dir.path().join("src/e.rs")).expect("remove e");
    write_file(&dir.path().join("src/b.rs"), "pub fn b() {}\n");
    write_file(&dir.path().join("src/d.rs"), "pub fn d() {}\n");

    let output = run_index(
        dir.path(),
        &[
            "index",
            "--manifest-only",
            "--print-diff",
            "--embeddings",
            "off",
        ],
    );
    let b_idx = output.find("    src/b.rs").expect("b in diff output");
    let d_idx = output.find("    src/d.rs").expect("d in diff output");
    assert!(b_idx < d_idx, "added paths should be sorted: {output}");
}

#[test]
fn json2_compact_output_is_stable_after_incremental_index() {
    let dir = TempDir::new().expect("tempdir");
    write_file(
        &dir.path().join("src/one.rs"),
        "pub fn alpha() { let needle = 1; }\n",
    );
    write_file(
        &dir.path().join("src/two.rs"),
        "pub fn beta() { let needle = 2; }\n",
    );

    let _ = run_index(dir.path(), &["index", "--embeddings", "off"]);
    write_file(
        &dir.path().join("src/one.rs"),
        "pub fn alpha() { let needle = 10; let needle_again = 11; }\n",
    );
    let _ = run_index(dir.path(), &["index", "--embeddings", "off"]);

    let first = run_search_json2_compact(dir.path(), "needle");
    let second = run_search_json2_compact(dir.path(), "needle");
    let first_json: Value = serde_json::from_str(&first).expect("first json2");
    let second_json: Value = serde_json::from_str(&second).expect("second json2");
    assert_eq!(first_json["results"], second_json["results"]);
}
