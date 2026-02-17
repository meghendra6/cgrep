# cgrep Documentation

`grep` finds text. `cgrep` finds code intent.

Local-first code search for humans and AI agents working in real repositories.
Current release: **v1.4.5**.

v1.4.5 highlights:
- Agent install now auto-configures MCP for `codex`, `claude-code`, `copilot`, and `cursor`.
- Codex install now auto-updates both `~/.codex/config.toml` and `~/.codex/AGENTS.md` for MCP-first workflows.
- Agent/MCP docs were tightened with verification-first setup guidance.

- Docs site: <https://meghendra6.github.io/cgrep/>
- Repository README: [README.md](https://github.com/meghendra6/cgrep/blob/main/README.md)
- Korean docs: [ko/index.md](./ko/index.md)

## Why cgrep

- Built for AI coding loops: compact, deterministic `json2` output and two-stage `agent locate/expand`.
- Code-aware navigation: `definition`, `references`, `callers`, `dependents`, `map`, and `read`.
- Ergonomic CLI: short aliases like `s`, `d`, `r`, `c`, `dep`, `i`, `a l`.
- Local-first operations: fast retrieval, private code, no cloud dependency.

## Benchmark Snapshot (PyTorch)

Measured on February 17, 2026 across 6 implementation-tracing scenarios.
Completion model: iterative retrieval until each scenario completion rule is satisfied.

| Metric | Baseline (`grep`) | cgrep (`agent locate/expand`) | Improvement |
|---|---:|---:|---:|
| Total tokens-to-complete | 128,042 | 6,159 | **95.2% less** |
| Avg tokens-to-complete per task | 21,340 | 1,027 | **20.79x smaller** |
| Avg retrieval latency to completion | 1,287.7 ms | 21.7 ms | **~59.5x faster** |

Details: [Benchmark: Agent Token Efficiency](./benchmarks/pytorch-agent-token-efficiency.md)

## Codex Real-Agent Snapshot (PyTorch)

Measured on February 17, 2026 with `gpt-5-codex` (`reasoning_effort=medium`, `runs=2`).

| Metric | Baseline | cgrep | Notes |
|---|---:|---:|---|
| Success rate (all cases) | 91.7% | 100.0% | strict command-policy validation enabled (one baseline timeout) |
| Total billable tokens (all cases) | 167,409 | 89,967 | cgrep **46.3% less** |

Details: [Benchmark: Codex Agent Efficiency](./benchmarks/pytorch-codex-agent-efficiency.md)

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
| [Benchmark: Codex Agent Efficiency](./benchmarks/pytorch-codex-agent-efficiency.md) | Real `codex exec` benchmark on PyTorch with provider token telemetry |
| [Troubleshooting](./troubleshooting.md) | Common issues and fixes |
| [Development](./development.md) | Build, test, and validation commands |

## Quick Links

- Changelog: [CHANGELOG.md](https://github.com/meghendra6/cgrep/blob/main/CHANGELOG.md)
- Comparison: [COMPARISON.md](https://github.com/meghendra6/cgrep/blob/main/COMPARISON.md)
