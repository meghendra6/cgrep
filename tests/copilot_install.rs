// SPDX-License-Identifier: MIT OR Apache-2.0

use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

#[test]
fn copilot_install_appends_section_even_if_file_mentions_cgrep() {
    let dir = TempDir::new().expect("tempdir");
    let github_dir = dir.path().join(".github");
    fs::create_dir_all(&github_dir).expect("mkdir .github");
    let copilot_instructions = github_dir.join("copilot-instructions.md");
    fs::write(
        &copilot_instructions,
        "Project note: cgrep is already mentioned here.\n",
    )
    .expect("write copilot-instructions");

    let mut install_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    install_cmd
        .current_dir(dir.path())
        .args(["agent", "install", "copilot"])
        .assert()
        .success();

    let content = fs::read_to_string(&copilot_instructions).expect("read copilot-instructions");
    assert!(content.contains("## cgrep Local Code Search"));
}

#[test]
fn copilot_install_does_not_duplicate_existing_cgrep_section() {
    let dir = TempDir::new().expect("tempdir");
    let github_dir = dir.path().join(".github");
    fs::create_dir_all(&github_dir).expect("mkdir .github");
    let copilot_instructions = github_dir.join("copilot-instructions.md");
    fs::write(
        &copilot_instructions,
        "# Copilot Instructions\n\n## cgrep Local Code Search\nexisting section\n",
    )
    .expect("write copilot-instructions");

    let mut install_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    install_cmd
        .current_dir(dir.path())
        .args(["agent", "install", "copilot"])
        .assert()
        .success();

    let content = fs::read_to_string(&copilot_instructions).expect("read copilot-instructions");
    let occurrences = content.matches("## cgrep Local Code Search").count();
    assert_eq!(occurrences, 1);
}
