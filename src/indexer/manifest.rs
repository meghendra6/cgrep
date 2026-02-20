// SPDX-License-Identifier: MIT OR Apache-2.0

//! Manifest subsystem for fast change detection and incremental indexing.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::indexer::scanner::detect_language;

pub(crate) const MANIFEST_VERSION: &str = "1";
pub(crate) const MANIFEST_DIR_REL: &str = ".cgrep/manifest";
pub(crate) const MANIFEST_VERSION_FILE_REL: &str = ".cgrep/manifest/version";
pub(crate) const MANIFEST_V1_FILE_REL: &str = ".cgrep/manifest/v1.json";
pub(crate) const MANIFEST_ROOT_HASH_FILE_REL: &str = ".cgrep/manifest/root.hash";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ManifestEntry {
    pub path: String,
    pub size: u64,
    pub mtime: u64,
    pub hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Manifest {
    #[serde(default)]
    pub entries: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ManifestDiffSummary {
    #[serde(default)]
    pub added: Vec<String>,
    #[serde(default)]
    pub modified: Vec<String>,
    #[serde(default)]
    pub deleted: Vec<String>,
    pub unchanged: usize,
    pub scanned: usize,
    pub suspects: usize,
    pub hashed: usize,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ManifestDiff {
    pub summary: ManifestDiffSummary,
    pub next: Manifest,
}

pub(crate) fn load_manifest(root: &Path) -> Option<Manifest> {
    let path = root.join(MANIFEST_V1_FILE_REL);
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

pub(crate) fn write_manifest(root: &Path, manifest: &Manifest) -> Result<()> {
    let manifest_dir = root.join(MANIFEST_DIR_REL);
    std::fs::create_dir_all(&manifest_dir)
        .with_context(|| format!("failed to create {}", manifest_dir.display()))?;

    let mut sorted = manifest.clone();
    sorted.entries.sort_by(|a, b| a.path.cmp(&b.path));

    let version_path = root.join(MANIFEST_VERSION_FILE_REL);
    atomic_write_bytes(&version_path, format!("{}\n", MANIFEST_VERSION).as_bytes())?;

    let manifest_path = root.join(MANIFEST_V1_FILE_REL);
    let content = serde_json::to_string_pretty(&sorted)?;
    atomic_write_bytes(&manifest_path, content.as_bytes())?;

    let root_hash = compute_root_hash(&sorted.entries);
    let root_hash_path = root.join(MANIFEST_ROOT_HASH_FILE_REL);
    atomic_write_bytes(&root_hash_path, format!("{root_hash}\n").as_bytes())?;

    Ok(())
}

pub(crate) fn compute_manifest_diff(
    root: &Path,
    files: &[PathBuf],
    old_manifest: Option<&Manifest>,
) -> Result<ManifestDiff> {
    let old = old_manifest.cloned().unwrap_or_default();
    let old_map: HashMap<&str, &ManifestEntry> = old
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect();

    let mut rel_abs_pairs: Vec<(String, PathBuf)> = files
        .iter()
        .filter_map(|abs| relative_path(root, abs).map(|rel| (rel, abs.clone())))
        .collect();
    rel_abs_pairs.sort_by(|a, b| a.0.cmp(&b.0));

    let mut seen: HashSet<String> = HashSet::with_capacity(rel_abs_pairs.len());
    let mut next_entries: Vec<ManifestEntry> = Vec::with_capacity(rel_abs_pairs.len());
    let mut summary = ManifestDiffSummary::default();

    for (rel, abs) in rel_abs_pairs {
        let metadata = match std::fs::metadata(&abs) {
            Ok(metadata) if metadata.is_file() => metadata,
            _ => continue,
        };
        seen.insert(rel.clone());
        let size = metadata.len();
        let mtime = file_mtime_nanos(&metadata);

        let old_entry = old_map.get(rel.as_str()).copied();
        let mut hash = String::new();
        let mut unchanged = false;

        if let Some(existing) = old_entry {
            if existing.size == size && existing.mtime == mtime {
                hash = existing.hash.clone();
                unchanged = true;
            }
        }

        if !unchanged {
            summary.suspects += 1;
            hash = hash_file_streaming(&abs)?;
            summary.hashed += 1;
            match old_entry {
                Some(existing) if existing.hash == hash => {
                    unchanged = true;
                }
                Some(_) => summary.modified.push(rel.clone()),
                None => summary.added.push(rel.clone()),
            }
        }

        let ext = abs
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase());
        let language = ext.as_deref().and_then(detect_language);

        if unchanged && hash.is_empty() {
            hash = old_entry
                .map(|entry| entry.hash.clone())
                .unwrap_or_else(String::new);
        }

        next_entries.push(ManifestEntry {
            path: rel,
            size,
            mtime,
            hash,
            language,
            ext,
        });
    }

    for old_entry in &old.entries {
        if !seen.contains(&old_entry.path) {
            summary.deleted.push(old_entry.path.clone());
        }
    }

    summary.added.sort();
    summary.modified.sort();
    summary.deleted.sort();
    summary.scanned = next_entries.len();
    summary.unchanged = summary
        .scanned
        .saturating_sub(summary.added.len() + summary.modified.len());

    next_entries.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(ManifestDiff {
        summary,
        next: Manifest {
            entries: next_entries,
        },
    })
}

pub(crate) fn apply_manifest_delta(
    root: &Path,
    changed_paths: &[PathBuf],
    old_manifest: Option<&Manifest>,
) -> Result<ManifestDiff> {
    let old = old_manifest.cloned().unwrap_or_default();
    let mut entries_map: HashMap<String, ManifestEntry> = old
        .entries
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect();

    let mut rel_paths: Vec<String> = changed_paths
        .iter()
        .filter_map(|path| {
            if path.is_absolute() {
                relative_path(root, path)
            } else {
                relative_path(root, &root.join(path))
            }
        })
        .collect();
    rel_paths.sort();
    rel_paths.dedup();

    let mut summary = ManifestDiffSummary::default();
    for rel in rel_paths {
        let abs = root.join(&rel);
        summary.scanned += 1;
        if !abs.exists() || !abs.is_file() {
            if entries_map.remove(&rel).is_some() {
                summary.deleted.push(rel);
            }
            continue;
        }

        let metadata = match std::fs::metadata(&abs) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        let size = metadata.len();
        let mtime = file_mtime_nanos(&metadata);

        let old_entry = entries_map.get(&rel).cloned();
        let mut hash = String::new();
        let mut unchanged = false;

        if let Some(existing) = old_entry.as_ref() {
            if existing.size == size && existing.mtime == mtime {
                hash = existing.hash.clone();
                unchanged = true;
            }
        }

        if !unchanged {
            summary.suspects += 1;
            hash = hash_file_streaming(&abs)?;
            summary.hashed += 1;
            match old_entry.as_ref() {
                Some(existing) if existing.hash == hash => {
                    unchanged = true;
                }
                Some(_) => summary.modified.push(rel.clone()),
                None => summary.added.push(rel.clone()),
            }
        }

        let ext = abs
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase());
        let language = ext.as_deref().and_then(detect_language);

        if unchanged && hash.is_empty() {
            hash = old_entry
                .as_ref()
                .map(|entry| entry.hash.clone())
                .unwrap_or_else(String::new);
        }

        entries_map.insert(
            rel.clone(),
            ManifestEntry {
                path: rel,
                size,
                mtime,
                hash,
                language,
                ext,
            },
        );

        if unchanged {
            summary.unchanged += 1;
        }
    }

    summary.added.sort();
    summary.modified.sort();
    summary.deleted.sort();

    let mut next_entries: Vec<ManifestEntry> = entries_map.into_values().collect();
    next_entries.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(ManifestDiff {
        summary,
        next: Manifest {
            entries: next_entries,
        },
    })
}

pub(crate) fn relative_path(root: &Path, abs: &Path) -> Option<String> {
    let rel = abs.strip_prefix(root).ok()?;
    let path = rel.to_string_lossy().replace('\\', "/");
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

fn file_mtime_nanos(metadata: &std::fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0)
}

fn hash_file_streaming(path: &Path) -> Result<String> {
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 64 * 1024];

    loop {
        let read = reader.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }

    Ok(hasher.finalize().to_hex().to_string())
}

fn compute_root_hash(entries: &[ManifestEntry]) -> String {
    let mut hasher = blake3::Hasher::new();
    for entry in entries {
        hasher.update(entry.path.as_bytes());
        hasher.update(&[0]);
        hasher.update(entry.size.to_string().as_bytes());
        hasher.update(&[0]);
        hasher.update(entry.mtime.to_string().as_bytes());
        hasher.update(&[0]);
        hasher.update(entry.hash.as_bytes());
        hasher.update(&[0]);
        if let Some(ext) = &entry.ext {
            hasher.update(ext.as_bytes());
        }
        hasher.update(&[0]);
        if let Some(language) = &entry.language {
            hasher.update(language.as_bytes());
        }
        hasher.update(&[0]);
    }
    hasher.finalize().to_hex().to_string()
}

pub(crate) fn atomic_write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    let Some(parent) = path.parent() else {
        anyhow::bail!("cannot atomically write {} without parent", path.display());
    };
    std::fs::create_dir_all(parent)?;

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let tmp_name = format!(
        ".{}.tmp-{}-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("cgrep"),
        std::process::id(),
        nonce
    );
    let tmp_path = parent.join(tmp_name);

    {
        let mut file = File::create(&tmp_path)
            .with_context(|| format!("failed to create {}", tmp_path.display()))?;
        file.write_all(bytes)
            .with_context(|| format!("failed to write {}", tmp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("failed to sync {}", tmp_path.display()))?;
    }

    if let Err(err) = std::fs::rename(&tmp_path, path) {
        if path.exists() {
            let _ = std::fs::remove_file(path);
            std::fs::rename(&tmp_path, path).with_context(|| {
                format!(
                    "failed to replace {} with {} after rename error: {err}",
                    path.display(),
                    tmp_path.display()
                )
            })?;
        } else {
            return Err(err.into());
        }
    }

    Ok(())
}
