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

Baseline side uses plain `grep` + snippet expansion to model non-cgrep agent workflows.

This writes:
- `docs/benchmarks/pytorch-agent-token-efficiency.md`
- `local/benchmarks/pytorch-agent-token-efficiency.json` (local-only)
