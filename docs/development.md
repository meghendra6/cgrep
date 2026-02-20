# Development

## Daily Validation Loop

```bash
cargo build
cargo test
cargo clippy --all-targets --all-features -- -D warnings
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

Run this after search/indexing-related changes.

## Release-Ready Checklist

- Build passes (`cargo build`)
- Tests pass (`cargo test`)
- Clippy clean (`-D warnings`)
- Performance gate passes (`scripts/index_perf_gate.py`)
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
  --runs 1
```

Tracks:
- `input_tokens`, `cached_input_tokens`, `output_tokens`
- `billable_tokens = input - cached_input + output`
- success/failure under command-policy constraints

Outputs:
- `docs/benchmarks/pytorch-codex-agent-efficiency.md`
- `local/benchmarks/pytorch-codex-agent-efficiency.json` (local-only)

Latest checked snapshot (`2026-02-18`, `runs=1`, `gpt-5-codex`, `medium`):
- baseline `89,764` -> cgrep `21,092` billable tokens (`76.5%` reduction)
