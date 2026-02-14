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
