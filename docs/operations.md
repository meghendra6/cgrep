# Operations

## Scope

This guide covers day-2 operations: readiness checks, background indexing, reuse diagnostics, and safe manual cleanup.

## Runtime Artifacts Under `.cgrep/`

- `.cgrep/index/`: Tantivy search index files.
- `.cgrep/symbols/`: extracted symbol artifacts.
- `.cgrep/manifest/`: manifest metadata (`version`, `v1.json`, optional `root.hash`).
- `.cgrep/metadata.json`: persisted index profile and incremental metadata.
- `.cgrep/status.json`: basic/full readiness and background build progress.
- `.cgrep/reuse-state.json`: last reuse decision and fallback reason.
- `.cgrep/watch.pid`, `.cgrep/watch.log`: watch daemon process and log files.
- `.cgrep/background-index.log`: background index worker log.

## Readiness, Status, and Search Stats

```bash
# human-friendly
cgrep status

# deterministic machine payload
cgrep --format json2 --compact status

# search payload includes request-level stats in meta
cgrep --format json2 --compact search "sanity check" -m 5
```

`status` reports:
- readiness: `basic_ready`, `full_ready`
- build phase and counters: `phase`, `progress.total|processed|failed`
- daemon state: `running|stale`, `pid`, `pid_file`, `log_file`
- reuse diagnostics when available: `decision`, `source`, `snapshot_key`, `reason`

Search `json2.meta` reports request stats:
- `elapsed_ms`
- `files_with_matches`
- `total_matches`
- payload budget counters (`payload_chars`, `payload_tokens_estimate`)

## Background Indexing Lifecycle

```bash
# start full build asynchronously
cgrep index --background

# monitor state
cgrep --format json2 --compact status

# foreground watch daemon (managed process)
cgrep daemon start
cgrep daemon status
cgrep daemon stop
```

Behavior guarantees:
- background mode is opt-in (`--background`); default index flow is unchanged.
- status writes are atomic and interruption-safe.
- stale background pid state is recovered by status checks.

## Reuse Diagnostics and Migration Notes

New additive indexing flags:
- `cgrep index --background`
- `cgrep index --reuse off|strict|auto` (default is `off`)
- `cgrep index --manifest-only`
- `cgrep index --print-diff`
- `cgrep index --no-manifest`

Migration guidance:
- existing scripts keep baseline behavior with `--reuse off` (default).
- to explicitly pin baseline behavior in automation, pass `--reuse off`.
- enabling reuse creates/updates `.cgrep/reuse-state.json`; consumers should treat this as optional.
- status `reuse` fields are optional and may be absent when reuse is not attempted.

## Safe Cleanup (Manual Only)

No destructive cleanup runs automatically.

If cleanup is required, perform it manually and intentionally:

```bash
# remove local project index artifacts only
rm -rf .cgrep/index .cgrep/symbols .cgrep/manifest

# clear reuse diagnostics only
rm -f .cgrep/reuse-state.json
```

For shared cache snapshots (`~/.cache/cgrep/indexes/` on macOS/Linux, `%LOCALAPPDATA%/cgrep/indexes/` on Windows), remove directories manually only after confirming they are unused by other worktrees.

## Doctor Flow

Repository health sanity check:

```bash
bash scripts/doctor.sh .
```

This check is non-destructive and reports missing integration files/settings.
