# cgrep

Local code search for humans and AI agents.

`cgrep` combines:
- BM25 full-text search (Tantivy)
- AST symbol extraction (tree-sitter)
- optional semantic/hybrid search with embeddings
- deterministic JSON output for tool/agent workflows

Everything runs locally.

## Why cgrep

- Fast search in medium/large codebases
- Better code-aware lookup than plain grep for symbols/definitions
- Agent-friendly output (`json2`) and payload controls
- Background-friendly indexing/watch for large repositories
- MCP server mode for tool-call based AI workflows

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

Agent workflow example:

```bash
ID=$(cgrep agent locate "where token validation happens" --compact | jq -r '.results[0].id')
cgrep agent expand --id "$ID" -C 8 --compact
```

## Documentation

- Docs hub (GitHub): `docs/index.md`
- Docs site (left sidebar + right content): <https://meghendra6.github.io/cgrep/>
- Getting started: `docs/installation.md`
- CLI usage and search options: `docs/usage.md`
- Agent workflow and integration install: `docs/agent.md`
- MCP server and harness guidance: `docs/mcp.md`
- Indexing, watch, and daemon: `docs/indexing-watch.md`
- Configuration: `docs/configuration.md`
- Embeddings mode: `docs/embeddings.md`
- Troubleshooting: `docs/troubleshooting.md`
- Development and validation: `docs/development.md`

## Notes

- Changelog: `CHANGELOG.md`
- Comparison material: `COMPARISON.md`
- Harness rationale: <https://blog.can.ac/2026/02/12/the-harness-problem/>
