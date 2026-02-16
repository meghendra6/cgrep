# cgrep

`grep` finds text. `cgrep` finds code intent.

Built for humans and AI agents working in real repositories.
Current release: **v1.4.2**.

`cgrep` combines:
- BM25 full-text search (Tantivy)
- AST symbol extraction (tree-sitter)
- optional semantic/hybrid search with embeddings
- deterministic JSON output for tool/agent workflows

Everything runs locally.

## Why Teams Choose cgrep

- Proven on large codebases: in PyTorch scenario-completion workflows, cgrep cut tokens-to-complete by **95.2%** (**20.75x**) and reduced retrieval loop latency by about **58.2x** after indexing.
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
- Completion model: iterative retrieval loops run until each scenario’s completion criteria is satisfied.
- One-time index build: **5.31s**.

| Metric | Baseline (`grep`) | cgrep (`agent locate/expand`) | Improvement |
|---|---:|---:|---:|
| Total tokens-to-complete | 127,665 | 6,153 | **95.2% less** |
| Avg tokens-to-complete per task | 21,278 | 1,026 | **20.75x smaller** |
| Avg retrieval latency to completion | 1,321.3 ms | 22.7 ms | **~58.2x faster** |

- Practical meaning: for the same completed scenarios, cgrep used only **4.8%** of the token volume of a plain `grep` workflow.
- Full methodology and raw data: `docs/benchmarks/pytorch-agent-token-efficiency.md`.

## Install

```bash
# Option 1: install latest GitHub release binary (macOS/Linux)
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh \
  | bash

# Option 2: install from source
cargo install --path .

# Option 3: build manually
cargo build --release
cp target/release/cgrep ~/.local/bin/
```

macOS note:
- If Gatekeeper blocks first launch for a downloaded binary, run:
  `xattr -d com.apple.quarantine ~/.local/bin/cgrep`

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
