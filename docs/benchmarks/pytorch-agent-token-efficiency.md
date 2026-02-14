# PyTorch AI Agent Token Efficiency Benchmark

Generated: 2026-02-14T08:53:38.505639+00:00

## What This Measures

1. **Baseline (without cgrep):** `grep` locate + incremental snippet expansion tiers.
2. **With cgrep:** `agent locate` once + incremental `agent expand` ID tiers.
3. **Completion rule:** scenario is complete when each marker-group has at least one match in cumulative tool outputs.
4. **Primary metric:** cumulative tokens consumed until completion (`tokens-to-complete`).
5. **Tokenizer:** OpenAI `cl100k_base` when available (fallback: byte/4 approximation).

## Environment

- OS: `macOS-26.2-arm64-arm-64bit`
- cgrep commit: `723324c`
- pytorch commit: `b7abe8e3ab9`
- PyTorch files (`git ls-files`): `20437`
- Tokenizer: `tiktoken:cl100k_base`
- Baseline file tiers: `[2, 4, 6, 8, 12]`
- cgrep expand tiers: `[1, 2, 4, 6, 8]`

## Results

| Scenario | Baseline done | cgrep done | Baseline attempts | cgrep attempts | Baseline tokens-to-complete | cgrep tokens-to-complete | Reduction | Baseline latency (ms) | cgrep latency (ms) |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|
| Find where autograd engine evaluate_function is implemented and inspected. | yes | yes | 1 | 1 | 7,024 | 939 | 86.6% | 1907.61 | 21.58 |
| Find TensorIterator definition and major implementation usage points. | yes | yes | 1 | 1 | 43,255 | 1,026 | 97.6% | 1170.75 | 21.34 |
| Locate PythonArgParser implementation and usage points. | yes | yes | 1 | 1 | 6,740 | 1,000 | 85.2% | 1079.13 | 22.37 |
| Understand DispatchKeySet representation and references. | yes | yes | 1 | 1 | 43,740 | 1,028 | 97.6% | 1057.25 | 23.04 |
| Locate CUDAGraph implementation-related code quickly. | yes | yes | 1 | 1 | 11,217 | 1,018 | 90.9% | 1092.60 | 23.76 |
| Find addmm implementation and call sites. | yes | yes | 1 | 1 | 15,689 | 1,142 | 92.7% | 1620.41 | 24.21 |

## Aggregate

- One-time index build: **5.31s**
- Scenarios completed (baseline): **6/6**
- Scenarios completed (cgrep): **6/6**
- Baseline tokens-to-complete (total): **127,665**
- cgrep tokens-to-complete (total): **6,153**
- Token reduction (to completion): **95.2%**
- Token compression ratio (baseline/cgrep): **20.75x**
- Baseline total latency to completion: **7927.74ms**
- cgrep total latency to completion: **136.30ms**

## Re-run

```bash
python3 scripts/benchmark_agent_token_efficiency.py --repo /path/to/pytorch
```

## Periodic Tracking

```bash
python3 scripts/benchmark_agent_token_efficiency.py --repo /path/to/pytorch --history-dir local/benchmarks/history
```

```cron
0 3 * * 1 cd /path/to/cgrep && python3 scripts/benchmark_agent_token_efficiency.py --repo /path/to/pytorch --history-dir local/benchmarks/history
```
