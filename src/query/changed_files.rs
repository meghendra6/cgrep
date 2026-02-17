// SPDX-License-Identifier: MIT OR Apache-2.0

//! Git changed-files filter helpers.

use anyhow::{bail, Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct ChangedFiles {
    rev: String,
    repo_root: PathBuf,
    scope_prefix: Option<String>,
    paths: HashSet<String>,
    signature: String,
}

impl ChangedFiles {
    pub fn from_scope(scope_root: &Path, rev: &str) -> Result<Self> {
        let scope_root = scope_root
            .canonicalize()
            .with_context(|| format!("Failed to resolve path: {}", scope_root.display()))?;
        let repo_root = git_repo_root(&scope_root)?;
        let scope_prefix = scope_root.strip_prefix(&repo_root).ok().and_then(|p| {
            let v = normalize_rel_path_str(&p.to_string_lossy());
            if v.is_empty() {
                None
            } else {
                Some(v)
            }
        });

        let paths = collect_changed_paths(&repo_root, rev, scope_prefix.as_deref())?;

        let signature = signature_for(rev, scope_prefix.as_deref(), &paths);

        Ok(Self {
            rev: rev.to_string(),
            repo_root,
            scope_prefix,
            paths,
            signature,
        })
    }

    pub fn rev(&self) -> &str {
        &self.rev
    }

    pub fn signature(&self) -> &str {
        &self.signature
    }

    pub fn matches_rel_path(&self, rel_path: &str) -> bool {
        if self.paths.is_empty() {
            return false;
        }
        let rel = normalize_rel_path_str(rel_path);
        if rel.is_empty() {
            return false;
        }

        let repo_rel = if let Some(prefix) = &self.scope_prefix {
            format!("{}/{}", prefix, rel)
        } else {
            rel
        };
        self.paths.contains(&repo_rel)
    }

    #[allow(dead_code)]
    pub fn matches_path(&self, path: &Path) -> bool {
        let rel = path
            .strip_prefix(&self.repo_root)
            .ok()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        let normalized = normalize_rel_path_str(&rel);
        self.paths.contains(&normalized)
    }
}

fn git_repo_root(path: &Path) -> Result<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("Failed to run git rev-parse")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "--changed requires a git repository (git rev-parse failed): {}",
            stderr.trim()
        );
    }

    let top = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if top.is_empty() {
        bail!("--changed requires a git repository");
    }

    Ok(PathBuf::from(top))
}

fn collect_changed_paths(
    repo_root: &Path,
    rev: &str,
    scope_prefix: Option<&str>,
) -> Result<HashSet<String>> {
    let mut diff_args = vec!["diff", "--name-only", rev, "--"];
    if let Some(prefix) = scope_prefix {
        diff_args.push(prefix);
    }
    let diff_output = run_git_collect_paths(
        repo_root,
        &diff_args,
        "Failed to run git diff for changed-files filter",
        "Failed to resolve changed files from git diff",
    )?;

    let mut untracked_args = vec!["ls-files", "--others", "--exclude-standard", "--"];
    if let Some(prefix) = scope_prefix {
        untracked_args.push(prefix);
    }
    let untracked_output = run_git_collect_paths(
        repo_root,
        &untracked_args,
        "Failed to run git ls-files for changed-files filter",
        "Failed to resolve untracked files from git ls-files",
    )?;

    let mut paths = HashSet::new();
    extend_paths_from_stdout(&mut paths, &diff_output.stdout);
    extend_paths_from_stdout(&mut paths, &untracked_output.stdout);
    Ok(paths)
}

fn run_git_collect_paths(
    repo_root: &Path,
    args: &[&str],
    context_message: &str,
    failure_prefix: &str,
) -> Result<std::process::Output> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()
        .with_context(|| context_message.to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("{}: {}", failure_prefix, stderr.trim());
    }
    Ok(output)
}

fn extend_paths_from_stdout(paths: &mut HashSet<String>, stdout: &[u8]) {
    for line in String::from_utf8_lossy(stdout).lines() {
        let normalized = normalize_rel_path_str(line);
        if !normalized.is_empty() {
            paths.insert(normalized);
        }
    }
}

