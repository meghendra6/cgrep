# PyTorch Codex Agent Efficiency Benchmark

Generated: 2026-02-21T11:27:41.226670+00:00

## What This Measures

- Real `codex exec` runs on a local PyTorch repository.
- Baseline mode: autonomous retrieval with cgrep disallowed.
- cgrep mode: cgrep command usage required.
- Primary metric: Codex provider-reported billable tokens (`input - cached_input + output`).

## Environment

- OS: `macOS-26.3-arm64-arm-64bit`
- Python: `3.12.4`
- codex model: `gpt-5-codex`
- reasoning effort: `medium`
- runs per scenario/mode: `1`
- cgrep commit: `d2ad6c3`
- pytorch commit: `66e77ae932c`
- PyTorch files (`git ls-files`): `21634`

## Aggregate (All Cases)

| Mode | Cases | Success rate | Median billable tokens | P95 billable tokens | Median total tokens | Median duration (ms) | Median commands |
|---|---:|---:|---:|---:|---:|---:|---:|
| `baseline` | 6 | 66.7% | 12010 | 46327 | 44138 | 17921.5 | 3.0 |
| `cgrep` | 6 | 100.0% | 7052 | 9331 | 27622 | 13254.1 | 2.0 |

- Total billable tokens (baseline, no cgrep): **114,060**
- Total billable tokens (cgrep): **41,011**
- Billable token reduction: **64.0%**

## Per Scenario

| Run | Scenario | Mode | Success | Billable tokens | Total tokens | Duration (ms) | Commands |
|---:|---|---|---|---:|---:|---:|---:|
| 1 | `autograd_evaluate_function` | `baseline` | yes | 2,751 | 16,831 | 8902.3 | 1 |
| 1 | `autograd_evaluate_function` | `cgrep` | yes | 5,109 | 38,389 | 16996.7 | 3 |
| 1 | `tensor_iterator_impl` | `baseline` | no | 51,409 | 167,889 | 52705.0 | 10 |
| 1 | `tensor_iterator_impl` | `cgrep` | yes | 6,194 | 37,682 | 18780.0 | 3 |
| 1 | `python_arg_parser_impl` | `baseline` | yes | 4,799 | 28,735 | 14915.2 | 2 |
| 1 | `python_arg_parser_impl` | `cgrep` | yes | 9,210 | 17,402 | 8917.6 | 1 |
| 1 | `dispatch_key_set` | `baseline` | no | 31,082 | 106,474 | 36457.5 | 7 |
| 1 | `dispatch_key_set` | `cgrep` | yes | 9,371 | 17,563 | 9511.4 | 1 |
| 1 | `cuda_graph` | `baseline` | yes | 17,794 | 43,138 | 16910.6 | 3 |
| 1 | `cuda_graph` | `cgrep` | yes | 3,217 | 17,297 | 5866.1 | 1 |
| 1 | `addmm_path` | `baseline` | yes | 6,225 | 45,137 | 18932.3 | 3 |
| 1 | `addmm_path` | `cgrep` | yes | 7,910 | 42,086 | 17377.0 | 3 |

## Re-run

```bash
python3 scripts/benchmark_codex_agent_efficiency.py --repo /path/to/pytorch --cgrep-bin /path/to/cgrep
```
