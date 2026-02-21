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

fn run_success(root: &Path, args: &[String]) -> String {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = cmd.current_dir(root).args(args).assert().success();
    String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8")
}

fn run_json2(root: &Path, args: &[&str]) -> Value {
    let mut all = vec![
        "--format".to_string(),
        "json2".to_string(),
        "--compact".to_string(),
    ];
    all.extend(args.iter().map(|v| v.to_string()));
    let stdout = run_success(root, &all);
    serde_json::from_str(&stdout).expect("json parse")
}

fn run_json2_raw(root: &Path, args: &[&str]) -> String {
    let mut all = vec![
        "--format".to_string(),
        "json2".to_string(),
        "--compact".to_string(),
    ];
    all.extend(args.iter().map(|v| v.to_string()));
    run_success(root, &all)
}

fn run_index(root: &Path) {
    let args = vec![
        "index".to_string(),
        "--embeddings".to_string(),
        "off".to_string(),
    ];
    let _ = run_success(root, &args);
}

fn write_fixture(root: &Path) {
    write_file(
        &root.join("src/auth.rs"),
        "pub fn validate_token(input: &str) -> bool {\n    input.starts_with(\"tok_\")\n}\n",
    );
    write_file(
        &root.join("src/service.rs"),
        "pub fn auth_service_flow() {\n    let _ok = validate_token(\"tok_sample\");\n}\n",
    );
    write_file(
        &root.join("src/callers.rs"),
        "pub fn invoke_auth() {\n    if validate_token(\"tok_x\") {\n        println!(\"ok\");\n    }\n}\n",
    );
    write_file(
        &root.join("docs/guide.md"),
        "authentication middleware flow and retry policy\n",
    );
}

fn map_step(payload: &Value) -> &Value {
    payload["steps"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|step| step.get("command").and_then(Value::as_str) == Some("map"))
        .expect("map step")
}

#[test]
fn agent_plan_json2_compact_is_byte_stable() {
    let dir = TempDir::new().expect("tempdir");
    write_fixture(dir.path());
    run_index(dir.path());

    let first = run_json2_raw(
        dir.path(),
        &["agent", "plan", "validate_token", "--max-candidates", "5"],
    );
    let second = run_json2_raw(
        dir.path(),
        &["agent", "plan", "validate_token", "--max-candidates", "5"],
    );
    assert_eq!(first, second);
}

