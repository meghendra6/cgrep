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
| Keep agent loops small | noisy context payloads | `agent plan` or `agent locate -> agent expand` |
| Maintain stable retrieval in large repos | ad-hoc scripts | index/watch/daemon + MCP server |

## 60-Second Quick Start

### For Users

```bash
# 1) Install (release binary)
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh | bash

# 2) Build index once per repo (respects .gitignore/.ignore by default)
cgrep index
# or non-blocking background build
cgrep index --background
# or warm-start from local compatible cache snapshots
cgrep index --reuse strict
cgrep index --reuse auto

# 3) Search and navigate
cgrep s "token validation" src/
cgrep s "auth middleware" -P user
cgrep --format json2 --compact search "target_fn" --explain
cgrep d handle_auth
cgrep r UserService
cgrep read src/auth.rs
cgrep status
```

### For AI Agents

```bash
# Install agent guidance (pick your host)
cgrep agent install codex
cgrep agent install claude-code
cgrep agent install copilot
cgrep agent install cursor
cgrep agent install opencode

# Or install MCP server config directly
cgrep mcp install claude-code
cgrep mcp install cursor
cgrep mcp install windsurf
cgrep mcp install vscode
cgrep mcp install claude-desktop

# Token-efficient retrieval
ID=$(cgrep agent locate "where token validation happens" --compact | jq -r '.results[0].id')
cgrep agent expand --id "$ID" -C 8 --compact

# Deterministic retrieval plan
cgrep --format json2 --compact agent plan "trace authentication middleware flow"
# AI-agent preset aliases also work in search mode
cgrep search "trace auth flow" -P ai -B tight --format json2 --compact
```

Validate end-to-end (core/incremental/agent/status/docs):

```bash
CGREP_BIN=cgrep bash scripts/validate_all.sh
```

Details:
- Agent setup guide: <https://meghendra6.github.io/cgrep/agent/>
- MCP integration guide: <https://meghendra6.github.io/cgrep/mcp/>

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
- Empty/whitespace queries are rejected in all modes (including `--regex`).
- If query text starts with `-`, pass `--` after `search`.
  Example: `cgrep search -- --literal`
- If you also pass scope/options, place them before `--`.
  Example: `cgrep search -p src -- --help`
- Direct shorthand `cgrep "query"` is intentionally not used.
- `--explain` is supported for keyword mode and emits deterministic score components.
- `--profile` accepts aliases: `user/developer -> human`, `ai/ai-agent/coding-agent -> agent`, `quick -> fast`.
- `cgrep read` requires a non-empty path argument.
- `search` result `path` is always reusable:
  workspace-internal scopes return workspace-relative paths, and external scopes return absolute paths.
- `json2`/`--compact` deterministic contract and tie-break rules are documented in `docs/usage.md`.
- `agent plan` now includes bounded `read` follow-up steps for top candidates, improving verification loops.

For MCP usage:
- Codex setup uses `cgrep agent install codex` (not `cgrep mcp install codex`).
- `cgrep mcp install` host values: `claude-code`, `cursor`, `windsurf`, `vscode`, `claude-desktop`.
- `cgrep_search` treats dash-prefixed queries as literal text automatically.
- Pass optional `cwd` in MCP tool arguments to pin relative-path resolution.
- After `cgrep agent install codex`, restart the current Codex session so updated MCP config is reloaded.
- MCP install writes `command = "cgrep"` by default, so binary updates apply without reinstalling MCP config.

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
cgrep status
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
cgrep a p "task"      # agent plan
```

## Benchmarks

PyTorch scenario-completion benchmark snapshots:
- Agent token-efficiency benchmark: `docs/benchmarks/pytorch-agent-token-efficiency.md`
- Codex real-agent benchmark: `docs/benchmarks/pytorch-codex-agent-efficiency.md`
- Latest Codex snapshot (`2026-02-18`, `gpt-5-codex`, `runs=1`):
  baseline `89,764` -> cgrep `21,092` billable tokens (`76.5%` reduction).

## Documentation

- Docs site: <https://meghendra6.github.io/cgrep/>
- Docs hub: `docs/index.md`
- Korean docs: `docs/ko/index.md`
- Installation: `docs/installation.md`
- Usage: `docs/usage.md`
- Agent workflow: `docs/agent.md`
- MCP integration: `docs/mcp.md`
- Indexing/watch/daemon: `docs/indexing-watch.md`
- Operations: `docs/operations.md`
- Configuration: `docs/configuration.md`
- Embeddings mode: `docs/embeddings.md`
- Troubleshooting: `docs/troubleshooting.md`
- Development: `docs/development.md`

## Project Notes

- Current release: **v1.5.1**
- Changelog: `CHANGELOG.md`
- Comparison material: `COMPARISON.md`
- Contributing guide: `CONTRIBUTING.md`
- Security policy: `SECURITY.md`
- Code of conduct: `CODE_OF_CONDUCT.md`
