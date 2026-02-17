# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.4.5] - 2026-02-17

### Added
- Agent install commands now auto-configure MCP endpoints for supported targets (`codex`, `claude-code`, `copilot`, `cursor`) so installs are immediately MCP-ready.
- Codex install now writes MCP guidance into `~/.codex/AGENTS.md` and ensures `~/.codex/config.toml` contains the cgrep MCP server entry.

### Changed
- Agent and MCP documentation (EN/KO) now describes MCP-first usage and post-install verification flow.

## [1.4.4] - 2026-02-17

### Added
- Codex real-agent benchmark harness on PyTorch (`scripts/benchmark_codex_agent_efficiency.py`) using `codex exec` event telemetry.
- Benchmark report page for Codex runs: `docs/benchmarks/pytorch-codex-agent-efficiency.md`.

### Changed
- Benchmark documentation is now focused on PyTorch-based `tokens-to-complete` results only.

## [1.4.3] - 2026-02-16

### Fixed
- Release artifact verification no longer false-fails with `pipefail` when checking tarball contents (`grep -q` early-exit case).

## [1.4.2] - 2026-02-16

### Added
- Release installer script (`scripts/install_release.sh`) for GitHub release binaries on macOS/Linux with target auto-detection, checksum verification, and install-path control.
- Agent docs now include provider-specific instruction/skill file output paths.

### Changed
- Release workflow now ad-hoc signs macOS binaries for both Apple Silicon and Intel targets before packaging.
- Release workflow checksum files now use archive-relative paths and are self-validated during packaging.

### Fixed
- Release checksum manifests for Unix archives no longer reference a non-existent `dist/` prefix.
- Windows checksum manifests now use LF newline format for cross-platform `sha256sum/shasum -c` verification.

## [1.4.1] - 2026-02-14

### Added
- Shortcut-first CLI aliases for high-frequency commands (`s`, `d`, `r`, `c`, `dep`, `i`, `w`, `a`) and ergonomic subcommand aliases (`agent l/x`, `daemon up/st/down`, `mcp run/add/rm`).
- Short flag variants for common options (for example `-u`, `-M`, `-B`, `-P`, `-x`) to reduce typing overhead during iterative coding-agent workflows.
- CLI parser regression tests covering alias + short-flag usage patterns.
- Cursor agent profile support via `cgrep agent install/uninstall cursor` (project-local `.cursor/rules/cgrep.mdc` generation).
- Cursor MCP host install/uninstall integration tests and agent documentation updates.

### Changed
- Agent token benchmark now reports **tokens-to-complete per scenario** using iterative retrieval tiers with explicit completion markers, not only single-shot payload size.
- Documentation (README + docs site EN/KO) updated for v1.4.1 shortcut usage and revised benchmark methodology.

### Fixed
- Formatting-only consistency update in embedding storage helper signatures (no behavior change).

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
