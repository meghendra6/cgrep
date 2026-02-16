# cgrep Documentation

`grep` finds text. `cgrep` finds code intent.

Local-first code search for humans and AI agents working in real repositories.
Current release: **v1.4.3**.

v1.4.3 highlights:
- Shortcut-first CLI for high-frequency commands.
- Agent install support for `claude-code`, `codex`, `copilot`, `cursor`, and `opencode`.
- MCP host install support including Cursor (`cgrep mcp install cursor`).

- Docs site: <https://meghendra6.github.io/cgrep/>
- Repository README: [README.md](https://github.com/meghendra6/cgrep/blob/main/README.md)
- Korean docs: [ko/index.md](./ko/index.md)

## Why cgrep

- Built for AI coding loops: compact, deterministic `json2` output and two-stage `agent locate/expand`.
- Code-aware navigation: `definition`, `references`, `callers`, `dependents`, `map`, and `read`.
- Ergonomic CLI: short aliases like `s`, `d`, `r`, `c`, `dep`, `i`, `a l`.
- Local-first operations: fast retrieval, private code, no cloud dependency.

## Benchmark Snapshot (PyTorch)

Measured on February 14, 2026 across 6 implementation-tracing scenarios.
Completion model: iterative retrieval until each scenario completion rule is satisfied.

| Metric | Baseline (`grep`) | cgrep (`agent locate/expand`) | Improvement |
|---|---:|---:|---:|
| Total tokens-to-complete | 127,665 | 6,153 | **95.2% less** |
| Avg tokens-to-complete per task | 21,278 | 1,026 | **20.75x smaller** |
| Avg retrieval latency to completion | 1,321.3 ms | 22.7 ms | **~58.2x faster** |

Details: [Benchmark: Agent Token Efficiency](./benchmarks/pytorch-agent-token-efficiency.md)

## Start Here

| Section | Description |
|---|---|
| [Installation](./installation.md) | Install and first run |
| [Usage](./usage.md) | CLI commands and search options |
| [Agent Workflow](./agent.md) | Two-stage `locate` / `expand` flow |
| [MCP](./mcp.md) | MCP server mode and harness usage |
| [Indexing and Watch](./indexing-watch.md) | Indexing, watch, and daemon operations |
| [Configuration](./configuration.md) | `.cgreprc.toml` and config precedence |
| [Embeddings](./embeddings.md) | Semantic/hybrid mode setup and tuning |
| [Benchmark: Agent Token Efficiency](./benchmarks/pytorch-agent-token-efficiency.md) | AI coding workflow token reduction benchmark on PyTorch (`grep` baseline) |
| [Troubleshooting](./troubleshooting.md) | Common issues and fixes |
| [Development](./development.md) | Build, test, and validation commands |

## Quick Links

- Changelog: [CHANGELOG.md](https://github.com/meghendra6/cgrep/blob/main/CHANGELOG.md)
- Comparison: [COMPARISON.md](https://github.com/meghendra6/cgrep/blob/main/COMPARISON.md)
- Harness rationale: <https://blog.can.ac/2026/02/12/the-harness-problem/>
