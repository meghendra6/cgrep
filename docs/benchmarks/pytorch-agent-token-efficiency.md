# PyTorch AI Agent Token Efficiency Benchmark

Generated: 2026-02-17T01:13:05.138969+00:00

> Snapshot note: these numbers were collected at cgrep commit `3606f38`. They are historical benchmark results, not a guarantee for every later release.

## What This Measures

1. **Baseline (without cgrep):** `grep` locate + incremental snippet expansion tiers.
2. **With cgrep:** `agent locate` once + incremental `agent expand` ID tiers.
3. **Completion rule:** scenario is complete when each marker-group has at least one match in cumulative tool outputs.
4. **Primary metric:** cumulative tokens consumed until completion (`tokens-to-complete`).
5. **Tokenizer:** OpenAI `cl100k_base` when available (fallback: byte/4 approximation).

## Environment

- OS: `macOS-26.2-arm64-arm-64bit`
- cgrep commit: `3606f38`
- pytorch commit: `b7abe8e3ab9`
- PyTorch files (`git ls-files`): `20437`
- Tokenizer: `tiktoken:cl100k_base`
- Baseline file tiers: `[2, 4, 6, 8, 12]`
- cgrep expand tiers: `[1, 2, 4, 6, 8]`

## Results

| Scenario | Representative coding task | Baseline done | cgrep done | Baseline attempts | cgrep attempts | Baseline tokens-to-complete | cgrep tokens-to-complete | Reduction | Baseline latency (ms) | cgrep latency (ms) |
|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|
| Find where autograd engine evaluate_function is implemented and inspected. | Patch autograd evaluate_function flow and verify the implementation file + autograd context. | yes | yes | 1 | 1 | 7,129 | 939 | 86.8% | 2036.06 | 22.15 |
| Find TensorIterator definition and major implementation usage points. | Prepare a TensorIterator behavior change by locating the core declaration and implementation paths. | yes | yes | 1 | 1 | 43,263 | 1,026 | 97.6% | 1121.76 | 20.12 |
| Locate PythonArgParser implementation and usage points. | Implement a parser-related fix by gathering PythonArgParser definition and source implementation. | yes | yes | 1 | 1 | 6,741 | 1,004 | 85.1% | 1018.43 | 20.89 |
| Understand DispatchKeySet representation and references. | Refactor DispatchKeySet logic with confidence by finding its representation and core references. | yes | yes | 1 | 1 | 43,743 | 1,028 | 97.6% | 1017.19 | 21.95 |
| Locate CUDAGraph implementation-related code quickly. | Make a CUDAGraph code-path update by collecting implementation and CUDA path context. | yes | yes | 1 | 1 | 11,476 | 1,018 | 91.1% | 1016.10 | 23.94 |
| Find addmm implementation and call sites. | Modify addmm behavior by locating native implementation and addmm_out call path. | yes | yes | 1 | 1 | 15,690 | 1,144 | 92.7% | 1516.72 | 20.89 |

## Aggregate

- One-time index build: **5.11s**
- Scenarios completed (baseline): **6/6**
- Scenarios completed (cgrep): **6/6**
- Baseline tokens-to-complete (total): **128,042**
- cgrep tokens-to-complete (total): **6,159**
- Token reduction (to completion): **95.2%**
- Token compression ratio (baseline/cgrep): **20.79x**
- Baseline total latency to completion: **7726.26ms**
- cgrep total latency to completion: **129.95ms**

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
