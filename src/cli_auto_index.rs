// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::Result;
use ignore::WalkBuilder;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

use crate::indexer;

const CLI_AUTO_INDEX_CHECK_COOLDOWN_MS: u64 = 2_000;

pub fn maybe_prepare_cli_auto_index(path: Option<&str>) {
    if std::env::var("CGREP_DISABLE_CLI_AUTO_INDEX")
        .ok()
        .as_deref()
        .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    {
        return;
    }

    let Ok(search_scope) = resolve_cli_scope(path) else {
        return;
    };
    let existing_index_root = cgrep::utils::find_index_root(&search_scope);
    let index_scope = existing_index_root
        .as_ref()
        .map(|root| root.root.clone())
        .unwrap_or_else(|| search_scope.clone());

    if existing_index_root.is_some() && cli_auto_index_check_is_fresh(&index_scope) {
        return;
    }

    let should_index = if existing_index_root.is_some() {
        cli_scope_has_indexable_changes_since(&search_scope, &index_scope).unwrap_or(true)
    } else {
        true
    };
    if existing_index_root.is_some() {
        let _ = touch_cli_auto_index_check(&index_scope);
    }
    if !should_index {
        return;
    }

    if run_cli_index_for_scope(&index_scope).is_ok() {
        let _ = touch_cli_auto_index_check(&index_scope);
    }
}

pub fn background_index_active_for_scope(path: Option<&str>) -> bool {
    let Ok(scope) = resolve_cli_scope(path) else {
        return false;
    };
    nearest_background_build_phase(&scope)
        .map(|phase| matches!(phase.as_str(), "starting" | "indexing" | "embedding"))
        .unwrap_or(false)
}

pub fn touch_cli_auto_index_check_for_scope(path: Option<&str>) {
    let Ok(scope) = resolve_cli_scope(path) else {
        return;
    };
    let index_scope = cgrep::utils::find_index_root(&scope)
        .map(|root| root.root)
        .unwrap_or(scope);
    let _ = touch_cli_auto_index_check(&index_scope);
}

fn nearest_background_build_phase(scope: &Path) -> Option<String> {
    let mut current = Some(scope);
    while let Some(dir) = current {
        let status_path = dir.join(cgrep::utils::INDEX_DIR).join("status.json");
        if status_path.is_file() {
            let raw = std::fs::read_to_string(status_path).ok()?;
            let status = serde_json::from_str::<indexer::status::BuildStatus>(&raw).ok()?;
            return Some(status.phase);
        }
        current = dir.parent();
    }
    None
}

fn resolve_cli_scope(path: Option<&str>) -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let requested = path.map(PathBuf::from).unwrap_or_else(|| cwd.clone());
    let raw_scope = if requested.is_absolute() {
        requested
    } else {
        cwd.join(requested)
    };

    let mut scope = raw_scope.canonicalize().unwrap_or(raw_scope);
    if scope.is_file() {
        if let Some(parent) = scope.parent() {
            scope = parent.to_path_buf();
        }
    }
    Ok(scope)
}

fn cli_scope_has_indexable_changes_since(search_scope: &Path, index_scope: &Path) -> Result<bool> {
    let metadata_path = index_scope
        .join(cgrep::utils::INDEX_DIR)
        .join("metadata.json");
    let index_modified = std::fs::metadata(&metadata_path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let mut builder = WalkBuilder::new(search_scope);
    builder
        .hidden(false)
        .ignore(true)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true);
    let walker = builder
        .filter_entry(|entry| {
            entry
                .file_name()
                .to_str()
                .map(|name| !matches!(name, ".cgrep" | ".git" | ".hg" | ".svn"))
                .unwrap_or(true)
        })
        .build();

    for entry in walker {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if !path.is_file() || !should_track_cli_auto_index_path(search_scope, path) {
            continue;
        }

        let modified = match entry.metadata() {
            Ok(metadata) => match metadata.modified() {
                Ok(modified) => modified,
                Err(_) => return Ok(true),
            },
            Err(_) => return Ok(true),
        };
        if modified > index_modified {
            return Ok(true);
        }
    }

    Ok(false)
}

fn should_track_cli_auto_index_path(scope_root: &Path, path: &Path) -> bool {
    let relative = path.strip_prefix(scope_root).unwrap_or(path);
    if relative.as_os_str().is_empty() {
        return false;
    }

    for component in relative.components() {
        if let Component::Normal(name) = component {
            let Some(name) = name.to_str() else { continue };
            if matches!(name, ".cgrep" | ".git" | ".hg" | ".svn") {
                return false;
            }
        }
    }

    let file_name = relative.file_name().and_then(|f| f.to_str()).unwrap_or("");
    if file_name.starts_with('.')
        || file_name.starts_with(".#")
        || file_name.ends_with('~')
        || file_name.ends_with(".tmp")
        || file_name.ends_with(".swp")
        || file_name.ends_with(".swo")
    {
        return false;
    }

    let Some(ext) = relative.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    indexer::scanner::is_indexable_extension(ext)
}

fn run_cli_index_for_scope(scope: &Path) -> Result<()> {
    let exe = std::env::current_exe()?;
    let scope_arg = scope.display().to_string();
    let status = Command::new(exe)
        .args(["index", "-p", scope_arg.as_str(), "--embeddings", "off"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("auto-index command failed with status {}", status)
    }
}

fn cli_auto_index_check_is_fresh(index_scope: &Path) -> bool {
    let stamp_path = cli_auto_index_stamp_path(index_scope);
    let Ok(metadata) = std::fs::metadata(stamp_path) else {
        return false;
    };
    let Ok(modified) = metadata.modified() else {
        return false;
    };
    let Ok(elapsed) = modified.elapsed() else {
        return false;
    };
    elapsed.as_millis() < u128::from(CLI_AUTO_INDEX_CHECK_COOLDOWN_MS)
}

fn touch_cli_auto_index_check(index_scope: &Path) -> Result<()> {
    let stamp_path = cli_auto_index_stamp_path(index_scope);
    if let Some(parent) = stamp_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(stamp_path, b"1")?;
    Ok(())
}

fn cli_auto_index_stamp_path(index_scope: &Path) -> PathBuf {
    index_scope
        .join(cgrep::utils::INDEX_DIR)
        .join("auto_index_check.stamp")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn track_path_rejects_hidden_and_temp_files() {
        let root = Path::new("/workspace");

        assert!(!should_track_cli_auto_index_path(
            root,
            Path::new("/workspace/.git/config")
        ));
        assert!(!should_track_cli_auto_index_path(
            root,
            Path::new("/workspace/src/.hidden.rs")
        ));
        assert!(!should_track_cli_auto_index_path(
            root,
            Path::new("/workspace/src/file.tmp")
        ));
        assert!(!should_track_cli_auto_index_path(
            root,
            Path::new("/workspace/src/file")
        ));
    }

    #[test]
    fn track_path_accepts_indexable_extension() {
        let root = Path::new("/workspace");
        assert!(should_track_cli_auto_index_path(
            root,
            Path::new("/workspace/src/lib.rs")
        ));
    }

    #[test]
    fn resolve_scope_uses_parent_for_files() {
        let tmp = tempdir().expect("tempdir");
        let dir = tmp.path().join("src");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let file = dir.join("lib.rs");
        std::fs::write(&file, "fn main() {}\n").expect("write");

        let scope = resolve_cli_scope(file.to_str()).expect("resolve");
        assert_eq!(
            scope.canonicalize().expect("scope canonical"),
            dir.canonicalize().expect("dir canonical")
        );
    }
}
