# cgrep (English)

[English](./README.en.md) | [한국어](./README.ko.md) | [中文](./README.zh.md)

Local-first code search for humans and AI coding agents.

`grep` finds text. `cgrep` finds implementation intent.

## Why cgrep

- Fast local search with Tantivy index (no cloud dependency)
- Code-aware navigation: `definition`, `references`, `callers`, `read`, `map`
- Agent-friendly deterministic output: `--format json2 --compact`
- MCP integration for Codex/Claude/Cursor/VS Code hosts

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

```bash
# Install agent guidance + MCP wiring (Codex)
cgrep agent install codex

# Low-token two-stage retrieval
ID=$(cgrep agent locate "where token validation happens" --compact | jq -r '.results[0].id')
cgrep agent expand --id "$ID" -C 8 --compact

# Deterministic plan output
cgrep --format json2 --compact agent plan "trace authentication middleware flow"
```

## Indexing Modes (Simple Rule)

- One-off usage: just run `search/definition/read` (auto bootstrap handles indexing)
- Active coding session: `cgrep daemon start` and stop with `cgrep daemon stop`
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
