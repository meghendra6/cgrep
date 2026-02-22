# Indexing And Daemon

## Indexing

```bash
# Rebuild index
cgrep index --force

# Exclude paths while indexing
cgrep index -e vendor/ -e dist/

# Include a specific ignored path while keeping ignore rules on
cgrep index -e graph_mode -e eager_mode/torch-rbln/third_party/pytorch --include-path .venv --high-memory

# Include all ignored paths (opt-out)
cgrep index --include-ignored

# Embeddings mode
cgrep index --embeddings auto
cgrep index --embeddings precompute

# Manifest controls (incremental path)
cgrep index --print-diff
cgrep index --manifest-only --print-diff
cgrep index --no-manifest

# Build full index in background and return immediately
cgrep index --background

# Reuse compatible local cache snapshots
cgrep index --reuse strict
cgrep index --reuse auto
cgrep index --reuse off
```

## Daemon

```bash
# Daemon mode (managed background indexing)
cgrep daemon start
cgrep daemon status
cgrep daemon stop

# Short alias form
cgrep bg up
cgrep bg st
cgrep bg down
```

## Choosing Between `index --background` and `daemon`

- `cgrep index --background`:
  - one-shot async build
  - worker exits after the build
  - does not keep watching files
- `cgrep daemon start`:
  - long-running managed process
  - keeps tracking file changes and applies incremental reindex updates
  - stop explicitly with `cgrep daemon stop`

Large-repo low-pressure example:

```bash
cgrep daemon start --debounce 30 --min-interval 180 --max-batch-delay 240
```

Disable adaptive mode (fixed timing behavior):

```bash
cgrep daemon start --no-adaptive
```

## Behavior notes

- Index lives under `.cgrep/`
- Manifest lives under `.cgrep/manifest/` (`version`, `v1.json`, optional `root.hash`)
- Search from subdirectories reuses nearest parent index
- Search/symbol-style commands auto-bootstrap and call-driven refresh the index by default; daemon is optional for always-hot indexing.
- Indexing respects `.gitignore`/`.ignore` by default (`--include-ignored` to opt out)
- `--include-path <path>` lets you index selected ignored paths without indexing everything ignored
- Default indexing uses a two-stage change detector:
  - stage1 quick filter by file `mtime` + `size`
  - stage2 BLAKE3 hash only for suspected changes
- `--manifest-only` updates manifest + diff summary in `.cgrep/metadata.json` without document reindex
- `--no-manifest` disables manifest usage and falls back to legacy incremental behavior
- Reuse cache root:
  - macOS/Linux: `~/.cache/cgrep/indexes/`
  - Windows: `%LOCALAPPDATA%/cgrep/indexes/`
- Reuse snapshot layout:
  - `repo_key/snapshot_key/tantivy/`
  - `repo_key/snapshot_key/symbols/`
  - `repo_key/snapshot_key/manifest/`
  - `repo_key/snapshot_key/metadata.json`
- Reuse safety:
  - stale/nonexistent files are filtered while reuse is active
  - incompatible/corrupt snapshots fall back to normal indexing
- `status` reads `.cgrep/status.json` and reports deterministic readiness/progress fields
- Daemon uses adaptive backoff by default (`--no-adaptive` to disable)
- Daemon defaults are tuned for background operation (`--min-interval 180`, about 3 minutes)
- Daemon reacts only to indexable source extensions and skips temp/swap files
- Daemon reuses the most recent index profile from `.cgrep/metadata.json`
- Reused profile preserves the latest `cgrep index` options as-is
- Daemon reindex is changed-path incremental (update/remove touched files only)
- For high-churn batches (for example large branch switches), daemon automatically switches to bulk incremental refresh to reduce event/memory overhead.
- Bulk switch threshold is auto-sized from indexed files (about 25%), clamped to `1500..12000`.

## Daemon defaults

| Option | Default | Purpose |
|---|---:|---|
| `--debounce` | `15` | wait for event bursts to settle |
| `--min-interval` | `180` | minimum interval between reindex runs |
| `--max-batch-delay` | `180` | force a run if events keep streaming |
| adaptive mode | `on` | auto-backoff based on change volume/reindex cost |

## Managed daemon lifecycle

```bash
cgrep daemon start
cgrep daemon status
cgrep daemon stop
```

For very large repositories, prefer:
- `--min-interval 180` or higher
- `--debounce 30` or higher
- keeping adaptive mode enabled (default)

See also:
- operations runbook: `docs/operations.md`
