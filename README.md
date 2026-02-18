# cgrep

Code search for humans and AI coding agents.

`grep` finds lines. `cgrep` finds code intent.

- Local-first: no cloud index required
- Code-aware navigation: `definition`, `references`, `callers`, `dependents`, `map`, `read`
- Agent-ready output: deterministic `json2` + compact mode
- Proven efficiency on PyTorch retrieval workflows (large token and latency reductions)

## Why It Stands Out

| Problem | Typical flow | cgrep flow |
|---|---|---|
| Locate real implementation points | repeat grep + manual file opens | `search -> definition/references -> read` |
| Keep agent loops small | noisy context payloads | `agent locate -> agent expand` |
| Maintain stable retrieval in large repos | ad-hoc scripts | index/watch/daemon + MCP server |

## 60-Second Quick Start

### For Users

```bash
# 1) Install (release binary)
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh | bash

# 2) Build index once per repo
cgrep index

# 3) Search and navigate
cgrep s "token validation" src/
cgrep d handle_auth
cgrep r UserService
cgrep read src/auth.rs
```

### For AI Agents

```bash
# Install agent guidance + MCP wiring
cgrep agent install codex

# Token-efficient retrieval
ID=$(cgrep agent locate "where token validation happens" --compact | jq -r '.results[0].id')
cgrep agent expand --id "$ID" -C 8 --compact
```

## Search UX (grep-friendly, explicit)

Use explicit search entrypoints:
- `cgrep search "query" [path]`
- `cgrep s "query" [path]`

Common grep-style options are supported:
- `-r/--recursive`, `--no-recursive`
- `--include`, `--exclude-dir`
- `--no-ignore`
- `-i/--ignore-case`, `--case-sensitive`

Notes:
- If query text starts with `-`, pass `--` after `search`.
  Example: `cgrep search -- --literal`
- Direct shorthand `cgrep "query"` is intentionally not used.
- `search` result `path` is always reusable:
  workspace-internal scopes return workspace-relative paths, and external scopes return absolute paths.

For MCP usage:
- `cgrep_search` treats dash-prefixed queries as literal text automatically.
- Pass optional `cwd` in MCP tool arguments to pin relative-path resolution.
- If MCP server cwd is `/`, relative scopes are rejected unless `cwd` is provided (or an absolute `path` is used).

## Core Commands

```bash
cgrep search "authentication flow" src/
cgrep symbols UserService
cgrep definition handleAuth
cgrep callers validateToken
cgrep references UserService
cgrep dependents src/auth.rs
cgrep read src/auth.rs
cgrep map --depth 2
```

Shortcut aliases:

```bash
cgrep s "query"      # search
cgrep d name          # definition
cgrep r name          # references
cgrep c function      # callers
cgrep dep file        # dependents
cgrep i               # index
cgrep a l "query"     # agent locate
```

## Benchmarks

PyTorch scenario-completion benchmark snapshots:
- Agent token-efficiency benchmark: `docs/benchmarks/pytorch-agent-token-efficiency.md`
- Codex real-agent benchmark: `docs/benchmarks/pytorch-codex-agent-efficiency.md`

## Documentation

- Docs hub: `docs/index.md`
- Korean docs: `docs/ko/index.md`
- Installation: `docs/installation.md`
- Usage: `docs/usage.md`
- Agent workflow: `docs/agent.md`
- MCP integration: `docs/mcp.md`
- Indexing/watch/daemon: `docs/indexing-watch.md`
- Configuration: `docs/configuration.md`
- Embeddings mode: `docs/embeddings.md`
- Troubleshooting: `docs/troubleshooting.md`
- Development: `docs/development.md`

## Project Notes

- Current release: **v1.4.6**
- Changelog: `CHANGELOG.md`
- Comparison material: `COMPARISON.md`
