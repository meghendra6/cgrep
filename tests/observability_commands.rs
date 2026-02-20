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

fn run_cgrep(dir: &Path, args: &[&str]) -> String {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = cmd.current_dir(dir).args(args).assert().success();
    String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout")
}

fn run_search_json2_compact(dir: &Path, query: &str) -> Value {
    let raw = run_cgrep(
        dir,
        &[
            "--format",
            "json2",
            "--compact",
            "search",
            query,
            "--limit",
            "20",
        ],
    );
    serde_json::from_str(&raw).expect("search json2")
}

#[test]
fn status_json2_compact_is_byte_stable() {
    let dir = TempDir::new().expect("tempdir");
    write_file(
        &dir.path().join("src/lib.rs"),
        "pub fn marker() { let needle = 1; }\n",
    );

    let _ = run_cgrep(dir.path(), &["index", "--embeddings", "off"]);

    let first = run_cgrep(dir.path(), &["--format", "json2", "--compact", "status"]);
    let second = run_cgrep(dir.path(), &["--format", "json2", "--compact", "status"]);

    assert_eq!(first, second, "status json2 compact should be byte-stable");
}

#[test]
fn stats_persists_and_exposes_required_fields() {
    let dir = TempDir::new().expect("tempdir");
    write_file(
        &dir.path().join("src/lib.rs"),
        "pub fn marker() { let needle = 1; }\n",
    );

    let _ = run_cgrep(dir.path(), &["index", "--embeddings", "off"]);

    let raw = run_cgrep(dir.path(), &["--format", "json2", "--compact", "stats"]);
    let payload: Value = serde_json::from_str(&raw).expect("stats json2");

    assert_eq!(payload["meta"]["command"], "stats");

    let last_run = payload["result"]["last_run"]
        .as_object()
        .expect("last_run object");
    for key in [
        "mode",
        "force",
        "started_at_ms",
        "finished_at_ms",
        "total_ms",
        "timings_ms",
        "diff",
        "cache_reuse",
        "indexed_files",
        "skipped_files",
        "deleted_files",
        "error_files",
    ] {
        assert!(last_run.contains_key(key), "missing last_run field: {key}");
    }

    let timings = payload["result"]["last_run"]["timings_ms"]
        .as_object()
        .expect("timings object");
    for key in ["scan_ms", "hash_ms", "parse_ms", "index_ms", "commit_ms"] {
        assert!(timings.contains_key(key), "missing timing field: {key}");
    }

    assert!(dir.path().join(".cgrep/metadata.json").exists());
}

#[test]
fn doctor_reports_broken_state_without_mutation() {
    let dir = TempDir::new().expect("tempdir");
    let cgrep_dir = dir.path().join(".cgrep");
    fs::create_dir_all(&cgrep_dir).expect("create .cgrep");
    fs::write(cgrep_dir.join("metadata.json"), "{this-is-not-json").expect("write broken metadata");

    let raw = run_cgrep(dir.path(), &["--format", "json2", "--compact", "doctor"]);
    let payload: Value = serde_json::from_str(&raw).expect("doctor json2");

    assert_eq!(payload["meta"]["command"], "doctor");
    assert_eq!(payload["result"]["healthy"], false);

    let findings = payload["result"]["findings"]
        .as_array()
        .expect("findings array");
    let ids: Vec<String> = findings
        .iter()
        .filter_map(|finding| finding["id"].as_str().map(|s| s.to_string()))
        .collect();

    assert!(
        ids.iter().any(|id| id == "missing_tantivy_meta"),
        "doctor should report missing tantivy meta: {ids:?}"
    );
    assert!(
        ids.iter().any(|id| id == "metadata_parse_error"),
        "doctor should report metadata parse error: {ids:?}"
    );

    // Doctor is read-only: broken metadata should remain unchanged.
    let after =
        fs::read_to_string(cgrep_dir.join("metadata.json")).expect("read metadata after doctor");
    assert_eq!(after, "{this-is-not-json");
}

#[test]
fn existing_search_results_unchanged_after_observability_commands() {
    let dir = TempDir::new().expect("tempdir");
    write_file(
        &dir.path().join("src/lib.rs"),
        "pub fn marker() { let needle = 1; let keep = needle + 1; }\n",
    );

    let _ = run_cgrep(dir.path(), &["index", "--embeddings", "off"]);
    let before = run_search_json2_compact(dir.path(), "needle");

    let _ = run_cgrep(dir.path(), &["status"]);
    let _ = run_cgrep(dir.path(), &["stats"]);
    let _ = run_cgrep(dir.path(), &["doctor"]);

    let after = run_search_json2_compact(dir.path(), "needle");

    assert_eq!(before["meta"]["query"], "needle");
    assert_eq!(after["meta"]["query"], "needle");
    assert_eq!(before["results"], after["results"]);
}

#[test]
fn stats_writes_leave_no_temp_files_after_multiple_updates() {
    let dir = TempDir::new().expect("tempdir");
    let file = dir.path().join("src/lib.rs");
    write_file(&file, "pub fn marker() { let needle = 1; }\n");

    let _ = run_cgrep(dir.path(), &["index", "--embeddings", "off"]);
    write_file(&file, "pub fn marker() { let needle = 2; }\n");
    let _ = run_cgrep(dir.path(), &["index", "--embeddings", "off"]);

    let metadata_path = dir.path().join(".cgrep/metadata.json");
    assert!(metadata_path.exists());
    let metadata_raw = fs::read_to_string(&metadata_path).expect("read metadata");
    let parsed: Value = serde_json::from_str(&metadata_raw).expect("parse metadata");
    assert!(parsed["last_run_stats"].is_object());

    for entry in fs::read_dir(dir.path().join(".cgrep")).expect("read .cgrep") {
        let entry = entry.expect("entry");
        let name = entry.file_name();
        let name = name.to_string_lossy();
        assert!(
            !name.contains(".tmp-"),
            "unexpected temporary artifact left behind: {name}"
        );
    }
}
