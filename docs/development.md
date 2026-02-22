# Development

## Daily Validation Loop

```bash
cargo build
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## One-command validation workflow

```bash
CGREP_BIN=cgrep bash scripts/validate_all.sh
```

This single workflow verifies:
- core indexing/search
- incremental update path (`--print-diff`)
- agent planning flow (`agent plan`)
- status/search stats payload checks (`json2 --compact`)
- doctor flow (`scripts/doctor.sh`) when repository-integrations files are present
- docs local-link sanity checks (README + docs hub files)

## Docs Site Validation (GitHub Pages)

Local preview:

```bash
mkdocs serve
```

Strict build parity with CI (`docs-pages` workflow):

```bash
mkdocs build --strict
```

## Performance Gate

```bash
python3 scripts/index_perf_gate.py \
  --baseline-bin /path/to/baseline/cgrep \
  --candidate-bin /path/to/candidate/cgrep \
  --runs 3 \
  --warmup 1 \
  --files 1200
```

```bash
python3 scripts/agent_plan_perf_gate.py \
  --baseline-bin /path/to/baseline/cgrep \
  --candidate-bin /path/to/candidate/cgrep \
  --runs 5 \
  --warmup 2 \
  --files 800
```

Run this after search/indexing-related changes.
Performance gate tracks latency `p50`/`p95` for:
- fresh worktree index latency with `--reuse off`
- first keyword search latency after `--reuse off`
- incremental index update latency after a small tracked-file change (`--reuse off`)
- fresh worktree index latency with `--reuse strict`
- fresh worktree index latency with `--reuse auto`
- first keyword search latency after `--reuse strict`
- first keyword search latency after `--reuse auto`
- agent plan latency for simple identifier-like query
- agent plan latency for complex phrase-like query
- agent plan end-to-end latency on expand-heavy query

Methodology:
- `--warmup` executes non-reported warmup runs per metric.
- `--runs` executes measured runs per metric.
- `p50` uses median.
- `p95` uses nearest-rank percentile over measured runs.

CI thresholds (median):
- search regression > `5%`: fail
- cold index build regression > `10%`: fail
- incremental/reuse update regression > `10%`: fail
- agent plan regression > `10%`: fail
- small absolute deltas (`<= 3ms`) are treated as noise for agent-plan perf checks

## Tag-Triggered Release CI

`release` workflow (`.github/workflows/release.yml`) runs when a release tag is pushed.
Accepted tag forms:
- `vMAJOR.MINOR.PATCH` (for example `v1.5.2`)
- `MAJOR.MINOR.PATCH` (for example `1.5.2`)

```bash
git tag v1.5.2
git push origin v1.5.2
```

or

```bash
git tag 1.5.2
git push origin 1.5.2
```

Manual fallback (`workflow_dispatch`) uses the selected commit and publishes with the provided tag input.

## Release-Ready Checklist

- Build passes (`cargo build`)
- Tests pass (`cargo test`)
- Clippy clean (`-D warnings`)
- Validation workflow passes (`scripts/validate_all.sh`)
- Performance gates pass (`scripts/index_perf_gate.py`, `scripts/agent_plan_perf_gate.py`)
- Docs updated for CLI/behavior changes

## Benchmark: Agent Token Efficiency (PyTorch)

```bash
python3 scripts/benchmark_agent_token_efficiency.py --repo /path/to/pytorch
```

Tier tuning:

```bash
python3 scripts/benchmark_agent_token_efficiency.py \
  --repo /path/to/pytorch \
  --baseline-file-tiers 2,4,6,8,12 \
  --cgrep-expand-tiers 1,2,4,6,8
```

Outputs:
- `docs/benchmarks/pytorch-agent-token-efficiency.md`
- `local/benchmarks/pytorch-agent-token-efficiency.json` (local-only)

## Benchmark: Codex Real-Agent Efficiency (PyTorch)

```bash
python3 scripts/benchmark_codex_agent_efficiency.py \
  --repo /path/to/pytorch \
  --cgrep-bin /path/to/cgrep \
  --model gpt-5-codex \
  --reasoning-effort medium \
  --runs 2
```

Tracks:
- `input_tokens`, `cached_input_tokens`, `output_tokens`
- `billable_tokens = input - cached_input + output`
- success/failure under command-policy constraints
- scenario set includes: autograd, TensorIterator, PythonArgParser, DispatchKeySet, CUDAGraph, addmm

Outputs:
- `docs/benchmarks/pytorch-codex-agent-efficiency.md`
- `local/benchmarks/pytorch-codex-agent-efficiency.json` (local-only)

Codex single-run variance can be high; prefer multi-run medians (`--runs >= 2`) for release decisions.

## Benchmark: Search Option Performance (PyTorch)

```bash
python3 scripts/benchmark_search_option_performance.py \
  --repo /path/to/pytorch \
  --cgrep-bin /path/to/cgrep \
  --runs 5 \
  --warmup 1
```

Covers practical `search` option/scenario pairs, including:
- scoped keyword search
- `--type`, `--glob`, `-C`, `-B`, `-P`
- payload-focused flags (`--path-alias`, `--dedupe-context`, `--suppress-boilerplate`)
- scan-mode comparisons (`--no-index`, `--regex --no-index`)

Outputs:
- `docs/benchmarks/pytorch-search-options-performance.md`
- `local/benchmarks/pytorch-search-options-performance.json` (local-only)
