# Usage

## Core Commands

| Command | Purpose |
|---|---|
| `cgrep s "query" [path]` | text/code search |
| `cgrep d <symbol>` | definition lookup |
| `cgrep r <symbol>` | references lookup |
| `cgrep c <function>` | caller lookup |
| `cgrep symbols <name>` | symbol search |
| `cgrep read <file>` | smart file read |
| `cgrep map --depth 2` | quick codebase map |
| `cgrep dep <file>` | reverse dependents |
| `cgrep status` | index + daemon status |

## Daily Workflow

```bash
# 1) Find candidate files
cgrep s "authentication middleware" src/

# 2) Jump to implementation
cgrep d handle_auth

# 3) Check usage impact
cgrep r handle_auth
cgrep c handle_auth

# 4) Read focused context
cgrep read src/auth.rs
```

## Search Scoping (Most Important)

Use scope early to reduce noise and tokens.

```bash
# Path scope
cgrep s "DispatchKeySet" -p c10/core

# File type scope
cgrep s "token refresh" -t rust

# Changed files only (default revision: HEAD)
cgrep s "retry" -u

# Context lines
cgrep s "evaluate_function" -C 2

# Result limit
cgrep s "TensorIterator" -m 10
```

## Agent-Friendly Output

```bash
# Deterministic compact payload
cgrep --format json2 --compact s "PythonArgParser" -p torch/csrc/utils

# Score explain (keyword mode)
cgrep --format json2 --compact s "target_fn" --explain
```

## Profiles and Budgets

```bash
# Human-friendly defaults
cgrep s "auth flow" -P human

# Agent-focused payload control
cgrep s "auth flow" -P agent -B tight --format json2 --compact
```

## Indexing Behavior (Simple)

- `search/read/definition/...` commands can auto-bootstrap index if missing.
- You can still prebuild manually with `cgrep index`.
- For continuous session updates, run `cgrep daemon start` and stop with `cgrep daemon stop`.

## Important Notes

- Empty queries are rejected.
- If query starts with `-`, pass `--`:

```bash
cgrep s -- --help
```

- `semantic` and `hybrid` modes are experimental and require embeddings index.

## Next

- Agent flow: [agent.md](./agent.md)
- MCP integration: [mcp.md](./mcp.md)
- Indexing/daemon operation: [indexing-watch.md](./indexing-watch.md)
- Troubleshooting: [troubleshooting.md](./troubleshooting.md)
