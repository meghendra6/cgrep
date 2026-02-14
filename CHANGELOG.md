# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

- No unreleased changes yet.

## [1.4.0] - 2026-02-14

### Added
- Background daemon management for watch mode: `cgrep daemon start|status|stop`.
- MCP server mode (`cgrep mcp serve`) and host config install/uninstall commands.
- Smart repository navigation commands: `cgrep read` and `cgrep map`.
- AST/regex/auto mode selection for symbol usage queries (`callers`, `references`).
- Adaptive watch controls for large repositories (`--min-interval`, `--max-batch-delay`, `--no-adaptive`).
- Performance regression CI gate (`scripts/perf_gate.py` + `.github/workflows/perf-gate.yml`).
- Documentation hub + GitHub Pages docs site, including separate Korean documentation.

### Changed
- Improved definition lookup speed with index-first candidate narrowing.
- Watch reindex behavior is now changed-path incremental rather than full rebuild per cycle.
- Watch defaults are tuned for low-resource background operation (`--min-interval 180`).
- Agent install/harness documentation refreshed for deterministic tool-call workflows.

### Fixed
- Scoped-index regressions that caused slow non-search commands and missing keyword hits.
- Agent expand fallback/scope matching edge cases.
- Watch noise handling for temp/swap and non-indexable file events.

## [1.3.1] - 2026-02-09

### Added
- Agent-focused payload controls and token-efficiency options.
- `agent locate`/`agent expand` workflow for two-stage retrieval.
- Additional compact output options for automation flows.

### Changed
- Hybrid search quality/runtime improvements.
- Lower memory usage and faster embeddings precompute indexing.
- CLI output and ergonomics cleanup for agent/human use.

### Fixed
- Embedding single-result contract handling and context packing edge cases.

## [1.3.0] - 2026-02-05

### Added
- Embedding generation during indexing (`cgrep index --embeddings ...`).
- Symbol-level embeddings for semantic/hybrid retrieval.
- Parent-index aware search scoping to current working directory.
- Compact JSON output for agent workflows.

### Changed
- Faster definition/callers/references symbol lookups.
- Indexing performance/correctness improvements.
- FastEmbed MiniLM batching/truncation optimization.
- Indexing now includes gitignored paths.

### Removed
- `cg` shortcut binary (use `cgrep search <query>`).

### Fixed
- macOS x86_64 fastembed build path.
- Linux ONNX Runtime dynamic linking path.

## [1.2.0] - 2026-02-03

### Added
- Initial hybrid/semantic search modes and embeddings integration.
- Agent-friendly output formats including `json2`.
- Embedding provider configuration (`builtin`, `command`, `dummy`).

## [1.1.0] - 2026-02-01

### Added
- Scan mode fallback when index doesn't exist.
- Regex search support (`--regex` flag).
- Case-sensitive search option (`--case-sensitive` flag).
- Context lines display (`-C, --context` flag).

### Changed
- Improved indexing performance with parallel parsing.
- Better file type detection and filtering.
- Enhanced output formatting with colors.

### Fixed
- Binary file detection improvements.
- Large file handling with chunked indexing.

## [1.0.0] - 2026-02-01

### Added
- BM25 full-text search (Tantivy).
- AST-based symbol extraction (tree-sitter).
- Multi-language: TS, JS, Python, Rust, Go, C, C++, Java, Ruby.
- AI agent integrations: Copilot, Claude Code, Codex, OpenCode.
- JSON output format.
- Incremental indexing with parallel parsing.
- Shell completions.
