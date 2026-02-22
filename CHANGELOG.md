# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Added `scripts/validate_all.sh` as a single deterministic validation workflow for indexing/search, incremental update, agent plan, status/search-stats checks, doctor flow, and docs link sanity checks.
- Added operations runbooks: `docs/operations.md` and `docs/ko/operations.md`.
- Added hardening integration tests in `tests/m7_hardening.rs` covering deterministic json2/compact contracts, cross-feature option matrix smoke, and legacy mode-alias compatibility.

### Changed
- Consolidated docs around deterministic output and compatibility:
  - `README.md`, `docs/usage.md`, `docs/ko/usage.md`
  - `docs/configuration.md`, `docs/ko/configuration.md`
  - `docs/agent.md`, `docs/ko/agent.md`
  - `docs/index.md`, `docs/ko/index.md`
  - `docs/indexing-watch.md`, `docs/ko/indexing-watch.md`
  - `docs/development.md`, `docs/ko/development.md`
- Extended `perf-gate` workflow coverage to include deterministic local validation (`scripts/validate_all.sh`) in addition to existing perf gates.

### Fixed
- Improved C/C++ type resolution in `definition` for macro-annotated declarations (for example `struct TORCH_API Foo`) so symbol lookup returns primary type definitions instead of noisy constructor/base-class artifacts.
- `.h` headers now use C++ parsing for symbol navigation, improving real-world accuracy in C++-heavy repositories (including mixed `.h` header layouts).

### Compatibility
- CLI command surface remains unchanged; C/C++ symbol parsing now treats `.h` headers as C++ for navigation commands.
- Existing aliases and deprecated compatibility mode flags remain supported.
- `json2` contracts remain additive; optional fields are omitted when empty and required fields remain stable.

### Known limitations
- Search timing fields (for example `elapsed_ms`) are intentionally informational and should not be used as deterministic ordering keys.
- Docs link sanity checks validate repository-local links in core docs targets; external URL health remains out of scope.

## [1.5.2] - 2026-02-21

### Changed
- Bumped package version to `v1.5.2` and synchronized current-release markers across `Cargo.toml`, `README.md`, and docs hubs (EN/KO).
- Updated installation verification snippets to use `TAG=v1.5.2` in both English and Korean docs.
- Refreshed Codex benchmark methodology so baseline runs prohibit `cgrep` but allow autonomous non-cgrep retrieval, while cgrep mode requires explicit cgrep usage.
- Hardened benchmark command-policy checks for determinism (`--help` token matching, shell-control rejection, tool allowlists, exact cgrep binary/subcommand validation).
- Refreshed Codex benchmark snapshot to the latest measured run (`114,060` -> `41,011`, `64.0%` billable-token reduction, `runs=1`).
- Reviewed and polished docs wording for newly-added operational/validation features so README and docs hubs remain concise and consistent.

## [1.5.1] - 2026-02-19

### Changed
- Bumped package version to `v1.5.1` and synchronized current-release markers across `Cargo.toml`, `README.md`, and docs hubs (EN/KO).
- Updated installation verification snippets to use `TAG=v1.5.1` in both English and Korean docs.
- Updated installer usage example to reference `v1.5.1`.

## [1.5.0] - 2026-02-18

### Changed
- Bumped package version to `v1.5.0` and synchronized release markers across `Cargo.toml`, `README.md`, and docs hubs (EN/KO).
- Updated installation guide verification snippets to use `TAG=v1.5.0` in both English and Korean docs.
- Refreshed Codex PyTorch benchmark snapshots to the latest measured run (`89,764` -> `21,092`, `76.5%` billable-token reduction, `runs=1`).
- Expanded README's "For AI Agents" quick install path across host targets and linked to docs-site guides for agent/MCP details.

## [1.4.8] - 2026-02-18

### Changed
- Bumped package version to `v1.4.8` and synchronized release markers across `Cargo.toml`, `README.md`, and docs hubs (EN/KO).
- Updated installation guide verification snippets to use `TAG=v1.4.8` in both English and Korean docs.
- Refreshed docs index highlights so the docs site reflects the latest release state.

### Fixed
- Restored and validated the docs-site quick link in `README.md` so users can reliably jump to the canonical site.
- Resolved docs-site freshness drift by aligning release/version text at the primary documentation entry points.

## [1.4.7] - 2026-02-18

### Changed
- Search UX is now consistently explicit and grep-friendly: use `cgrep search`/`cgrep s` with positional scope (`[path]`) and familiar scope flags.
- Agent install guidance was compacted and unified so providers share the same low-token `map -> search -> read -> symbol` core flow.
- Documentation (README + docs site, EN/KO) was refactored for readability and consistency, including clearer `read`/`map` guidance and Codex-vs-MCP install separation.
- Codex PyTorch benchmark snapshots were refreshed to the latest re-run (`233,825` -> `134,432`, `42.5%` billable-token reduction).

### Fixed
- MCP search/read round-trip reliability: search result paths are reusable across workspace-internal and external scopes, and empty path artifacts were removed.
- MCP path resolution stability in agent environments: optional `cwd` handling and bounded MCP tool execution now prevent root-scan accidents and host-timeout hangs.
- Dash-prefixed query handling now stays literal in MCP search flows, and query/path validation was hardened (`empty/whitespace` query rejection and empty `read` path rejection).
- Scan-mode snippet extraction no longer panics on invalid UTF-8 slices.

## [1.4.6] - 2026-02-17

### Changed
- `definition` now supports `-p/--path` scope and `-m/--limit` result limits for lower-noise symbol lookups in large repositories.
- `definition` ranking now prefers high-signal matches and deduplicates repeated overload-style entries from the same file.
- Codex benchmark cgrep command plans now use scoped, capped definition/search calls to reduce retrieval payload size.
- Codex benchmark documentation and README/docs snapshots are synced to the latest rerun, including aggregate and per-scenario values.

### Fixed
- `definition` no longer treats C/C++ forward declarations as full definitions, reducing token-heavy false positives.
- `definition` now filters declaration-only C/C++ function signatures by default, reducing constructor/overload noise without requiring extra flags.

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
