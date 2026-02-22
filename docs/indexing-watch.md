# Indexing and Daemon

## Which Mode Should I Use?

| Scenario | Recommended |
|---|---|
| Quick one-off searches | run `search/read/definition` directly (auto bootstrap) |
| Active coding session | `cgrep daemon start` while coding, then `cgrep daemon stop` |
| One-time async prebuild | `cgrep index --background` |
| Semantic/hybrid experiments | `cgrep index --embeddings precompute` (experimental) |

## Core Commands

```bash
# Build or refresh index
cgrep index

# Force full rebuild
cgrep index --force

# One-shot background build
cgrep index --background

# Daemon lifecycle
cgrep daemon start
cgrep daemon status
cgrep daemon stop

# Read readiness quickly
cgrep status
```

## Large Repository Tips

```bash
# Lower pressure settings for high-churn repos
cgrep daemon start --debounce 30 --min-interval 180 --max-batch-delay 240
```

Defaults are already tuned for background operation; adjust only if needed.

## Notes

- Index files are stored in `.cgrep/`.
- Ignore files (`.gitignore`, `.ignore`) are respected by default.
- `--include-ignored` disables ignore filtering.
- `--include-path <path>` lets you include selected ignored paths.
- Daemon is event-driven; without file changes it stays idle.
