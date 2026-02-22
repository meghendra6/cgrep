# cgrep

[English](./README.en.md) | [한국어](./README.ko.md) | [中文](./README.zh.md)

Local-first code search for humans and AI coding agents.

`grep` tells you where text appears. `cgrep` helps you find where behavior is implemented.

## Why cgrep

- Fast local search with Tantivy index (no cloud dependency)
- Code-aware navigation: `definition`, `references`, `callers`, `read`, `map`
- Agent-friendly deterministic output: `--format json2 --compact`
- MCP integration for Codex, Claude Code, Cursor, Copilot, and more

## Install in 30 seconds

```bash
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh | bash
cgrep --help
```

## Start in 2 minutes

```bash
# Optional warm-up
cgrep index

# Daily workflow
cgrep s "token validation" src/
cgrep d handle_auth
cgrep r UserService
cgrep read src/auth.rs
cgrep map --depth 2
```

## For AI Coding Agents

### 1) One-time install (choose your host)

```bash
cgrep agent install codex
cgrep agent install claude-code
cgrep agent install cursor
cgrep agent install copilot
cgrep agent install opencode
```

### 2) What is required vs optional

- Required: restart the current agent session once after install.
- Not required for normal use: manual `cgrep index` or `cgrep daemon start`.
- Optional: run daemon during long, high-churn coding sessions to keep index warm.

### Optional CLI retrieval examples

```bash
ID=$(cgrep agent locate "where token validation happens" --compact | jq -r '.results[0].id')
cgrep agent expand --id "$ID" -C 8 --compact
cgrep --format json2 --compact agent plan "trace authentication middleware flow"
```

## Indexing Modes (Simple Rule)

- One-off usage: run `search/definition/read` directly (auto bootstrap handles indexing)
- Active coding session: `cgrep daemon start`, then `cgrep daemon stop`
- Semantic/hybrid search: experimental, requires embeddings index

## Benchmark Snapshot (PyTorch, Codex, runs=2)

- Date: **February 22, 2026 (UTC)**
- Baseline billable tokens: **151,466**
- cgrep billable tokens: **69,874**
- Billable token reduction: **53.9%**

Full report: [`docs/benchmarks/pytorch-codex-agent-efficiency.md`](./docs/benchmarks/pytorch-codex-agent-efficiency.md)

## Documentation

- Docs site: <https://meghendra6.github.io/cgrep/>
- Quick install: [`docs/installation.md`](./docs/installation.md)
- Usage: [`docs/usage.md`](./docs/usage.md)
- Agent workflow: [`docs/agent.md`](./docs/agent.md)
- MCP: [`docs/mcp.md`](./docs/mcp.md)
- Indexing/daemon: [`docs/indexing-watch.md`](./docs/indexing-watch.md)
- Troubleshooting: [`docs/troubleshooting.md`](./docs/troubleshooting.md)

## Release

- Current version: **v1.5.2**
- Changelog: [`CHANGELOG.md`](./CHANGELOG.md)
