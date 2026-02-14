// SPDX-License-Identifier: MIT OR Apache-2.0

//! Agent-oriented query helpers.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::indexer::scanner::FileScanner;
use cgrep::output::print_json;

const AGENT_HINT_CACHE_REL: &str = ".cgrep/cache/agent_expand_hints.json";
const AGENT_HINT_CACHE_VERSION: u32 = 1;
const AGENT_HINT_TTL_SECS: u64 = 60 * 60 * 24 * 7; // 7 days
const AGENT_HINT_MAX_ENTRIES: usize = 10_000;

#[derive(Debug, Serialize)]
struct AgentExpandMeta {
    schema_version: &'static str,
    stage: &'static str,
    requested_ids: usize,
    resolved_ids: usize,
    hint_resolved_ids: usize,
    scan_resolved_ids: usize,
    context: usize,
    search_root: String,
}

#[derive(Debug, Serialize)]
struct AgentExpandResult {
    id: String,
    path: String,
    line: usize,
    start_line: usize,
    end_line: usize,
    snippet: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    context_before: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    context_after: Vec<String>,
}

#[derive(Debug, Serialize)]
struct AgentExpandPayload {
    meta: AgentExpandMeta,
    results: Vec<AgentExpandResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentHintEntry {
    id: String,
    path: String,
    line: usize,
    snippet: String,
    updated_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct AgentHintCacheFile {
    version: u32,
    entries: Vec<AgentHintEntry>,
}

/// Expand stable result IDs into richer context windows for agent workflows.
pub fn run_expand(ids: &[String], path: Option<&str>, context: usize, compact: bool) -> Result<()> {
    let search_root = resolve_search_root(path)?;
    let wanted: HashSet<String> = ids.iter().cloned().collect();
    let mut results: Vec<AgentExpandResult> = Vec::new();
    let mut resolved: HashSet<String> = HashSet::new();
    let mut hint_resolved_ids = 0usize;
    let mut scan_resolved_ids = 0usize;

    let hint_map = load_hint_map(&search_root).unwrap_or_default();
    let mut line_cache: HashMap<String, Vec<String>> = HashMap::new();
    for id in ids {
        if let Some(hint) = hint_map.get(id) {
            if let Some(result) = resolve_from_hint(&search_root, hint, context, &mut line_cache) {
                results.push(result);
                resolved.insert(id.clone());
                hint_resolved_ids += 1;
            }
        }
    }

    let unresolved: HashSet<String> = wanted.difference(&resolved).cloned().collect();
    if !unresolved.is_empty() {
        let scanner = FileScanner::new(&search_root);
        let files = scanner.scan()?;
        for file in files {
            let rel_path = file
                .path
                .strip_prefix(&search_root)
                .unwrap_or(&file.path)
                .display()
                .to_string();

            let lines: Vec<&str> = file.content.lines().collect();
            for (idx, line) in lines.iter().enumerate() {
                let line_num = idx + 1;
                let snippet = line_to_snippet(line);
                let id = stable_result_id(&rel_path, line_num, &snippet);
                if !unresolved.contains(&id) {
                    continue;
                }

                let (context_before, context_after) = context_from_lines(&lines, line_num, context);
                let start_line = line_num.saturating_sub(context_before.len());
                let end_line = line_num + context_after.len();

                results.push(AgentExpandResult {
                    id: id.clone(),
                    path: rel_path.clone(),
                    line: line_num,
                    start_line,
                    end_line,
                    snippet,
                    context_before,
                    context_after,
                });
                resolved.insert(id);
                scan_resolved_ids += 1;
            }
        }
    }

    results.sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));

    let payload = AgentExpandPayload {
        meta: AgentExpandMeta {
            schema_version: "1",
            stage: "expand",
            requested_ids: wanted.len(),
            resolved_ids: results.len(),
            hint_resolved_ids,
            scan_resolved_ids,
            context,
            search_root: search_root.display().to_string(),
        },
        results,
    };
    print_json(&payload, compact)?;

    Ok(())
}

