# cgrep Docs

Local-first code search for developers and AI coding agents.

Current release: **v1.5.2**

## Start Here (2 Minutes)

```bash
# Install
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh | bash

# Search + navigate
cgrep s "token validation" src/
cgrep d handle_auth
cgrep read src/auth.rs
```

## By Goal

| Goal | Open this page |
|---|---|
| Install quickly | [Installation](./installation.md) |
| Learn daily commands | [Usage](./usage.md) |
| Set up AI-agent retrieval | [Agent Workflow](./agent.md) |
| Connect MCP hosts | [MCP](./mcp.md) |
| Keep index warm while coding | [Indexing and Daemon](./indexing-watch.md) |
| Fix common issues | [Troubleshooting](./troubleshooting.md) |

## AI Agent Setup (Required vs Optional)

```bash
# One-time install (choose one)
cgrep agent install codex
cgrep agent install claude-code
cgrep agent install cursor
cgrep agent install copilot
cgrep agent install opencode
```

- Required: restart the agent session once.
- Not required for normal usage: manual `cgrep index` or `cgrep daemon start`.
- Optional CLI retrieval examples are in [agent.md](./agent.md).

## Benchmark Snapshot (PyTorch, Codex, runs=2)

- Date: **February 22, 2026 (UTC)**
- Baseline billable tokens: **151,466**
- cgrep billable tokens: **69,874**
- Billable token reduction: **53.9%**

Reports:
- [Codex Agent Efficiency](./benchmarks/pytorch-codex-agent-efficiency.md)
- [Search Option Performance](./benchmarks/pytorch-search-options-performance.md)
- [Agent Token Efficiency](./benchmarks/pytorch-agent-token-efficiency.md)

## Language

- Korean hub: [ko/index.md](./ko/index.md)
- Repository README (EN/KO/中文): [README.md](https://github.com/meghendra6/cgrep/blob/main/README.md)
