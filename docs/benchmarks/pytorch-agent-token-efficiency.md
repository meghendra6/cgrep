# PyTorch AI Agent Token Efficiency Benchmark

Generated: 2026-02-22T05:54:51.403171+00:00

> Snapshot note: benchmark outputs vary by repository state and model behavior.
> Compare trends over repeated runs rather than relying on one run.

## What This Measures

1. **Baseline (without cgrep):** `grep` locate + incremental snippet expansion tiers.
2. **With cgrep:** `agent locate` once + incremental `agent expand` ID tiers.
3. **Completion rule:** scenario is complete when each marker-group has at least one match in cumulative tool outputs.
4. **Primary metric:** cumulative tokens consumed until completion (`tokens-to-complete`).
5. **Tokenizer:** OpenAI `cl100k_base` when available (fallback: byte/4 approximation).

## Environment

- OS: `macOS-26.3-arm64-arm-64bit`
- cgrep commit: `88299fb`
- pytorch commit: `66e77ae932c`
- PyTorch files (`git ls-files`): `21634`
- Tokenizer: `tiktoken:cl100k_base`
- Baseline file tiers: `[2, 4, 6, 8, 12]`
- cgrep expand tiers: `[1, 2, 4, 6, 8]`

## Results

| Scenario | Representative coding task | Baseline done | cgrep done | Baseline attempts | cgrep attempts | Baseline tokens-to-complete | cgrep tokens-to-complete | Reduction | Baseline latency (ms) | cgrep latency (ms) |
|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|
| Find where autograd engine evaluate_function is implemented and inspected. | Patch autograd evaluate_function flow and verify the implementation file + autograd context. | yes | yes | 1 | 1 | 9,592 | 1,068 | 88.9% | 2156.00 | 122.63 |
| Find TensorIterator definition and major implementation usage points. | Prepare a TensorIterator behavior change by locating the core declaration and implementation paths. | yes | yes | 1 | 1 | 42,843 | 1,098 | 97.4% | 1297.29 | 23.63 |
| Locate PythonArgParser implementation and usage points. | Implement a parser-related fix by gathering PythonArgParser definition and source implementation. | yes | yes | 1 | 1 | 6,846 | 1,045 | 84.7% | 1216.50 | 125.95 |
| Understand DispatchKeySet representation and references. | Refactor DispatchKeySet logic with confidence by finding its representation and core references. | yes | yes | 1 | 1 | 43,775 | 1,123 | 97.4% | 1211.08 | 35.84 |
| Locate CUDAGraph implementation-related code quickly. | Make a CUDAGraph code-path update by collecting implementation and CUDA path context. | yes | yes | 1 | 1 | 11,702 | 1,107 | 90.5% | 1261.31 | 124.17 |
| Find addmm implementation and call sites. | Modify addmm behavior by locating native implementation and addmm_out call path. | yes | yes | 1 | 1 | 15,678 | 1,219 | 92.2% | 1757.03 | 24.08 |

## Aggregate

- One-time index build: **5.66s**
- Scenarios completed (baseline): **6/6**
- Scenarios completed (cgrep): **6/6**
- Baseline tokens-to-complete (total): **130,436**
- cgrep tokens-to-complete (total): **6,660**
- Token reduction (to completion): **94.9%**
- Token compression ratio (baseline/cgrep): **19.58x**
- Baseline total latency to completion: **8899.20ms**
- cgrep total latency to completion: **456.31ms**

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
