# cgrep Documentation

Local-first code search and navigation for users and AI agents.

Current release: **v1.5.2**

## Start By Goal

| I want to... | Open this doc |
|---|---|
| Install quickly and run first command | [Installation](./installation.md) |
| Learn daily search/navigation commands | [Usage](./usage.md) |
| Set up token-efficient agent retrieval | [Agent Workflow](./agent.md) |
| Connect editor/host tools via MCP | [MCP](./mcp.md) |
| Keep large repo index fresh | [Indexing and Daemon](./indexing-watch.md) |
| Operate background/reuse safely | [Operations](./operations.md) |
| Tune defaults and profiles | [Configuration](./configuration.md) |
| Use semantic/hybrid retrieval (experimental) | [Embeddings](./embeddings.md) |
| Fix common failures quickly | [Troubleshooting](./troubleshooting.md) |
| Run build/test/perf validation | [Development](./development.md) |

## Common Paths

### User path (2 minutes)

```bash
# optional warm-up
cgrep index
cgrep s "token validation" src/
cgrep d handle_auth
cgrep read src/auth.rs
```

### Agent path (low-token retrieval)

```bash
ID=$(cgrep agent locate "where token validation happens" --compact | jq -r '.results[0].id')
cgrep agent expand --id "$ID" -C 8 --compact
cgrep --format json2 --compact agent plan "trace authentication middleware flow"
```

## Indexing Decision Quick Guide

| Situation | Recommended |
|---|---|
| Start searching immediately | run `search/read/definition` directly (auto-bootstrap) |
| Keep index warm while coding | `cgrep daemon start` during session, then `cgrep daemon stop` |
| One-time asynchronous prebuild | `cgrep index --background` |
| Semantic/hybrid experiments | build embeddings index (`cgrep index --embeddings precompute`) |

## Benchmark References

- [Benchmark: Agent Token Efficiency (PyTorch)](./benchmarks/pytorch-agent-token-efficiency.md)
- [Benchmark: Codex Agent Efficiency (PyTorch)](./benchmarks/pytorch-codex-agent-efficiency.md)
- Latest Codex snapshot (February 21, 2026 UTC): cgrep billable tokens **41,011** vs baseline **114,060** (**64.0%** reduction, `runs=1`).
- Latest measured numbers are tracked in each benchmark page.

## Language And Site

- Korean docs hub: [ko/index.md](./ko/index.md)
- Canonical docs site: <https://meghendra6.github.io/cgrep/>
- Repository README: [README.md](https://github.com/meghendra6/cgrep/blob/main/README.md)

## Related Files

- Changelog: [CHANGELOG.md](https://github.com/meghendra6/cgrep/blob/main/CHANGELOG.md)
- Comparison: [COMPARISON.md](https://github.com/meghendra6/cgrep/blob/main/COMPARISON.md)
- Contributing: [CONTRIBUTING.md](https://github.com/meghendra6/cgrep/blob/main/CONTRIBUTING.md)
- Security: [SECURITY.md](https://github.com/meghendra6/cgrep/blob/main/SECURITY.md)