fn signature_for(rev: &str, scope_prefix: Option<&str>, paths: &HashSet<String>) -> String {
    let mut sorted_paths: Vec<&String> = paths.iter().collect();
    sorted_paths.sort();
    let payload = format!(
        "{}|{}|{}",
        rev,
        scope_prefix.unwrap_or(""),
        sorted_paths
            .iter()
            .map(|p| p.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
    blake3::hash(payload.as_bytes()).to_hex()[..16].to_string()
}

fn normalize_rel_path_str(input: &str) -> String {
    let path = input.replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();

    for part in path.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            if !parts.is_empty() {
                parts.pop();
            }
            continue;
        }
        parts.push(part);
    }

    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn run(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .status()
            .expect("run git");
        assert!(status.success(), "git {:?} failed", args);
    }

    #[test]
    fn changed_files_filters_scope_relative_paths() {
        let dir = TempDir::new().expect("tempdir");
        run(dir.path(), &["init"]);
        run(dir.path(), &["config", "user.email", "test@example.com"]);
        run(dir.path(), &["config", "user.name", "test"]);

        let src = dir.path().join("src");
        let nested = src.join("nested");
        std::fs::create_dir_all(&nested).expect("mkdir");
        std::fs::write(src.join("lib.rs"), "pub fn alpha() {}\n").expect("write lib");
        std::fs::write(nested.join("util.rs"), "pub fn beta() {}\n").expect("write util");

        run(dir.path(), &["add", "."]);
        run(dir.path(), &["commit", "-m", "initial"]);

        std::fs::write(nested.join("util.rs"), "pub fn beta() { let _ = 1; }\n")
            .expect("rewrite util");

        let changed = ChangedFiles::from_scope(&src, "HEAD").expect("changed");
        assert!(changed.matches_rel_path("nested/util.rs"));
        assert!(!changed.matches_rel_path("lib.rs"));
    }

    #[test]
    fn changed_files_include_untracked_paths() {
        let dir = TempDir::new().expect("tempdir");
        run(dir.path(), &["init"]);
        run(dir.path(), &["config", "user.email", "test@example.com"]);
        run(dir.path(), &["config", "user.name", "test"]);

        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).expect("mkdir");
        std::fs::write(src.join("tracked.rs"), "pub fn tracked() {}\n").expect("write tracked");
        run(dir.path(), &["add", "."]);
        run(dir.path(), &["commit", "-m", "initial"]);

        std::fs::write(src.join("new_file.rs"), "pub fn newly_added() {}\n").expect("write new");

        let changed = ChangedFiles::from_scope(&src, "HEAD").expect("changed");
        assert!(changed.matches_rel_path("new_file.rs"));
        assert!(!changed.matches_rel_path("tracked.rs"));
    }

    #[test]
    fn normalize_rel_path_handles_windows_and_dots() {
        assert_eq!(normalize_rel_path_str(".\\src\\lib.rs"), "src/lib.rs");
        assert_eq!(
            normalize_rel_path_str("./src/./nested/../lib.rs"),
            "src/lib.rs"
        );
    }

    #[test]
    fn changed_files_signature_ignores_out_of_scope_changes() {
        let dir = TempDir::new().expect("tempdir");
        run(dir.path(), &["init"]);
        run(dir.path(), &["config", "user.email", "test@example.com"]);
        run(dir.path(), &["config", "user.name", "test"]);

        let src = dir.path().join("src");
        let docs = dir.path().join("docs");
        std::fs::create_dir_all(&src).expect("mkdir src");
        std::fs::create_dir_all(&docs).expect("mkdir docs");
        std::fs::write(src.join("lib.rs"), "pub fn scoped() {}\n").expect("write scoped");
        std::fs::write(docs.join("guide.md"), "v1\n").expect("write docs");

        run(dir.path(), &["add", "."]);
        run(dir.path(), &["commit", "-m", "initial"]);

        let base = ChangedFiles::from_scope(&src, "HEAD").expect("base changed");

        // Change outside scope only.
        std::fs::write(docs.join("guide.md"), "v2\n").expect("rewrite docs");
        let outside_only = ChangedFiles::from_scope(&src, "HEAD").expect("outside changed");
        assert_eq!(outside_only.signature(), base.signature());
        assert!(!outside_only.matches_rel_path("lib.rs"));

        // Change inside scope.
        std::fs::write(src.join("lib.rs"), "pub fn scoped() { let _ = 1; }\n")
            .expect("rewrite src");
        let scoped = ChangedFiles::from_scope(&src, "HEAD").expect("scoped changed");
        assert_ne!(scoped.signature(), outside_only.signature());
        assert!(scoped.matches_rel_path("lib.rs"));
    }
}
