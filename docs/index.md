# cgrep Documentation

Local-first code search and navigation for users and AI agents.

Current release: **v1.4.8**

## Start By Goal

| I want to... | Open this doc |
|---|---|
| Install quickly and run first command | [Installation](./installation.md) |
| Learn daily search/navigation commands | [Usage](./usage.md) |
| Set up token-efficient agent retrieval | [Agent Workflow](./agent.md) |
| Connect editor/host tools via MCP | [MCP](./mcp.md) |
| Keep large repo index fresh | [Indexing and Watch](./indexing-watch.md) |
| Tune defaults and profiles | [Configuration](./configuration.md) |
| Use semantic/hybrid retrieval | [Embeddings](./embeddings.md) |
| Fix common failures quickly | [Troubleshooting](./troubleshooting.md) |
| Run build/test/perf validation | [Development](./development.md) |

## Common Paths

### User path (2 minutes)

```bash
cgrep index
cgrep s "token validation" src/
cgrep d handle_auth
cgrep read src/auth.rs
```

### Agent path (low-token retrieval)

```bash
ID=$(cgrep agent locate "where token validation happens" --compact | jq -r '.results[0].id')
cgrep agent expand --id "$ID" -C 8 --compact
```

## Benchmark References

- [Benchmark: Agent Token Efficiency (PyTorch)](./benchmarks/pytorch-agent-token-efficiency.md)
- [Benchmark: Codex Agent Efficiency (PyTorch)](./benchmarks/pytorch-codex-agent-efficiency.md)
- Latest Codex snapshot (`2026-02-18`, `gpt-5-codex`, `runs=2`, historical benchmark snapshot):
  baseline `233,825` -> cgrep `134,432` billable tokens (`42.5%` reduction).

## Language And Site

- Korean docs hub: [ko/index.md](./ko/index.md)
- Canonical docs site: <https://meghendra6.github.io/cgrep/>
- Repository README: [README.md](https://github.com/meghendra6/cgrep/blob/main/README.md)

## Related Files

- Changelog: [CHANGELOG.md](https://github.com/meghendra6/cgrep/blob/main/CHANGELOG.md)
- Comparison: [COMPARISON.md](https://github.com/meghendra6/cgrep/blob/main/COMPARISON.md)
- Contributing: [CONTRIBUTING.md](https://github.com/meghendra6/cgrep/blob/main/CONTRIBUTING.md)
- Security: [SECURITY.md](https://github.com/meghendra6/cgrep/blob/main/SECURITY.md)
