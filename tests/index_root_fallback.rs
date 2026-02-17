// SPDX-License-Identifier: MIT OR Apache-2.0

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use tempfile::TempDir;

#[test]
fn search_falls_back_to_parent_index_when_nested_cgrep_has_only_cache() {
    let dir = TempDir::new().expect("tempdir");
    let root = dir.path();

    let src_dir = root.join("src").join("module");
    fs::create_dir_all(&src_dir).expect("create src/module");
    fs::write(src_dir.join("lib.rs"), "pub fn fallback_probe() {}\n").expect("write fixture");

    let mut index_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    index_cmd
        .current_dir(root)
        .args(["index", "--embeddings", "off"])
        .assert()
        .success();

    // Simulate a nested agent cache directory without a valid Tantivy index.
    fs::create_dir_all(src_dir.join(".cgrep").join("cache")).expect("create nested cache");

    let mut search_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let output = search_cmd
        .current_dir(root)
        .args([
            "--format",
            "json2",
            "--compact",
            "search",
            "fallback_probe",
            "-p",
            "src/module",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let raw = String::from_utf8(output).expect("utf8");
    let payload: Value = serde_json::from_str(&raw).expect("json2");
    assert!(payload["results"].is_array());
    assert!(payload["results"]
        .as_array()
        .map(|rows| !rows.is_empty())
        .unwrap_or(false));
}
