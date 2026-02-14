# PyTorch AI Agent Token Efficiency Benchmark

Generated: 2026-02-14T08:03:08.493784+00:00

## What This Measures

1. **Baseline (without cgrep):** `grep` locate + manual snippet expansion from multiple files.
2. **With cgrep:** `agent locate` + `agent expand` (tight budget, compact JSON).
3. **Primary metric:** token volume sent to an AI coding agent for task completion.
4. **Tokenizer:** OpenAI `cl100k_base` when available (fallback: byte/4 approximation).

## Environment

- OS: `macOS-26.2-arm64-arm-64bit`
- cgrep commit: `be3d92b`
- pytorch commit: `b7abe8e3ab9`
- PyTorch files (`git ls-files`): `20437`
- Tokenizer: `tiktoken:cl100k_base`

## Results

| Scenario | Baseline tokens | cgrep tokens | Reduction | Baseline latency (ms) | cgrep latency (ms) |
|---|---:|---:|---:|---:|---:|
| Find where autograd engine evaluate_function is implemented and inspected. | 11,871 | 1,476 | 87.6% | 1852.42 | 20.99 |
| Find TensorIterator definition and major implementation usage points. | 48,608 | 1,563 | 96.8% | 1107.06 | 21.47 |
| Locate PythonArgParser implementation and usage points. | 17,825 | 1,938 | 89.1% | 1016.71 | 19.99 |
| Understand DispatchKeySet representation and references. | 43,742 | 2,186 | 95.0% | 992.59 | 21.62 |
| Locate CUDAGraph implementation-related code quickly. | 17,955 | 2,027 | 88.7% | 1006.02 | 19.96 |
| Find addmm implementation and call sites. | 24,960 | 2,103 | 91.6% | 1490.39 | 20.97 |

## Aggregate

- One-time index build: **4.93s**
- Baseline total tokens: **164,961**
- cgrep total tokens: **11,293**
- Token reduction: **93.2%**
- Token compression ratio (baseline/cgrep): **14.61x**

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
