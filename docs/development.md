# Development

## Build and checks

```bash
cargo build
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## Performance gate

```bash
python3 scripts/perf_gate.py
```

Run the performance gate after search/indexing changes to catch regressions.

## Agent token benchmark (PyTorch)

```bash
python3 scripts/benchmark_agent_token_efficiency.py --repo /path/to/pytorch
```

This benchmark measures **tokens-to-complete** for each scenario:
- baseline: `grep` locate + incremental file-snippet tiers
- cgrep: `agent locate` + incremental `agent expand` ID tiers

Tier controls:

```bash
python3 scripts/benchmark_agent_token_efficiency.py \
  --repo /path/to/pytorch \
  --baseline-file-tiers 2,4,6,8,12 \
  --cgrep-expand-tiers 1,2,4,6,8
```

This writes:
- `docs/benchmarks/pytorch-agent-token-efficiency.md`
- `local/benchmarks/pytorch-agent-token-efficiency.json` (local-only)

## Codex real-agent benchmark (PyTorch)

```bash
python3 scripts/benchmark_codex_agent_efficiency.py \
  --repo /path/to/pytorch \
  --cgrep-bin /path/to/cgrep \
  --model gpt-5-codex \
  --reasoning-effort medium \
  --runs 2
```

This benchmark runs real `codex exec` sessions and records provider telemetry:
- `input_tokens`, `cached_input_tokens`, `output_tokens`
- `billable_tokens = input - cached_input + output`
- success/failure under command-policy constraints
- both `all_cases` and `success_only` aggregates are reported

This writes:
- `docs/benchmarks/pytorch-codex-agent-efficiency.md`
- `local/benchmarks/pytorch-codex-agent-efficiency.json` (local-only)
