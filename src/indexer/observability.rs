// SPDX-License-Identifier: MIT OR Apache-2.0

//! Observability and diagnostics for index state.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tantivy::{
    collector::Count,
    query::{AllQuery, TermQuery},
    schema::IndexRecordOption,
    Index, Term,
};

use crate::cli::OutputFormat;
use crate::indexer::manifest::{
    ManifestDiffSummary, MANIFEST_ROOT_HASH_FILE_REL, MANIFEST_V1_FILE_REL, MANIFEST_VERSION,
    MANIFEST_VERSION_FILE_REL,
};
use cgrep::output::print_json;
use cgrep::utils::{find_index_root, INDEX_DIR};

const METADATA_FILE_REL: &str = ".cgrep/metadata.json";
pub(crate) const STATS_FILE_REL: &str = ".cgrep/stats.json";
const WATCH_PID_FILE_REL: &str = ".cgrep/watch.pid";
const WATCH_LOG_FILE_REL: &str = ".cgrep/watch.log";

const STATUS_SCHEMA_VERSION: &str = "1";
const STATS_SCHEMA_VERSION: &str = "1";
const DOCTOR_SCHEMA_VERSION: &str = "1";

const INDEX_SCHEMA_SYMBOL_V1: &str = "symbol-v1";
const INDEX_SCHEMA_LEGACY: &str = "legacy-or-unknown";
const INDEX_SCHEMA_CORRUPT: &str = "corrupt-or-unreadable";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub(crate) struct RunTimingsMs {
    pub scan_ms: Option<u64>,
    pub hash_ms: Option<u64>,
    pub parse_ms: Option<u64>,
    pub index_ms: Option<u64>,
    pub commit_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub(crate) struct DiffCounts {
    pub added: Option<usize>,
    pub modified: Option<usize>,
    pub deleted: Option<usize>,
    pub unchanged: Option<usize>,
    pub scanned: Option<usize>,
    pub suspects: Option<usize>,
    pub hashed: Option<usize>,
}

impl DiffCounts {
    pub(crate) fn from_manifest(summary: &ManifestDiffSummary) -> Self {
        Self {
            added: Some(summary.added.len()),
            modified: Some(summary.modified.len()),
            deleted: Some(summary.deleted.len()),
            unchanged: Some(summary.unchanged),
            scanned: Some(summary.scanned),
            suspects: Some(summary.suspects),
            hashed: Some(summary.hashed),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub(crate) struct CacheReuseStats {
    pub hit: Option<u64>,
    pub miss: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub(crate) struct LastRunStats {
    pub mode: String,
    pub force: bool,
    pub started_at_ms: u64,
    pub finished_at_ms: u64,
    pub total_ms: u64,
    pub timings_ms: RunTimingsMs,
    pub diff: DiffCounts,
    pub cache_reuse: CacheReuseStats,
    pub indexed_files: usize,
    pub skipped_files: usize,
    pub deleted_files: usize,
    pub error_files: usize,
}

impl Default for LastRunStats {
    fn default() -> Self {
        Self {
            mode: "unknown".to_string(),
            force: false,
            started_at_ms: 0,
            finished_at_ms: 0,
            total_ms: 0,
            timings_ms: RunTimingsMs::default(),
            diff: DiffCounts::default(),
            cache_reuse: CacheReuseStats::default(),
            indexed_files: 0,
            skipped_files: 0,
            deleted_files: 0,
            error_files: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub(crate) struct PersistedStats {
    pub schema_version: String,
    pub last_run: Option<LastRunStats>,
}

impl Default for PersistedStats {
    fn default() -> Self {
        Self {
            schema_version: STATS_SCHEMA_VERSION.to_string(),
            last_run: None,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MetadataSnapshot {
    files: BTreeMap<String, serde_json::Value>,
    manifest_diff: Option<ManifestDiffSummary>,
    last_run_stats: Option<LastRunStats>,
}

#[derive(Debug, Clone)]
struct ResolvedRoots {
    requested_root: PathBuf,
    index_root: PathBuf,
    using_parent_index: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
struct DocCounts {
    total_docs: Option<u64>,
    file_docs: Option<u64>,
    symbol_docs: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
struct StatusIndex {
    exists: bool,
    tantivy_meta_exists: bool,
    metadata_exists: bool,
    schema_version: Option<String>,
    schema_ok: bool,
    tracked_files: Option<usize>,
    docs: DocCounts,
}

#[derive(Debug, Clone, Serialize)]
struct StatusManifest {
    version: Option<String>,
    has_v1_snapshot: bool,
    has_root_hash: bool,
    last_diff: DiffCounts,
}

#[derive(Debug, Clone, Serialize)]
struct StatusReadiness {
    basic_ready: bool,
    full_ready: bool,
    last_build_time_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
struct StatusWatch {
    status: String,
    pid: Option<u32>,
    stale_pid: bool,
    pid_file_exists: bool,
    log_file_exists: bool,
}

#[derive(Debug, Clone, Serialize)]
struct StatusResult {
    root: String,
    index_root: String,
    using_parent_index: bool,
    index: StatusIndex,
    manifest: StatusManifest,
    readiness: StatusReadiness,
    watch: StatusWatch,
}

#[derive(Debug, Clone, Serialize)]
struct StatusJson2Meta {
    schema_version: &'static str,
    command: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct StatusJson2Payload {
    meta: StatusJson2Meta,
    result: StatusResult,
}

#[derive(Debug, Clone, Serialize)]
struct StatsResult {
    root: String,
    index_root: String,
    using_parent_index: bool,
    stats_file_exists: bool,
    stats_schema_version: Option<String>,
    last_run: Option<LastRunStats>,
    diff_counts: DiffCounts,
    cache_reuse: CacheReuseStats,
}

#[derive(Debug, Clone, Serialize)]
struct StatsJson2Meta {
    schema_version: &'static str,
    command: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct StatsJson2Payload {
    meta: StatsJson2Meta,
    result: StatsResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FindingSeverity {
    Error,
    Warning,
}

impl FindingSeverity {
    fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }
}

#[derive(Debug, Clone)]
struct DoctorFindingInternal {
    id: &'static str,
    severity: FindingSeverity,
    message: String,
    recommendation: String,
}

#[derive(Debug, Clone, Serialize)]
struct DoctorFinding {
    id: String,
    severity: &'static str,
    message: String,
    recommendation: String,
}

#[derive(Debug, Clone, Serialize)]
struct DoctorResult {
    root: String,
    index_root: String,
    using_parent_index: bool,
    healthy: bool,
    errors: usize,
    warnings: usize,
    findings: Vec<DoctorFinding>,
}

#[derive(Debug, Clone, Serialize)]
struct DoctorJson2Meta {
    schema_version: &'static str,
    command: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct DoctorJson2Payload {
    meta: DoctorJson2Meta,
    result: DoctorResult,
}

pub(crate) fn now_epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

pub(crate) fn duration_to_millis(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

pub(crate) fn stats_path(root: &Path) -> PathBuf {
    root.join(STATS_FILE_REL)
}

pub(crate) fn load_stats(root: &Path) -> Option<PersistedStats> {
    let path = stats_path(root);
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

#[cfg(test)]
pub(crate) fn persist_last_run(root: &Path, run: LastRunStats) -> Result<()> {
    let mut state = load_stats(root).unwrap_or_default();
    state.schema_version = STATS_SCHEMA_VERSION.to_string();
    state.last_run = Some(run);

    let bytes = serde_json::to_vec(&state)?;
    atomic_write_stats_bytes(&stats_path(root), &bytes)
}

#[cfg(test)]
fn atomic_write_stats_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("cannot write stats without parent: {}", path.display()))?;
    fs::create_dir_all(parent)?;

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let tmp_name = format!(
        ".{}.tmp-{}-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("stats"),
        std::process::id(),
        nonce
    );
    let tmp_path = parent.join(tmp_name);

    fs::write(&tmp_path, bytes)
        .with_context(|| format!("failed to write temp stats file {}", tmp_path.display()))?;

    if let Err(err) = fs::rename(&tmp_path, path) {
        if path.exists() {
            let _ = fs::remove_file(path);
            fs::rename(&tmp_path, path).with_context(|| {
                format!(
                    "failed to replace stats file {} with {} after rename error: {err}",
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

pub fn status(path: Option<&str>, format: OutputFormat, compact: bool) -> Result<()> {
    let roots = resolve_roots(path)?;
    let result = collect_status(&roots)?;

    match format {
        OutputFormat::Text => print_status_text(&result),
        OutputFormat::Json => print_json(&result, compact)?,
        OutputFormat::Json2 => {
            let payload = StatusJson2Payload {
                meta: StatusJson2Meta {
                    schema_version: STATUS_SCHEMA_VERSION,
                    command: "status",
                },
                result,
            };
            print_json(&payload, compact)?;
        }
    }

    Ok(())
}

pub fn stats(path: Option<&str>, format: OutputFormat, compact: bool) -> Result<()> {
    let roots = resolve_roots(path)?;
    let result = collect_stats(&roots)?;

    match format {
        OutputFormat::Text => print_stats_text(&result),
        OutputFormat::Json => print_json(&result, compact)?,
        OutputFormat::Json2 => {
            let payload = StatsJson2Payload {
                meta: StatsJson2Meta {
                    schema_version: STATS_SCHEMA_VERSION,
                    command: "stats",
                },
                result,
            };
            print_json(&payload, compact)?;
        }
    }

    Ok(())
}

pub fn doctor(path: Option<&str>, format: OutputFormat, compact: bool) -> Result<()> {
    let roots = resolve_roots(path)?;
    let result = collect_doctor(&roots);

    match format {
        OutputFormat::Text => print_doctor_text(&result),
        OutputFormat::Json => print_json(&result, compact)?,
        OutputFormat::Json2 => {
            let payload = DoctorJson2Payload {
                meta: DoctorJson2Meta {
                    schema_version: DOCTOR_SCHEMA_VERSION,
                    command: "doctor",
                },
                result,
            };
            print_json(&payload, compact)?;
        }
    }

    Ok(())
}

fn resolve_roots(path: Option<&str>) -> Result<ResolvedRoots> {
    let requested_root = path
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| anyhow::anyhow!("Cannot determine current directory"))?;
    let requested_root = requested_root.canonicalize().unwrap_or(requested_root);

    if let Some(found) = find_index_root(&requested_root) {
        return Ok(ResolvedRoots {
            requested_root,
            index_root: found.root,
            using_parent_index: found.is_parent,
        });
    }

    Ok(ResolvedRoots {
        requested_root: requested_root.clone(),
        index_root: requested_root,
        using_parent_index: false,
    })
}

fn collect_status(roots: &ResolvedRoots) -> Result<StatusResult> {
    let index_dir = roots.index_root.join(INDEX_DIR);
    let metadata_path = roots.index_root.join(METADATA_FILE_REL);
    let stats_path = stats_path(&roots.index_root);

    let metadata = load_metadata(&metadata_path).ok().flatten();
    let stats = load_stats(&roots.index_root);
    let latest_run = stats
        .as_ref()
        .and_then(|state| state.last_run.as_ref())
        .cloned()
        .or_else(|| {
            metadata
                .as_ref()
                .and_then(|meta| meta.last_run_stats.as_ref())
                .cloned()
        });

    let mut docs = DocCounts::default();
    let mut schema_version: Option<String> = None;
    let mut schema_ok = false;
    if index_dir.join("meta.json").exists() {
        if let Ok((version, ok, counts)) = inspect_index(&index_dir) {
            schema_version = Some(version.to_string());
            schema_ok = ok;
            docs = counts;
        } else {
            schema_version = Some(INDEX_SCHEMA_CORRUPT.to_string());
        }
    }

    let manifest_version = read_trimmed(roots.index_root.join(MANIFEST_VERSION_FILE_REL));
    let last_diff = metadata
        .as_ref()
        .and_then(|meta| meta.manifest_diff.as_ref())
        .map(DiffCounts::from_manifest)
        .unwrap_or_default();

    let last_build_time_ms = latest_run
        .as_ref()
        .map(|run| run.finished_at_ms)
        .filter(|v| *v > 0)
        .or_else(|| file_modified_ms(&metadata_path))
        .or_else(|| file_modified_ms(&stats_path));

    let basic_ready = index_dir.join("meta.json").exists() && schema_ok;
    let full_ready = basic_ready
        && metadata_path.exists()
        && manifest_version.as_deref() == Some(MANIFEST_VERSION)
        && latest_run.is_some();

    Ok(StatusResult {
        root: roots.requested_root.display().to_string(),
        index_root: roots.index_root.display().to_string(),
        using_parent_index: roots.using_parent_index,
        index: StatusIndex {
            exists: index_dir.exists(),
            tantivy_meta_exists: index_dir.join("meta.json").exists(),
            metadata_exists: metadata_path.exists(),
            schema_version,
            schema_ok,
            tracked_files: metadata.as_ref().map(|meta| meta.files.len()),
            docs,
        },
        manifest: StatusManifest {
            version: manifest_version,
            has_v1_snapshot: roots.index_root.join(MANIFEST_V1_FILE_REL).exists(),
            has_root_hash: roots.index_root.join(MANIFEST_ROOT_HASH_FILE_REL).exists(),
            last_diff,
        },
        readiness: StatusReadiness {
            basic_ready,
            full_ready,
            last_build_time_ms,
        },
        watch: collect_watch_status(&roots.index_root),
    })
}

fn collect_stats(roots: &ResolvedRoots) -> Result<StatsResult> {
    let metadata_path = roots.index_root.join(METADATA_FILE_REL);
    let metadata = load_metadata(&metadata_path).ok().flatten();
    let stats_file = stats_path(&roots.index_root);
    let loaded = load_stats(&roots.index_root);
    let last_run = loaded
        .as_ref()
        .and_then(|state| state.last_run.as_ref())
        .cloned()
        .or_else(|| {
            metadata
                .as_ref()
                .and_then(|meta| meta.last_run_stats.as_ref())
                .cloned()
        });

    let diff_counts = last_run
        .as_ref()
        .map(|run| run.diff.clone())
        .or_else(|| {
            metadata
                .as_ref()
                .and_then(|meta| meta.manifest_diff.as_ref())
                .map(DiffCounts::from_manifest)
        })
        .unwrap_or_default();

    let cache_reuse = last_run
        .as_ref()
        .map(|run| run.cache_reuse.clone())
        .unwrap_or_default();
    let stats_schema_version = loaded
        .as_ref()
        .map(|state| state.schema_version.clone())
        .or_else(|| {
            if metadata
                .as_ref()
                .and_then(|meta| meta.last_run_stats.as_ref())
                .is_some()
            {
                Some("embedded-v1".to_string())
            } else {
                None
            }
        });

    Ok(StatsResult {
        root: roots.requested_root.display().to_string(),
        index_root: roots.index_root.display().to_string(),
        using_parent_index: roots.using_parent_index,
        stats_file_exists: stats_file.exists(),
        stats_schema_version,
        last_run,
        diff_counts,
        cache_reuse,
    })
}

fn collect_doctor(roots: &ResolvedRoots) -> DoctorResult {
    let index_dir = roots.index_root.join(INDEX_DIR);
    let metadata_path = roots.index_root.join(METADATA_FILE_REL);
    let stats_path = stats_path(&roots.index_root);

    let mut findings: Vec<DoctorFindingInternal> = Vec::new();

    if !index_dir.exists() {
        findings.push(DoctorFindingInternal {
            id: "missing_index_dir",
            severity: FindingSeverity::Warning,
            message: format!("Index directory is missing: {}", index_dir.display()),
            recommendation: "Run `cgrep index` to create a fresh local index.".to_string(),
        });
    } else if !index_dir.is_dir() {
        findings.push(DoctorFindingInternal {
            id: "index_path_not_directory",
            severity: FindingSeverity::Error,
            message: format!("Index path is not a directory: {}", index_dir.display()),
            recommendation: "Remove the invalid path and rebuild with `cgrep index --force`."
                .to_string(),
        });
    }

    let tantivy_meta = index_dir.join("meta.json");
    if index_dir.is_dir() && !tantivy_meta.exists() {
        findings.push(DoctorFindingInternal {
            id: "missing_tantivy_meta",
            severity: FindingSeverity::Error,
            message: format!("Missing Tantivy metadata file: {}", tantivy_meta.display()),
            recommendation: "Rebuild the index with `cgrep index --force`.".to_string(),
        });
    }

    if tantivy_meta.exists() {
        match inspect_index(&index_dir) {
            Ok((schema, schema_ok, _)) => {
                if !schema_ok {
                    findings.push(DoctorFindingInternal {
                        id: "index_schema_mismatch",
                        severity: FindingSeverity::Warning,
                        message: format!(
                            "Index schema is not fully compatible (detected: {}).",
                            schema
                        ),
                        recommendation:
                            "Run `cgrep index --force` to upgrade index schema in-place."
                                .to_string(),
                    });
                }
            }
            Err(err) => findings.push(DoctorFindingInternal {
                id: "corrupt_tantivy_index",
                severity: FindingSeverity::Error,
                message: format!("Failed to open index: {err}"),
                recommendation:
                    "Rebuild index artifacts with `cgrep index --force` and retry doctor."
                        .to_string(),
            }),
        }
    }

    let metadata = if metadata_path.exists() {
        match load_metadata(&metadata_path) {
            Ok(meta) => meta,
            Err(err) => {
                findings.push(DoctorFindingInternal {
                    id: "metadata_parse_error",
                    severity: FindingSeverity::Error,
                    message: format!("Failed to parse {}: {err}", metadata_path.display()),
                    recommendation:
                        "Inspect/remove the metadata file and rebuild with `cgrep index`."
                            .to_string(),
                });
                None
            }
        }
    } else {
        if tantivy_meta.exists() {
            findings.push(DoctorFindingInternal {
                id: "missing_metadata_file",
                severity: FindingSeverity::Warning,
                message: format!("Missing index metadata file: {}", metadata_path.display()),
                recommendation:
                    "Run `cgrep index` once to regenerate metadata.json and manifest summary."
                        .to_string(),
            });
        }
        None
    };

    let manifest_version_path = roots.index_root.join(MANIFEST_VERSION_FILE_REL);
    let manifest_v1_path = roots.index_root.join(MANIFEST_V1_FILE_REL);
    let manifest_root_hash_path = roots.index_root.join(MANIFEST_ROOT_HASH_FILE_REL);

    let manifest_version = read_trimmed(&manifest_version_path);
    if manifest_version.is_none()
        && (manifest_v1_path.exists()
            || manifest_root_hash_path.exists()
            || metadata
                .as_ref()
                .and_then(|meta| meta.manifest_diff.as_ref())
                .is_some())
    {
        findings.push(DoctorFindingInternal {
            id: "missing_manifest_version",
            severity: FindingSeverity::Warning,
            message: format!(
                "Missing manifest version file: {}",
                manifest_version_path.display()
            ),
            recommendation:
                "Refresh manifest artifacts with `cgrep index --manifest-only --print-diff`."
                    .to_string(),
        });
    }

    if let Some(version) = manifest_version.as_ref() {
        if version != MANIFEST_VERSION {
            findings.push(DoctorFindingInternal {
                id: "manifest_version_mismatch",
                severity: FindingSeverity::Warning,
                message: format!(
                    "Manifest version mismatch: expected {}, found {}.",
                    MANIFEST_VERSION, version
                ),
                recommendation:
                    "Rebuild manifest with `cgrep index --manifest-only` using this binary."
                        .to_string(),
            });
        }

        if !manifest_v1_path.exists() {
            findings.push(DoctorFindingInternal {
                id: "missing_manifest_snapshot",
                severity: FindingSeverity::Warning,
                message: format!(
                    "Missing manifest snapshot file: {}",
                    manifest_v1_path.display()
                ),
                recommendation: "Regenerate manifest snapshot via `cgrep index --manifest-only`."
                    .to_string(),
            });
        } else if let Err(err) = load_manifest_json(&manifest_v1_path) {
            findings.push(DoctorFindingInternal {
                id: "manifest_parse_error",
                severity: FindingSeverity::Error,
                message: format!("Failed to parse {}: {err}", manifest_v1_path.display()),
                recommendation:
                    "Rebuild manifest snapshot using `cgrep index --manifest-only --print-diff`."
                        .to_string(),
            });
        }
    }

    if stats_path.exists() {
        if let Some(stats) = load_stats(&roots.index_root) {
            if stats.schema_version != STATS_SCHEMA_VERSION {
                findings.push(DoctorFindingInternal {
                    id: "stats_schema_mismatch",
                    severity: FindingSeverity::Warning,
                    message: format!(
                        "Stats schema mismatch: expected {}, found {}.",
                        STATS_SCHEMA_VERSION, stats.schema_version
                    ),
                    recommendation:
                        "Run `cgrep index` once to refresh observability stats with this binary."
                            .to_string(),
                });
            }
        } else {
            findings.push(DoctorFindingInternal {
                id: "stats_parse_error",
                severity: FindingSeverity::Warning,
                message: format!("Failed to parse {}.", stats_path.display()),
                recommendation:
                    "Delete the malformed stats file and run `cgrep index` to recreate it."
                        .to_string(),
            });
        }
    }

    let tmp_files = collect_tmp_files(&roots.index_root.join(INDEX_DIR));
    if !tmp_files.is_empty() {
        findings.push(DoctorFindingInternal {
            id: "interrupted_state_tmp_files",
            severity: FindingSeverity::Warning,
            message: format!(
                "Detected {} temporary file(s) under .cgrep that may indicate interrupted writes.",
                tmp_files.len()
            ),
            recommendation:
                "Validate current index with `cgrep status`; if inconsistent, run `cgrep index --force`."
                    .to_string(),
        });
    }

    findings.sort_by(|a, b| {
        a.severity
            .cmp(&b.severity)
            .then_with(|| a.id.cmp(b.id))
            .then_with(|| a.message.cmp(&b.message))
    });

    let errors = findings
        .iter()
        .filter(|finding| finding.severity == FindingSeverity::Error)
        .count();
    let warnings = findings
        .iter()
        .filter(|finding| finding.severity == FindingSeverity::Warning)
        .count();

    DoctorResult {
        root: roots.requested_root.display().to_string(),
        index_root: roots.index_root.display().to_string(),
        using_parent_index: roots.using_parent_index,
        healthy: errors == 0 && warnings == 0,
        errors,
        warnings,
        findings: findings
            .into_iter()
            .map(|finding| DoctorFinding {
                id: finding.id.to_string(),
                severity: finding.severity.as_str(),
                message: finding.message,
                recommendation: finding.recommendation,
            })
            .collect(),
    }
}

fn load_metadata(path: &Path) -> Result<Option<MetadataSnapshot>> {
    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(path)
        .with_context(|| format!("Failed to read metadata file: {}", path.display()))?;
    let parsed: MetadataSnapshot = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse metadata file: {}", path.display()))?;
    Ok(Some(parsed))
}

fn load_manifest_json(path: &Path) -> Result<()> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("Failed to read manifest file: {}", path.display()))?;
    let _: serde_json::Value = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse manifest file: {}", path.display()))?;
    Ok(())
}

fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn file_modified_ms(path: &Path) -> Option<u64> {
    fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| duration.as_millis().try_into().ok())
}

fn inspect_index(index_dir: &Path) -> Result<(&'static str, bool, DocCounts)> {
    let index = Index::open_in_dir(index_dir)
        .with_context(|| format!("Failed to open index at {}", index_dir.display()))?;
    let schema = index.schema();

    let has_path_exact = schema.get_field("path_exact").is_ok();
    let has_doc_type = schema.get_field("doc_type").is_ok();
    let has_symbol_id = schema.get_field("symbol_id").is_ok();
    let has_symbol_end_line = schema.get_field("symbol_end_line").is_ok();
    let schema_ok = has_path_exact && has_doc_type && has_symbol_id && has_symbol_end_line;

    let schema_version = if schema_ok {
        INDEX_SCHEMA_SYMBOL_V1
    } else {
        INDEX_SCHEMA_LEGACY
    };

    let docs = collect_doc_counts(&index).unwrap_or_default();

    Ok((schema_version, schema_ok, docs))
}

fn collect_doc_counts(index: &Index) -> Result<DocCounts> {
    let schema = index.schema();
    let doc_type_field = schema
        .get_field("doc_type")
        .context("doc_type field missing in schema")?;

    let reader = index.reader().context("failed to create index reader")?;
    let searcher = reader.searcher();

    let total = searcher.search(&AllQuery, &Count)? as u64;
    let file_term = Term::from_field_text(doc_type_field, "file");
    let symbol_term = Term::from_field_text(doc_type_field, "symbol");

    let file_docs =
        searcher.search(&TermQuery::new(file_term, IndexRecordOption::Basic), &Count)? as u64;
    let symbol_docs = searcher.search(
        &TermQuery::new(symbol_term, IndexRecordOption::Basic),
        &Count,
    )? as u64;

    Ok(DocCounts {
        total_docs: Some(total),
        file_docs: Some(file_docs),
        symbol_docs: Some(symbol_docs),
    })
}

fn collect_watch_status(index_root: &Path) -> StatusWatch {
    let pid_path = index_root.join(WATCH_PID_FILE_REL);
    let log_path = index_root.join(WATCH_LOG_FILE_REL);

    let pid_file_exists = pid_path.exists();
    let log_file_exists = log_path.exists();

    if !pid_file_exists {
        return StatusWatch {
            status: "stopped".to_string(),
            pid: None,
            stale_pid: false,
            pid_file_exists,
            log_file_exists,
        };
    }

    let raw = match fs::read_to_string(&pid_path) {
        Ok(raw) => raw,
        Err(_) => {
            return StatusWatch {
                status: "unavailable".to_string(),
                pid: None,
                stale_pid: true,
                pid_file_exists,
                log_file_exists,
            };
        }
    };

    let pid = raw.trim().parse::<u32>().ok();
    let alive = pid.map(process_alive).unwrap_or(false);

    if alive {
        StatusWatch {
            status: "running".to_string(),
            pid,
            stale_pid: false,
            pid_file_exists,
            log_file_exists,
        }
    } else {
        StatusWatch {
            status: "stale".to_string(),
            pid,
            stale_pid: true,
            pid_file_exists,
            log_file_exists,
        }
    }
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn process_alive(_pid: u32) -> bool {
    false
}

fn collect_tmp_files(index_dir: &Path) -> Vec<String> {
    let mut hits: Vec<String> = Vec::new();
    if !index_dir.exists() {
        return hits;
    }

    let mut stack = vec![index_dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        let Ok(read_dir) = fs::read_dir(&path) else {
            continue;
        };

        for entry in read_dir.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                stack.push(entry_path);
                continue;
            }

            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') && name.contains(".tmp-") {
                hits.push(entry.path().display().to_string());
            }
        }
    }

    hits.sort();
    hits
}

fn print_status_text(result: &StatusResult) {
    println!("Root: {}", result.root);
    if result.using_parent_index {
        println!("Index root: {} (parent)", result.index_root);
    } else {
        println!("Index root: {}", result.index_root);
    }

    println!(
        "Index: exists={} tantivy_meta={} metadata={} schema={} schema_ok={}",
        result.index.exists,
        result.index.tantivy_meta_exists,
        result.index.metadata_exists,
        result.index.schema_version.as_deref().unwrap_or("unknown"),
        result.index.schema_ok
    );
    println!(
        "Docs: total={} file={} symbol={}",
        fmt_opt_u64(result.index.docs.total_docs),
        fmt_opt_u64(result.index.docs.file_docs),
        fmt_opt_u64(result.index.docs.symbol_docs)
    );
    println!(
        "Tracked files: {}",
        result
            .index
            .tracked_files
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    );

    println!(
        "Manifest: version={} v1={} root_hash={}",
        result.manifest.version.as_deref().unwrap_or("unknown"),
        result.manifest.has_v1_snapshot,
        result.manifest.has_root_hash
    );
    println!(
        "Last diff: added={} modified={} deleted={} unchanged={} scanned={} suspects={} hashed={}",
        fmt_opt_usize(result.manifest.last_diff.added),
        fmt_opt_usize(result.manifest.last_diff.modified),
        fmt_opt_usize(result.manifest.last_diff.deleted),
        fmt_opt_usize(result.manifest.last_diff.unchanged),
        fmt_opt_usize(result.manifest.last_diff.scanned),
        fmt_opt_usize(result.manifest.last_diff.suspects),
        fmt_opt_usize(result.manifest.last_diff.hashed)
    );

    println!(
        "Readiness: basic_ready={} full_ready={} last_build_time_ms={}",
        result.readiness.basic_ready,
        result.readiness.full_ready,
        fmt_opt_u64(result.readiness.last_build_time_ms)
    );

    println!(
        "Watch daemon: status={} pid={} stale_pid={} pid_file={} log_file={}",
        result.watch.status,
        result
            .watch
            .pid
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        result.watch.stale_pid,
        result.watch.pid_file_exists,
        result.watch.log_file_exists,
    );
}

fn print_stats_text(result: &StatsResult) {
    println!("Root: {}", result.root);
    if result.using_parent_index {
        println!("Index root: {} (parent)", result.index_root);
    } else {
        println!("Index root: {}", result.index_root);
    }

    println!(
        "Stats file: exists={} schema={}",
        result.stats_file_exists,
        result.stats_schema_version.as_deref().unwrap_or("unknown")
    );

    if let Some(run) = result.last_run.as_ref() {
        println!(
            "Last run: mode={} force={} total_ms={} indexed={} skipped={} deleted={} errors={}",
            run.mode,
            run.force,
            run.total_ms,
            run.indexed_files,
            run.skipped_files,
            run.deleted_files,
            run.error_files
        );
        println!(
            "Timings(ms): scan={} hash={} parse={} index={} commit={}",
            fmt_opt_u64(run.timings_ms.scan_ms),
            fmt_opt_u64(run.timings_ms.hash_ms),
            fmt_opt_u64(run.timings_ms.parse_ms),
            fmt_opt_u64(run.timings_ms.index_ms),
            fmt_opt_u64(run.timings_ms.commit_ms)
        );
    } else {
        println!("Last run: unknown");
        println!(
            "Timings(ms): scan=unknown hash=unknown parse=unknown index=unknown commit=unknown"
        );
    }

    println!(
        "Diff: added={} modified={} deleted={} unchanged={} scanned={} suspects={} hashed={}",
        fmt_opt_usize(result.diff_counts.added),
        fmt_opt_usize(result.diff_counts.modified),
        fmt_opt_usize(result.diff_counts.deleted),
        fmt_opt_usize(result.diff_counts.unchanged),
        fmt_opt_usize(result.diff_counts.scanned),
        fmt_opt_usize(result.diff_counts.suspects),
        fmt_opt_usize(result.diff_counts.hashed)
    );
    println!(
        "Cache reuse: hit={} miss={}",
        fmt_opt_u64(result.cache_reuse.hit),
        fmt_opt_u64(result.cache_reuse.miss)
    );
}

fn print_doctor_text(result: &DoctorResult) {
    println!("Root: {}", result.root);
    if result.using_parent_index {
        println!("Index root: {} (parent)", result.index_root);
    } else {
        println!("Index root: {}", result.index_root);
    }

    println!(
        "Doctor: healthy={} errors={} warnings={}",
        result.healthy, result.errors, result.warnings
    );

    if result.findings.is_empty() {
        println!("No issues found.");
        return;
    }

    for finding in &result.findings {
        println!(
            "- [{}] {}: {}",
            finding.severity.to_uppercase(),
            finding.id,
            finding.message
        );
        println!("  recommendation: {}", finding.recommendation);
    }
}

fn fmt_opt_u64(value: Option<u64>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn fmt_opt_usize(value: Option<usize>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_counts_from_manifest_sets_all_fields() {
        let summary = ManifestDiffSummary {
            added: vec!["a.rs".to_string(), "b.rs".to_string()],
            modified: vec!["c.rs".to_string()],
            deleted: vec![],
            unchanged: 7,
            scanned: 10,
            suspects: 3,
            hashed: 3,
        };

        let counts = DiffCounts::from_manifest(&summary);
        assert_eq!(counts.added, Some(2));
        assert_eq!(counts.modified, Some(1));
        assert_eq!(counts.deleted, Some(0));
        assert_eq!(counts.unchanged, Some(7));
        assert_eq!(counts.scanned, Some(10));
        assert_eq!(counts.suspects, Some(3));
        assert_eq!(counts.hashed, Some(3));
    }

    #[test]
    fn persist_last_run_replaces_previous_state_atomically() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let root = dir.path();

        persist_last_run(
            root,
            LastRunStats {
                mode: "full".to_string(),
                total_ms: 100,
                ..LastRunStats::default()
            },
        )
        .expect("persist first");

        persist_last_run(
            root,
            LastRunStats {
                mode: "incremental".to_string(),
                total_ms: 55,
                ..LastRunStats::default()
            },
        )
        .expect("persist second");

        let state = load_stats(root).expect("load stats");
        let last = state.last_run.expect("last run");
        assert_eq!(last.mode, "incremental");
        assert_eq!(last.total_ms, 55);

        let cgrep_dir = root.join(INDEX_DIR);
        if cgrep_dir.exists() {
            for entry in fs::read_dir(cgrep_dir).expect("read dir") {
                let entry = entry.expect("entry");
                let name = entry.file_name();
                let name = name.to_string_lossy();
                assert!(!name.contains(".tmp-"), "unexpected tmp file: {name}");
            }
        }
    }
}
