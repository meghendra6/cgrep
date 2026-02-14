# cgrep

`grep` finds text. `cgrep` finds code intent.

Built for humans and AI agents working in real repositories.

`cgrep` combines:
- BM25 full-text search (Tantivy)
- AST symbol extraction (tree-sitter)
- optional semantic/hybrid search with embeddings
- deterministic JSON output for tool/agent workflows

Everything runs locally.

## Why Teams Choose cgrep

- Proven on large codebases: in PyTorch agent workflows, cgrep cut context tokens by **93.2%** (**14.61x**) and reduced retrieval loop latency by about **59.7x** after indexing.
- Get answers, not just matching lines: `definition`, `references`, `callers`, `dependents`, `map`, `read`.
- Keep AI-agent loops small with `agent locate` + `agent expand` and compact `json2` output.
- Ergonomic CLI shortcuts: `s`, `d`, `r`, `c`, `dep`, `i`, `a l`, plus short flags like `-u`, `-M`, `-B`, `-P`.
- Stay local-first for speed and privacy (no cloud index required).
- Scale safely on large repos with indexing, watch/daemon, and MCP server mode.

## grep vs cgrep (Practical)

| You need to... | Plain grep workflow | cgrep workflow |
|---|---|---|
| Find where logic is implemented | Iterate patterns + open many files manually | `cgrep definition/references/callers` directly |
| Feed context to AI coding agents | Large, noisy payloads | Budgeted, structured payloads (`agent`, `json2`) |
| Keep retrieval stable over time | Ad-hoc scripts per repo | Built-in index/watch/daemon + MCP integration |

## Benchmark Snapshot (PyTorch)

- Measured on February 14, 2026 across 6 AI-coding scenarios (implementation/structure tracing on PyTorch).
- One-time index build: **4.93s**.

| Metric | Baseline (`grep`) | cgrep (`agent locate/expand`) | Improvement |
|---|---:|---:|---:|
| Total agent context tokens | 164,961 | 11,293 | **93.2% less** |
| Avg tokens per task | 27,494 | 1,882 | **14.61x smaller** |
| Avg retrieval latency per task | 1,244.2 ms | 20.8 ms | **~59.7x faster** |

- Practical meaning: for the same tasks, cgrep sends only **6.8%** of the context that a plain `grep` workflow sends.
- Full methodology and raw data: `docs/benchmarks/pytorch-agent-token-efficiency.md`.

## Install

```bash
cargo install --path .

# or build manually
cargo build --release
cp target/release/cgrep ~/.local/bin/
```

Detailed setup: `docs/installation.md`

## Quick Start

```bash
# Build index once (recommended)
cgrep index

# Keyword search
cgrep search "authentication flow" -t rust -p src/

# Symbol navigation
cgrep definition handle_auth
cgrep references UserService --mode auto

# Smart read/map
cgrep read src/auth.rs
cgrep map --depth 2
```

Shortcut-first equivalents:

```bash
cgrep i                       # index
cgrep s "authentication flow" # search
cgrep d handle_auth           # definition
cgrep r UserService           # references
cgrep c validate_token        # callers
cgrep dep src/auth.rs         # dependents
```

Agent workflow example:

```bash
ID=$(cgrep agent locate "where token validation happens" --compact | jq -r '.results[0].id')
cgrep agent expand --id "$ID" -C 8 --compact
```

## Documentation

- Docs hub (GitHub): `docs/index.md`
- Docs site: <https://meghendra6.github.io/cgrep/>
- Korean docs: `docs/ko/index.md`
- Getting started: `docs/installation.md`
- CLI usage and search options: `docs/usage.md`
- Agent workflow and integration install: `docs/agent.md`
- MCP server and harness guidance: `docs/mcp.md`
- Indexing, watch, and daemon: `docs/indexing-watch.md`
- Configuration: `docs/configuration.md`
- Embeddings mode: `docs/embeddings.md`
- Agent token benchmark: `docs/benchmarks/pytorch-agent-token-efficiency.md`
- Troubleshooting: `docs/troubleshooting.md`
- Development and validation: `docs/development.md`

## Notes

- Changelog: `CHANGELOG.md`
- Comparison material: `COMPARISON.md`
- Harness rationale: <https://blog.can.ac/2026/02/12/the-harness-problem/>
