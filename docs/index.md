# cgrep Documentation

`grep` finds text. `cgrep` finds code intent.

Local-first code search for humans and AI agents working in real repositories.

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

| Metric | Baseline (`grep`) | cgrep (`agent locate/expand`) | Improvement |
|---|---:|---:|---:|
| Total agent context tokens | 164,961 | 11,293 | **93.2% less** |
| Avg tokens per task | 27,494 | 1,882 | **14.61x smaller** |
| Avg retrieval latency per task | 1,244.2 ms | 20.8 ms | **~59.7x faster** |

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