pub(crate) fn persist_expand_hints(
    search_root: &Path,
    hints: impl IntoIterator<Item = AgentHintInput>,
) -> Result<()> {
    let path = hint_cache_path(search_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }

    let now = current_unix_secs();
    let mut cache = load_hint_cache(&path).unwrap_or_default();
    if cache.version != AGENT_HINT_CACHE_VERSION {
        cache = AgentHintCacheFile {
            version: AGENT_HINT_CACHE_VERSION,
            entries: Vec::new(),
        };
    }

    let mut by_id: HashMap<String, AgentHintEntry> = HashMap::with_capacity(cache.entries.len());
    for entry in cache.entries {
        if now.saturating_sub(entry.updated_at) > AGENT_HINT_TTL_SECS {
            continue;
        }
        by_id.insert(entry.id.clone(), entry);
    }

    for hint in hints {
        if hint.line == 0 || hint.path.is_empty() || hint.snippet.is_empty() {
            continue;
        }
        let id = hint
            .id
            .unwrap_or_else(|| stable_result_id(&hint.path, hint.line, &hint.snippet));
        let entry = AgentHintEntry {
            id: id.clone(),
            path: hint.path,
            line: hint.line,
            snippet: hint.snippet,
            updated_at: now,
        };
        by_id.insert(id, entry);
    }

    let mut entries: Vec<AgentHintEntry> = by_id.into_values().collect();
    entries.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    if entries.len() > AGENT_HINT_MAX_ENTRIES {
        entries.truncate(AGENT_HINT_MAX_ENTRIES);
    }

    let data = AgentHintCacheFile {
        version: AGENT_HINT_CACHE_VERSION,
        entries,
    };
    let content = serde_json::to_string_pretty(&data).context("Failed to encode hint cache")?;
    fs::write(&path, content).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct AgentHintInput {
    pub id: Option<String>,
    pub path: String,
    pub line: usize,
    pub snippet: String,
}

fn load_hint_map(search_root: &Path) -> Result<HashMap<String, AgentHintEntry>> {
    let path = hint_cache_path(search_root);
    let cache = load_hint_cache(&path)?;
    let now = current_unix_secs();
    let mut map = HashMap::with_capacity(cache.entries.len());
    for entry in cache.entries {
        if now.saturating_sub(entry.updated_at) > AGENT_HINT_TTL_SECS {
            continue;
        }
        map.insert(entry.id.clone(), entry);
    }
    Ok(map)
}

fn load_hint_cache(path: &Path) -> Result<AgentHintCacheFile> {
    if !path.exists() {
        return Ok(AgentHintCacheFile {
            version: AGENT_HINT_CACHE_VERSION,
            entries: Vec::new(),
        });
    }
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let parsed = serde_json::from_str::<AgentHintCacheFile>(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(parsed)
}

fn hint_cache_path(search_root: &Path) -> PathBuf {
    search_root.join(AGENT_HINT_CACHE_REL)
}

fn resolve_from_hint(
    search_root: &Path,
    hint: &AgentHintEntry,
    context: usize,
    line_cache: &mut HashMap<String, Vec<String>>,
) -> Option<AgentExpandResult> {
    if hint.line == 0 {
        return None;
    }
    let full_path = search_root.join(&hint.path);
    if !full_path.exists() {
        return None;
    }

    if !line_cache.contains_key(&hint.path) {
        let content = fs::read_to_string(&full_path).ok()?;
        let lines = content
            .lines()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();
        line_cache.insert(hint.path.clone(), lines);
    }
    let lines = line_cache.get(&hint.path)?;
    if hint.line > lines.len() {
        return None;
    }

    let snippet = line_to_snippet(lines[hint.line - 1].as_str());
    let actual_id = stable_result_id(&hint.path, hint.line, &snippet);
    if actual_id != hint.id {
        return None;
    }

    let (context_before, context_after) = context_from_string_lines(lines, hint.line, context);
    let start_line = hint.line.saturating_sub(context_before.len());
    let end_line = hint.line + context_after.len();

    Some(AgentExpandResult {
        id: hint.id.clone(),
        path: hint.path.clone(),
        line: hint.line,
        start_line,
        end_line,
        snippet,
        context_before,
        context_after,
    })
}

fn resolve_search_root(path: Option<&str>) -> Result<PathBuf> {
    let cwd = std::env::current_dir().context("Cannot determine current directory")?;
    let requested = path.map(PathBuf::from).unwrap_or_else(|| cwd.clone());
    let absolute = if requested.is_absolute() {
        requested
    } else {
        cwd.join(requested)
    };
    Ok(normalize_path(&absolute))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut cleaned = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                cleaned.pop();
            }
            std::path::Component::Prefix(_)
            | std::path::Component::RootDir
            | std::path::Component::Normal(_) => {
                cleaned.push(component.as_os_str());
            }
        }
    }
    if cleaned.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        cleaned
    }
}

fn line_to_snippet(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.len() <= 150 {
        trimmed.to_string()
    } else {
        format!("{}...", &trimmed[..150])
    }
}

fn stable_result_id(path: &str, line: usize, snippet: &str) -> String {
    let payload = format!("{}:{}:{}", path, line, snippet);
    let hash = blake3::hash(payload.as_bytes());
    hash.to_hex()[..16].to_string()
}

fn context_from_lines(
    lines: &[&str],
    line_num: usize,
    context: usize,
) -> (Vec<String>, Vec<String>) {
    if context == 0 || lines.is_empty() {
        return (vec![], vec![]);
    }

    let idx = line_num.saturating_sub(1);
    let start = idx.saturating_sub(context);
    let end = (idx + context + 1).min(lines.len());

    let before = lines[start..idx].iter().map(|l| (*l).to_string()).collect();
    let after = if idx + 1 < end {
        lines[idx + 1..end]
            .iter()
            .map(|l| (*l).to_string())
            .collect()
    } else {
        vec![]
    };
    (before, after)
}

fn context_from_string_lines(
    lines: &[String],
    line_num: usize,
    context: usize,
) -> (Vec<String>, Vec<String>) {
    if context == 0 || lines.is_empty() {
        return (vec![], vec![]);
    }
    let idx = line_num.saturating_sub(1);
    let start = idx.saturating_sub(context);
    let end = (idx + context + 1).min(lines.len());

    let before = lines[start..idx].to_vec();
    let after = if idx + 1 < end {
        lines[idx + 1..end].to_vec()
    } else {
        vec![]
    };
    (before, after)
}

fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn stable_result_id_is_deterministic() {
        let a = stable_result_id("src/lib.rs", 10, "fn alpha() {}");
        let b = stable_result_id("src/lib.rs", 10, "fn alpha() {}");
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn persist_and_load_hints_roundtrip() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let hints = vec![AgentHintInput {
            id: None,
            path: "src/lib.rs".to_string(),
            line: 3,
            snippet: "fn alpha() {}".to_string(),
        }];
        persist_expand_hints(root, hints).expect("persist");
        let map = load_hint_map(root).expect("load");
        assert_eq!(map.len(), 1);
        let only = map.values().next().expect("entry");
        assert_eq!(only.path, "src/lib.rs");
        assert_eq!(only.line, 3);
    }
}
