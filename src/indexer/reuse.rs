// SPDX-License-Identifier: MIT OR Apache-2.0

//! Local index warm-start reuse cache.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::indexer::manifest;
use crate::indexer::scanner::FileScanner;
use cgrep::utils::INDEX_DIR;

pub(crate) const REUSE_STATE_FILE_NAME: &str = "reuse-state.json";
const CACHE_SCHEMA_VERSION: &str = "1";
const CACHE_ENV_OVERRIDE: &str = "CGREP_REUSE_CACHE_DIR";
const CACHE_SUBDIR: &str = "indexes";
const FINGERPRINT_SAMPLE_SIZE: usize = 32;
const MAX_AUTO_CANDIDATES: usize = 64;
const INDEX_SCHEMA_FINGERPRINT: &str =
    "path:path_exact:content:language:symbols:doc_type:symbol_id:symbol_end_line:line_number";
const SYMBOLS_DB_FILE: &str = "embeddings.sqlite";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReuseMode {
    Off,
    Strict,
    Auto,
}

impl ReuseMode {
    pub fn parse(raw: &str) -> Result<Self> {
        match raw.to_ascii_lowercase().as_str() {
            "off" => Ok(Self::Off),
            "strict" => Ok(Self::Strict),
            "auto" => Ok(Self::Auto),
            other => anyhow::bail!(
                "Invalid value for --reuse: '{}'. Expected one of: off, strict, auto",
                other
            ),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Strict => "strict",
            Self::Auto => "auto",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReuseRuntimeState {
    pub schema_version: String,
    pub mode: String,
    pub decision: String,
    pub active: bool,
    pub updated_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ReuseDecision {
    pub mode: ReuseMode,
    pub decision: &'static str,
    pub active: bool,
    pub source: Option<String>,
    pub snapshot_key: Option<String>,
    pub repo_key: Option<String>,
    pub reason: Option<String>,
}

impl ReuseDecision {
    pub fn off() -> Self {
        Self {
            mode: ReuseMode::Off,
            decision: "off",
            active: false,
            source: None,
            snapshot_key: None,
            repo_key: None,
            reason: None,
        }
    }

    pub fn miss(mode: ReuseMode, repo_key: Option<String>, reason: &'static str) -> Self {
        Self {
            mode,
            decision: "miss",
            active: false,
            source: None,
            snapshot_key: None,
            repo_key,
            reason: Some(reason.to_string()),
        }
    }

    pub fn fallback(
        mode: ReuseMode,
        repo_key: Option<String>,
        snapshot_key: Option<String>,
        source: Option<String>,
        reason: &'static str,
    ) -> Self {
        Self {
            mode,
            decision: "fallback",
            active: false,
            source,
            snapshot_key,
            repo_key,
            reason: Some(reason.to_string()),
        }
    }

    pub fn hit(
        mode: ReuseMode,
        repo_key: String,
        snapshot_key: String,
        source: &'static str,
        active: bool,
    ) -> Self {
        Self {
            mode,
            decision: "hit",
            active,
            source: Some(source.to_string()),
            snapshot_key: Some(snapshot_key),
            repo_key: Some(repo_key),
            reason: None,
        }
    }

    pub fn as_runtime_state(&self) -> ReuseRuntimeState {
        ReuseRuntimeState {
            schema_version: CACHE_SCHEMA_VERSION.to_string(),
            mode: self.mode.as_str().to_string(),
            decision: self.decision.to_string(),
            active: self.active,
            updated_at: now_unix_ms(),
            source: self.source.clone(),
            snapshot_key: self.snapshot_key.clone(),
            repo_key: self.repo_key.clone(),
            reason: self.reason.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReuseProfile {
    pub profile_hash: String,
    pub excludes: Vec<String>,
    pub includes: Vec<String>,
    pub respect_git_ignore: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SimilarityFingerprint {
    pub digest: String,
    pub file_count: usize,
    pub sample_paths: Vec<String>,
    pub sample_hashes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnapshotMetadata {
    schema_version: String,
    repo_key: String,
    snapshot_key: String,
    created_at: u64,
    updated_at: u64,
    cgrep_version: String,
    index_schema_fingerprint: String,
    profile_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    head_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    manifest_root_hash: Option<String>,
    fingerprint: SimilarityFingerprint,
}

#[derive(Debug, Clone)]
struct SnapshotEntry {
    dir: PathBuf,
    metadata: SnapshotMetadata,
}

#[derive(Debug, Deserialize, Default)]
struct IndexMetadataView {
    #[serde(default)]
    files: HashMap<String, IndexFileMetadataView>,
}

#[derive(Debug, Deserialize, Default)]
struct IndexFileMetadataView {
    #[serde(default)]
    hash: String,
}

#[derive(Debug, Clone)]
struct RepoIdentity {
    repo_key: String,
}

pub fn reuse_state_path(root: &Path) -> PathBuf {
    root.join(INDEX_DIR).join(REUSE_STATE_FILE_NAME)
}

pub fn load_runtime_state(root: &Path) -> Option<ReuseRuntimeState> {
    let path = reuse_state_path(root);
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save_runtime_state(root: &Path, state: &ReuseRuntimeState) -> Result<()> {
    let path = reuse_state_path(root);
    let bytes = serde_json::to_vec_pretty(state)?;
    manifest::atomic_write_bytes(&path, &bytes)
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn cache_root() -> Option<PathBuf> {
    if let Ok(override_dir) = std::env::var(CACHE_ENV_OVERRIDE) {
        if !override_dir.trim().is_empty() {
            return Some(PathBuf::from(override_dir));
        }
    }

    #[cfg(windows)]
    {
        dirs::data_local_dir().map(|base| base.join("cgrep").join(CACHE_SUBDIR))
    }
    #[cfg(not(windows))]
    {
        dirs::cache_dir().map(|base| base.join("cgrep").join(CACHE_SUBDIR))
    }
}

fn read_trimmed(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let value = raw.trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn canonical_or_original(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn normalize_repo_name(root: &Path) -> String {
    let raw = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repo");
    let mut normalized = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
        } else if !normalized.ends_with('-') {
            normalized.push('-');
        }
    }
    let normalized = normalized.trim_matches('-').to_string();
    if normalized.is_empty() {
        "repo".to_string()
    } else {
        normalized
    }
}

fn normalized_repo_name_from_origin(origin: &str) -> Option<String> {
    let trimmed = origin.trim().trim_end_matches('/');
    let mut tail = trimmed
        .rsplit(['/', ':'])
        .next()
        .map(|s| s.trim())
        .unwrap_or_default()
        .to_string();
    if let Some(stripped) = tail.strip_suffix(".git") {
        tail = stripped.to_string();
    }
    if tail.is_empty() {
        None
    } else {
        let synthetic_root = PathBuf::from(tail);
        Some(normalize_repo_name(&synthetic_root))
    }
}

fn normalize_origin_url(raw: &str) -> String {
    let mut value = raw.trim().to_ascii_lowercase();
    if value.starts_with("git@") {
        if let Some((host, path)) = value
            .trim_start_matches("git@")
            .split_once(':')
            .map(|(h, p)| (h.trim(), p.trim()))
        {
            value = format!("ssh://{host}/{path}");
        }
    }
    if let Some(stripped) = value.strip_suffix(".git") {
        value = stripped.to_string();
    }
    value
}

fn git_output(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn repo_identity(root: &Path) -> RepoIdentity {
    let canonical = canonical_or_original(root);
    let fallback_name = normalize_repo_name(&canonical);
    let origin = git_output(&canonical, &["config", "--get", "remote.origin.url"])
        .map(|url| normalize_origin_url(&url));
    let normalized_name = origin
        .as_deref()
        .and_then(normalized_repo_name_from_origin)
        .unwrap_or(fallback_name);

    let key_source = if let Some(origin) = origin {
        format!("origin:{origin}|repo:{normalized_name}")
    } else {
        format!("fallback:{}|repo:{normalized_name}", canonical.display())
    };

    let key = blake3::hash(key_source.as_bytes()).to_hex().to_string();
    RepoIdentity {
        repo_key: key[..24].to_string(),
    }
}

fn head_commit(root: &Path) -> Option<String> {
    git_output(root, &["rev-parse", "HEAD"])
}

fn choose_sample_indices(len: usize, sample_size: usize) -> Vec<usize> {
    if len == 0 {
        return Vec::new();
    }
    if len <= sample_size || sample_size <= 1 {
        return (0..len).collect();
    }
    let mut out = Vec::with_capacity(sample_size);
    for idx in 0..sample_size {
        let mapped = idx * (len - 1) / (sample_size - 1);
        if out.last().copied() != Some(mapped) {
            out.push(mapped);
        }
    }
    out
}

fn hash_file_prefix(path: &Path) -> Result<String> {
    const MAX_SAMPLE_BYTES: usize = 16 * 1024;
    let file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut hasher = blake3::Hasher::new();
    let mut remaining = MAX_SAMPLE_BYTES;
    let mut buf = [0u8; 4096];
    while remaining > 0 {
        let read_cap = remaining.min(buf.len());
        let read = reader
            .read(&mut buf[..read_cap])
            .with_context(|| format!("failed to read {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
        remaining -= read;
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn fingerprint_from_workspace(
    root: &Path,
    profile: &ReuseProfile,
) -> Result<SimilarityFingerprint> {
    let scanner = FileScanner::with_excludes(root, profile.excludes.clone())
        .with_includes(profile.includes.clone())
        .with_gitignore(profile.respect_git_ignore);
    let files = scanner.list_files()?;

    let mut rel_abs_pairs: Vec<(String, PathBuf)> = files
        .iter()
        .filter_map(|abs| manifest::relative_path(root, abs).map(|rel| (rel, abs.clone())))
        .collect();
    rel_abs_pairs.sort_by(|a, b| a.0.cmp(&b.0));

    let indices = choose_sample_indices(rel_abs_pairs.len(), FINGERPRINT_SAMPLE_SIZE);
    let mut sample_paths = Vec::with_capacity(indices.len());
    let mut sample_hashes = Vec::with_capacity(indices.len());
    let mut hasher = blake3::Hasher::new();
    for index in indices {
        if let Some((rel, abs)) = rel_abs_pairs.get(index) {
            let hash = hash_file_prefix(abs)?;
            sample_paths.push(rel.clone());
            sample_hashes.push(hash.clone());
            hasher.update(rel.as_bytes());
            hasher.update(&[0]);
            hasher.update(hash.as_bytes());
            hasher.update(&[0]);
        }
    }
    let digest = hasher.finalize().to_hex().to_string();
    Ok(SimilarityFingerprint {
        digest,
        file_count: rel_abs_pairs.len(),
        sample_paths,
        sample_hashes,
    })
}

fn fingerprint_from_index_metadata(root: &Path) -> Result<SimilarityFingerprint> {
    let metadata_path = root.join(INDEX_DIR).join("metadata.json");
    let raw = fs::read_to_string(&metadata_path)
        .with_context(|| format!("failed to read {}", metadata_path.display()))?;
    let metadata: IndexMetadataView =
        serde_json::from_str(&raw).context("failed to parse index metadata.json")?;

    let mut rows: Vec<(String, String)> = metadata
        .files
        .iter()
        .filter_map(|(stored_path, file_meta)| {
            let rel = manifest::relative_path(root, Path::new(stored_path))?;
            Some((rel, file_meta.hash.clone()))
        })
        .collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0));

    let indices = choose_sample_indices(rows.len(), FINGERPRINT_SAMPLE_SIZE);
    let mut sample_paths = Vec::with_capacity(indices.len());
    let mut sample_hashes = Vec::with_capacity(indices.len());
    let mut hasher = blake3::Hasher::new();
    for index in indices {
        if let Some((rel, hash)) = rows.get(index) {
            sample_paths.push(rel.clone());
            sample_hashes.push(hash.clone());
            hasher.update(rel.as_bytes());
            hasher.update(&[0]);
            hasher.update(hash.as_bytes());
            hasher.update(&[0]);
        }
    }

    Ok(SimilarityFingerprint {
        digest: hasher.finalize().to_hex().to_string(),
        file_count: rows.len(),
        sample_paths,
        sample_hashes,
    })
}

fn profile_compatible(snapshot: &SnapshotMetadata, profile_hash: &str) -> bool {
    snapshot.schema_version == CACHE_SCHEMA_VERSION
        && snapshot.index_schema_fingerprint == INDEX_SCHEMA_FINGERPRINT
        && snapshot.profile_hash == profile_hash
}

fn snapshot_metadata_path(snapshot_dir: &Path) -> PathBuf {
    snapshot_dir.join("metadata.json")
}

fn snapshot_tantivy_dir(snapshot_dir: &Path) -> PathBuf {
    snapshot_dir.join("tantivy")
}

fn snapshot_symbols_dir(snapshot_dir: &Path) -> PathBuf {
    snapshot_dir.join("symbols")
}

fn snapshot_manifest_dir(snapshot_dir: &Path) -> PathBuf {
    snapshot_dir.join("manifest")
}

fn parse_snapshot_metadata(snapshot_dir: &Path) -> Option<SnapshotMetadata> {
    let path = snapshot_metadata_path(snapshot_dir);
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn snapshot_is_restorable(snapshot_dir: &Path) -> bool {
    let tantivy_dir = snapshot_tantivy_dir(snapshot_dir);
    tantivy_dir.join("meta.json").is_file() && tantivy_dir.join("metadata.json").is_file()
}

fn repo_cache_dir(root: &Path) -> Option<(PathBuf, RepoIdentity)> {
    let base = cache_root()?;
    let identity = repo_identity(root);
    Some((base.join(&identity.repo_key), identity))
}

fn list_snapshot_entries(root: &Path) -> Vec<SnapshotEntry> {
    let Some((repo_dir, _)) = repo_cache_dir(root) else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(repo_dir) else {
        return Vec::new();
    };

    let mut snapshots: Vec<SnapshotEntry> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let metadata = parse_snapshot_metadata(&path)?;
            Some(SnapshotEntry {
                dir: path,
                metadata,
            })
        })
        .collect();
    snapshots.sort_by(|a, b| {
        b.metadata
            .updated_at
            .cmp(&a.metadata.updated_at)
            .then_with(|| a.metadata.snapshot_key.cmp(&b.metadata.snapshot_key))
    });
    snapshots
}

fn similarity_score(snapshot: &SimilarityFingerprint, current: &SimilarityFingerprint) -> i64 {
    let snapshot_pairs: BTreeMap<&str, &str> = snapshot
        .sample_paths
        .iter()
        .zip(snapshot.sample_hashes.iter())
        .map(|(p, h)| (p.as_str(), h.as_str()))
        .collect();
    let current_pairs: BTreeMap<&str, &str> = current
        .sample_paths
        .iter()
        .zip(current.sample_hashes.iter())
        .map(|(p, h)| (p.as_str(), h.as_str()))
        .collect();

    let mut path_overlap = 0usize;
    let mut hash_overlap = 0usize;
    for (path, hash) in &snapshot_pairs {
        if let Some(current_hash) = current_pairs.get(path) {
            path_overlap += 1;
            if current_hash == hash {
                hash_overlap += 1;
            }
        }
    }

    let max_samples = snapshot_pairs.len().max(current_pairs.len()).max(1) as i64;
    let path_component = (path_overlap as i64 * 1000) / max_samples;
    let hash_component = (hash_overlap as i64 * 2000) / max_samples;

    let max_files = snapshot.file_count.max(current.file_count).max(1) as i64;
    let file_delta = (snapshot.file_count.abs_diff(current.file_count) as i64 * 1000) / max_files;

    hash_component + path_component - file_delta
}

fn copy_file(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(src, dst)
        .with_context(|| format!("failed to copy {} -> {}", src.display(), dst.display()))?;
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }
    fs::create_dir_all(dst)?;
    let mut stack: Vec<(PathBuf, PathBuf)> = vec![(src.to_path_buf(), dst.to_path_buf())];
    while let Some((from, to)) = stack.pop() {
        fs::create_dir_all(&to)?;
        for entry in fs::read_dir(&from)? {
            let entry = entry?;
            let entry_path = entry.path();
            let target_path = to.join(entry.file_name());
            if entry_path.is_dir() {
                stack.push((entry_path, target_path));
            } else if entry_path.is_file() {
                copy_file(&entry_path, &target_path)?;
            }
        }
    }
    Ok(())
}

fn clear_local_index_artifacts(root: &Path) -> Result<()> {
    let state_dir = root.join(INDEX_DIR);
    fs::create_dir_all(&state_dir)?;
    let keep_files = [
        "status.json",
        "index-background.log",
        "watch.pid",
        "watch.log",
        REUSE_STATE_FILE_NAME,
    ];
    for entry in fs::read_dir(&state_dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if keep_files.contains(&name) {
            continue;
        }
        if path.is_dir() {
            fs::remove_dir_all(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        } else if path.is_file() {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}

fn apply_snapshot(root: &Path, snapshot_dir: &Path) -> Result<()> {
    if !snapshot_is_restorable(snapshot_dir) {
        anyhow::bail!("snapshot artifacts are missing required files");
    }

    let state_dir = root.join(INDEX_DIR);
    fs::create_dir_all(&state_dir)?;
    clear_local_index_artifacts(root)?;

    let tantivy_dir = snapshot_tantivy_dir(snapshot_dir);
    copy_dir_recursive(&tantivy_dir, &state_dir)?;

    let manifest_dir = snapshot_manifest_dir(snapshot_dir);
    if manifest_dir.exists() {
        copy_dir_recursive(&manifest_dir, &state_dir.join("manifest"))?;
    }

    let symbol_db = snapshot_symbols_dir(snapshot_dir).join(SYMBOLS_DB_FILE);
    if symbol_db.is_file() {
        copy_file(&symbol_db, &state_dir.join(SYMBOLS_DB_FILE))?;
    }

    Ok(())
}

fn compatible_snapshot_entries(root: &Path, profile_hash: &str) -> Vec<SnapshotEntry> {
    let mut out: Vec<SnapshotEntry> = list_snapshot_entries(root)
        .into_iter()
        .filter(|entry| profile_compatible(&entry.metadata, profile_hash))
        .collect();
    out.sort_by(|a, b| {
        b.metadata
            .updated_at
            .cmp(&a.metadata.updated_at)
            .then_with(|| a.metadata.snapshot_key.cmp(&b.metadata.snapshot_key))
    });
    if out.len() > MAX_AUTO_CANDIDATES {
        out.truncate(MAX_AUTO_CANDIDATES);
    }
    out
}

pub fn try_restore_snapshot(
    root: &Path,
    mode: ReuseMode,
    profile: &ReuseProfile,
) -> Result<ReuseDecision> {
    if mode == ReuseMode::Off {
        return Ok(ReuseDecision::off());
    }

    let (repo_cache, identity) = match repo_cache_dir(root) {
        Some(value) => value,
        None => return Ok(ReuseDecision::miss(mode, None, "cache_root_unavailable")),
    };
    fs::create_dir_all(&repo_cache)?;

    let strict_head = head_commit(root);
    let selected = match mode {
        ReuseMode::Strict => {
            let Some(head) = strict_head.as_ref() else {
                return Ok(ReuseDecision::miss(
                    ReuseMode::Strict,
                    Some(identity.repo_key),
                    "head_unavailable",
                ));
            };
            let snapshot_dir = repo_cache.join(head);
            if !snapshot_dir.is_dir() {
                return Ok(ReuseDecision::miss(
                    ReuseMode::Strict,
                    Some(identity.repo_key),
                    "strict_snapshot_missing",
                ));
            }
            let Some(metadata) = parse_snapshot_metadata(&snapshot_dir) else {
                return Ok(ReuseDecision::fallback(
                    ReuseMode::Strict,
                    Some(identity.repo_key),
                    Some(head.clone()),
                    Some("strict".to_string()),
                    "snapshot_metadata_corrupt",
                ));
            };
            if !profile_compatible(&metadata, &profile.profile_hash) {
                return Ok(ReuseDecision::miss(
                    ReuseMode::Strict,
                    Some(identity.repo_key),
                    "snapshot_incompatible",
                ));
            }
            SnapshotEntry {
                dir: snapshot_dir,
                metadata,
            }
        }
        ReuseMode::Auto => {
            let current_fingerprint = fingerprint_from_workspace(root, profile)
                .context("failed to compute current workspace fingerprint")?;
            let entries = compatible_snapshot_entries(root, &profile.profile_hash);
            if entries.is_empty() {
                return Ok(ReuseDecision::miss(
                    ReuseMode::Auto,
                    Some(identity.repo_key),
                    "auto_snapshot_missing",
                ));
            }

            let mut best: Option<(i64, SnapshotEntry)> = None;
            for entry in entries {
                let score = similarity_score(&entry.metadata.fingerprint, &current_fingerprint);
                match &best {
                    None => {
                        best = Some((score, entry));
                    }
                    Some((best_score, best_entry)) => {
                        let ord = score
                            .cmp(best_score)
                            .then_with(|| {
                                entry
                                    .metadata
                                    .updated_at
                                    .cmp(&best_entry.metadata.updated_at)
                            })
                            .then_with(|| {
                                if entry.metadata.snapshot_key < best_entry.metadata.snapshot_key {
                                    Ordering::Greater
                                } else if entry.metadata.snapshot_key
                                    > best_entry.metadata.snapshot_key
                                {
                                    Ordering::Less
                                } else {
                                    Ordering::Equal
                                }
                            });
                        if ord == Ordering::Greater {
                            best = Some((score, entry));
                        }
                    }
                }
            }

            match best {
                Some((_score, entry)) => entry,
                None => {
                    return Ok(ReuseDecision::miss(
                        ReuseMode::Auto,
                        Some(identity.repo_key),
                        "auto_snapshot_missing",
                    ))
                }
            }
        }
        ReuseMode::Off => unreachable!(),
    };

    if !snapshot_is_restorable(&selected.dir) {
        return Ok(ReuseDecision::fallback(
            mode,
            Some(identity.repo_key),
            Some(selected.metadata.snapshot_key),
            Some(mode.as_str().to_string()),
            "snapshot_corrupt",
        ));
    }

    apply_snapshot(root, &selected.dir).context("failed to apply reuse snapshot")?;

    Ok(ReuseDecision::hit(
        mode,
        identity.repo_key,
        selected.metadata.snapshot_key,
        mode.as_str(),
        true,
    ))
}

fn manifest_root_hash(root: &Path) -> Option<String> {
    read_trimmed(&root.join(INDEX_DIR).join("manifest").join("root.hash"))
}

fn snapshot_key_for_store(root: &Path) -> Result<String> {
    if let Some(head) = head_commit(root) {
        return Ok(head);
    }
    let fingerprint = fingerprint_from_index_metadata(root)?;
    Ok(format!("snapshot-{}", &fingerprint.digest[..16]))
}

fn source_paths(root: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let state_dir = root.join(INDEX_DIR);
    (
        state_dir.clone(),
        state_dir.join("manifest"),
        state_dir.join(SYMBOLS_DB_FILE),
    )
}

fn copy_local_artifacts_to_snapshot(root: &Path, snapshot_stage: &Path) -> Result<()> {
    let (state_dir, manifest_dir, symbols_db) = source_paths(root);
    let snapshot_tantivy = snapshot_tantivy_dir(snapshot_stage);
    let snapshot_manifest = snapshot_manifest_dir(snapshot_stage);
    let snapshot_symbols = snapshot_symbols_dir(snapshot_stage);

    fs::create_dir_all(&snapshot_tantivy)?;
    fs::create_dir_all(&snapshot_symbols)?;

    for entry in fs::read_dir(&state_dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if matches!(
            name,
            "manifest"
                | "status.json"
                | "index-background.log"
                | "watch.pid"
                | "watch.log"
                | REUSE_STATE_FILE_NAME
        ) {
            continue;
        }
        if path.is_dir() {
            copy_dir_recursive(&path, &snapshot_tantivy.join(name))?;
        } else if path.is_file() {
            copy_file(&path, &snapshot_tantivy.join(name))?;
        }
    }

    if manifest_dir.exists() {
        copy_dir_recursive(&manifest_dir, &snapshot_manifest)?;
    }

    if symbols_db.is_file() {
        copy_file(&symbols_db, &snapshot_symbols.join(SYMBOLS_DB_FILE))?;
    }
    Ok(())
}

pub fn store_snapshot(root: &Path, profile_hash: &str) -> Result<Option<(String, String)>> {
    let Some((repo_dir, identity)) = repo_cache_dir(root) else {
        return Ok(None);
    };
    fs::create_dir_all(&repo_dir)?;

    let snapshot_key = snapshot_key_for_store(root)?;
    let snapshot_dir = repo_dir.join(&snapshot_key);
    if snapshot_dir.exists() {
        return Ok(Some((identity.repo_key, snapshot_key)));
    }

    let fingerprint = fingerprint_from_index_metadata(root)?;
    let metadata = SnapshotMetadata {
        schema_version: CACHE_SCHEMA_VERSION.to_string(),
        repo_key: identity.repo_key.clone(),
        snapshot_key: snapshot_key.clone(),
        created_at: now_unix_ms(),
        updated_at: now_unix_ms(),
        cgrep_version: env!("CARGO_PKG_VERSION").to_string(),
        index_schema_fingerprint: INDEX_SCHEMA_FINGERPRINT.to_string(),
        profile_hash: profile_hash.to_string(),
        head_commit: head_commit(root),
        manifest_root_hash: manifest_root_hash(root),
        fingerprint,
    };

    let staging = repo_dir.join(format!(".tmp-{}-{}", std::process::id(), now_unix_ms()));
    if staging.exists() {
        fs::remove_dir_all(&staging).ok();
    }
    fs::create_dir_all(&staging)?;
    copy_local_artifacts_to_snapshot(root, &staging)?;

    let metadata_raw = serde_json::to_vec_pretty(&metadata)?;
    manifest::atomic_write_bytes(&snapshot_metadata_path(&staging), &metadata_raw)?;

    if snapshot_dir.exists() {
        fs::remove_dir_all(&staging).ok();
        return Ok(Some((identity.repo_key, snapshot_key)));
    }
    fs::rename(&staging, &snapshot_dir).with_context(|| {
        format!(
            "failed to move snapshot stage {} -> {}",
            staging.display(),
            snapshot_dir.display()
        )
    })?;

    Ok(Some((identity.repo_key, snapshot_key)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn sample_indices_are_deterministic() {
        let first = choose_sample_indices(100, 8);
        let second = choose_sample_indices(100, 8);
        assert_eq!(first, second);
        assert_eq!(first.len(), 8);
        assert_eq!(first.first().copied(), Some(0));
        assert_eq!(first.last().copied(), Some(99));
    }

    #[test]
    fn similarity_prefers_hash_matches() {
        let base = SimilarityFingerprint {
            digest: "a".to_string(),
            file_count: 10,
            sample_paths: vec!["src/a.rs".to_string(), "src/b.rs".to_string()],
            sample_hashes: vec!["h1".to_string(), "h2".to_string()],
        };
        let same = SimilarityFingerprint {
            digest: "b".to_string(),
            file_count: 10,
            sample_paths: vec!["src/a.rs".to_string(), "src/b.rs".to_string()],
            sample_hashes: vec!["h1".to_string(), "h2".to_string()],
        };
        let diff = SimilarityFingerprint {
            digest: "c".to_string(),
            file_count: 10,
            sample_paths: vec!["src/a.rs".to_string(), "src/b.rs".to_string()],
            sample_hashes: vec!["x".to_string(), "y".to_string()],
        };
        assert!(similarity_score(&base, &same) > similarity_score(&base, &diff));
    }

    #[test]
    fn runtime_state_roundtrip() {
        let dir = TempDir::new().expect("tempdir");
        let root = dir.path();
        fs::create_dir_all(root.join(INDEX_DIR)).expect("mkdir .cgrep");
        let state = ReuseDecision::hit(
            ReuseMode::Strict,
            "repo-key".to_string(),
            "snapshot-key".to_string(),
            "strict",
            true,
        )
        .as_runtime_state();
        save_runtime_state(root, &state).expect("save");
        let loaded = load_runtime_state(root).expect("load");
        assert_eq!(loaded.mode, "strict");
        assert_eq!(loaded.decision, "hit");
        assert!(loaded.active);
    }
}