#[test]
fn agent_plan_emits_expected_step_sequence_by_query_shape() {
    let dir = TempDir::new().expect("tempdir");
    write_fixture(dir.path());
    run_index(dir.path());

    let phrase = run_json2(
        dir.path(),
        &[
            "agent",
            "plan",
            "trace authentication middleware flow",
            "--max-steps",
            "6",
            "--max-candidates",
            "4",
        ],
    );
    let phrase_steps = phrase["steps"].as_array().expect("steps");
    assert!(!phrase_steps.is_empty());
    let phrase_commands = phrase_steps
        .iter()
        .filter_map(|s| s.get("command").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(phrase_commands.contains(&"map"));
    assert!(phrase_commands.contains(&"agent locate"));
    assert!(phrase_commands.contains(&"agent expand"));
    assert!(!phrase_commands.contains(&"definition"));
    assert!(!phrase_commands.contains(&"references"));
    assert!(!phrase_commands.contains(&"callers"));

    let identifier = run_json2(
        dir.path(),
        &[
            "agent",
            "plan",
            "validate_token",
            "--max-steps",
            "6",
            "--max-candidates",
            "4",
        ],
    );
    let id_steps = identifier["steps"].as_array().expect("steps");
    let id_commands = id_steps
        .iter()
        .filter_map(|s| s.get("command").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(id_commands.contains(&"agent locate"));
    assert!(id_commands.contains(&"agent expand"));
    assert!(id_commands.contains(&"definition"));
    assert!(id_commands.contains(&"references"));
    assert!(id_commands.contains(&"callers"));
}

#[test]
fn unscoped_phrase_plan_keeps_map_step_planned() {
    let dir = TempDir::new().expect("tempdir");
    write_fixture(dir.path());
    run_index(dir.path());

    let payload = run_json2(
        dir.path(),
        &[
            "agent",
            "plan",
            "trace authentication middleware flow",
            "--max-steps",
            "6",
            "--max-candidates",
            "4",
        ],
    );
    let map = map_step(&payload);
    assert_eq!(map["status"], "planned");
    assert!(map.get("result_count").is_none());
    let reason = map["reason"].as_str().expect("map reason");
    assert!(reason.contains("skipped execution for unscoped planning"));
}

#[test]
fn scoped_plan_executes_map_step() {
    let dir = TempDir::new().expect("tempdir");
    write_fixture(dir.path());
    run_index(dir.path());

    let payload = run_json2(
        dir.path(),
        &[
            "agent",
            "plan",
            "trace authentication middleware flow",
            "--path",
            "src",
            "--max-steps",
            "6",
            "--max-candidates",
            "4",
        ],
    );
    let map = map_step(&payload);
    assert_eq!(map["status"], "executed");
    assert!(map.get("result_count").and_then(Value::as_u64).is_some());
}

#[test]
fn scoped_plan_emits_reusable_read_followups() {
    let dir = TempDir::new().expect("tempdir");
    write_fixture(dir.path());
    run_index(dir.path());

    let payload = run_json2(
        dir.path(),
        &[
            "agent",
            "plan",
            "trace authentication middleware flow",
            "--path",
            "src",
            "--max-steps",
            "6",
            "--max-candidates",
            "4",
        ],
    );
    let read_steps = payload["steps"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|step| step.get("command").and_then(Value::as_str) == Some("read"))
        .collect::<Vec<_>>();
    assert!(!read_steps.is_empty(), "expected read follow-up steps");

    for step in read_steps {
        let args = step["args"]
            .as_array()
            .expect("read args")
            .iter()
            .filter_map(Value::as_str)
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let read_path = Path::new(args.first().expect("read path arg"));
        assert!(
            read_path.starts_with("src"),
            "expected path rebased to repo root: {}",
            read_path.display()
        );

        let mut read_cmd_args = vec!["read".to_string()];
        read_cmd_args.extend(args);
        let _ = run_success(dir.path(), &read_cmd_args);
    }
}

#[test]
fn agent_plan_payload_is_bounded_against_locate_expand_baseline() {
    let dir = TempDir::new().expect("tempdir");
    write_fixture(dir.path());
    for i in 0..80 {
        write_file(
            &dir.path().join(format!("src/extra_{i}.rs")),
            &format!("pub fn payload_probe_{i}() {{ let _ = \"plan_payload_probe\"; }}\n"),
        );
    }
    run_index(dir.path());

    let locate_raw = run_json2_raw(
        dir.path(),
        &[
            "agent",
            "locate",
            "plan_payload_probe",
            "--limit",
            "30",
            "--budget",
            "balanced",
        ],
    );
    let locate_json: Value = serde_json::from_str(&locate_raw).expect("locate json");
    let ids = locate_json["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| row.get("id").and_then(Value::as_str).map(|s| s.to_string()))
        .take(5)
        .collect::<Vec<_>>();
    assert!(!ids.is_empty());

    let mut expand_args = vec![
        "--format".to_string(),
        "json2".to_string(),
        "--compact".to_string(),
        "agent".to_string(),
        "expand".to_string(),
    ];
    for id in &ids {
        expand_args.push("--id".to_string());
        expand_args.push(id.clone());
    }
    expand_args.push("--context".to_string());
    expand_args.push("8".to_string());
    let expand_raw = run_success(dir.path(), &expand_args);
    let baseline_len = locate_raw.len() + expand_raw.len();

    let plan_raw = run_json2_raw(
        dir.path(),
        &[
            "agent",
            "plan",
            "plan_payload_probe",
            "--max-candidates",
            "5",
            "--max-steps",
            "6",
        ],
    );
    assert!(
        plan_raw.len() <= baseline_len + 2_000,
        "plan payload exceeded bound: plan={}, baseline={}",
        plan_raw.len(),
        baseline_len
    );
}

#[test]
fn locate_and_expand_remain_compatible_after_plan_execution() {
    let dir = TempDir::new().expect("tempdir");
    write_fixture(dir.path());
    run_index(dir.path());

    let locate_before = run_json2(dir.path(), &["agent", "locate", "validate_token"]);
    let _plan = run_json2(dir.path(), &["agent", "plan", "validate_token"]);
    let locate_after = run_json2(dir.path(), &["agent", "locate", "validate_token"]);

    assert_eq!(locate_before["results"], locate_after["results"]);

    let first_id = locate_before["results"]
        .as_array()
        .into_iter()
        .flatten()
        .find_map(|row| row.get("id").and_then(Value::as_str))
        .expect("locate result id")
        .to_string();
    let expand = run_json2(dir.path(), &["agent", "expand", "--id", &first_id]);
    assert_eq!(expand["meta"]["stage"], "expand");
    assert!(expand["meta"]["resolved_ids"].as_u64().unwrap_or(0) >= 1);
}

#[test]
fn invalid_plan_limits_return_deterministic_parseable_error_payload() {
    let dir = TempDir::new().expect("tempdir");
    write_fixture(dir.path());
    run_index(dir.path());

    let first = run_json2_raw(
        dir.path(),
        &["agent", "plan", "validate_token", "--max-steps", "0"],
    );
    let second = run_json2_raw(
        dir.path(),
        &["agent", "plan", "validate_token", "--max-steps", "0"],
    );
    assert_eq!(first, second);

    let payload: Value = serde_json::from_str(&first).expect("error json");
    assert_eq!(payload["meta"]["stage"], "plan");
    assert_eq!(payload["error"]["code"], "invalid_option");
    assert_eq!(payload["error"]["field"], "max_steps");
}

#[test]
fn step_and_candidate_ids_are_stable_across_runs() {
    let dir = TempDir::new().expect("tempdir");
    write_fixture(dir.path());
    run_index(dir.path());

    let first = run_json2(
        dir.path(),
        &["agent", "plan", "validate_token", "--max-candidates", "5"],
    );
    let second = run_json2(
        dir.path(),
        &["agent", "plan", "validate_token", "--max-candidates", "5"],
    );

    let first_step_ids = first["steps"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| row.get("id").and_then(Value::as_str).map(|s| s.to_string()))
        .collect::<Vec<_>>();
    let second_step_ids = second["steps"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| row.get("id").and_then(Value::as_str).map(|s| s.to_string()))
        .collect::<Vec<_>>();
    assert_eq!(first_step_ids, second_step_ids);

    let first_candidate_ids = first["candidates"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| row.get("id").and_then(Value::as_str).map(|s| s.to_string()))
        .collect::<Vec<_>>();
    let second_candidate_ids = second["candidates"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| row.get("id").and_then(Value::as_str).map(|s| s.to_string()))
        .collect::<Vec<_>>();
    assert_eq!(first_candidate_ids, second_candidate_ids);
}
