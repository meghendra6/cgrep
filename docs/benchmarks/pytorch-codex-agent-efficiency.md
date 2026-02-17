# PyTorch Codex Agent Efficiency Benchmark

Generated: 2026-02-17T02:16:51.150186+00:00

## What This Measures

- Real `codex exec` runs on a local PyTorch repository.
- Baseline mode: grep/sed/cat style retrieval.
- cgrep mode: cgrep-based retrieval commands.
- Primary metric: Codex provider-reported billable tokens (`input - cached_input + output`).

## Environment

- OS: `macOS-26.2-arm64-arm-64bit`
- Python: `3.12.4`
- codex model: `gpt-5-codex`
- reasoning effort: `medium`
- runs per scenario/mode: `2`
- cgrep commit: `3606f38`
- pytorch commit: `b7abe8e3ab9`
- PyTorch files (`git ls-files`): `20437`

## Aggregate (All Cases)

| Mode | Cases | Success rate | Median billable tokens | P95 billable tokens | Median total tokens | Median duration (ms) | Median commands |
|---|---:|---:|---:|---:|---:|---:|---:|
| `baseline` | 12 | 100.0% | 9466 | 12801 | 29046 | 13145.0 | 2.0 |
| `cgrep` | 12 | 66.7% | 4798 | 35595 | 22392 | 11328.5 | 2.0 |

- Total billable tokens (baseline): **104,152**
- Total billable tokens (cgrep): **126,269**
- Billable token reduction: **-21.2%**

## Aggregate (Success-Only)

| Mode | Successful cases | Median billable tokens | P95 billable tokens | Median duration (ms) | Median commands |
|---|---:|---:|---:|---:|---:|
| `baseline` | 12 | 9466 | 12801 | 13145.0 | 2.0 |
| `cgrep` | 8 | 2244 | 7210 | 7753.6 | 1.5 |

- Success-only total billable tokens (baseline): **104,152**
- Success-only total billable tokens (cgrep): **27,087**
- Success-only billable token reduction: **74.0%**

## Per Scenario

| Run | Scenario | Mode | Success | Billable tokens | Total tokens | Duration (ms) | Commands |
|---:|---|---|---|---:|---:|---:|---:|
| 1 | `autograd_evaluate_function` | `baseline` | yes | 7,133 | 33,501 | 16382.6 | 3 |
| 1 | `autograd_evaluate_function` | `cgrep` | no | 7,199 | 39,327 | 15109.6 | 4 |
| 1 | `tensor_iterator_impl` | `baseline` | yes | 13,446 | 29,062 | 12900.4 | 2 |
| 1 | `tensor_iterator_impl` | `cgrep` | yes | 4,218 | 22,266 | 12436.0 | 2 |
| 1 | `python_arg_parser_impl` | `baseline` | yes | 4,242 | 24,850 | 12255.8 | 2 |
| 1 | `python_arg_parser_impl` | `cgrep` | yes | 8,197 | 14,213 | 7999.5 | 1 |
| 1 | `dispatch_key_set` | `baseline` | yes | 11,757 | 29,805 | 13189.6 | 2 |
| 1 | `dispatch_key_set` | `cgrep` | no | 19,414 | 76,118 | 21230.0 | 6 |
| 1 | `cuda_graph` | `baseline` | yes | 4,745 | 16,777 | 6236.0 | 1 |
| 1 | `cuda_graph` | `cgrep` | yes | 1,356 | 14,028 | 4910.3 | 1 |
| 1 | `addmm_path` | `baseline` | yes | 11,780 | 29,828 | 16210.3 | 2 |
| 1 | `addmm_path` | `cgrep` | yes | 5,378 | 23,426 | 18067.7 | 2 |
| 2 | `autograd_evaluate_function` | `cgrep` | yes | 2,192 | 21,520 | 7507.7 | 2 |
| 2 | `autograd_evaluate_function` | `baseline` | yes | 3,231 | 23,327 | 10216.0 | 2 |
| 2 | `tensor_iterator_impl` | `cgrep` | no | 29,385 | 124,489 | 36850.7 | 10 |
| 2 | `tensor_iterator_impl` | `baseline` | yes | 10,982 | 29,030 | 13100.4 | 2 |
| 2 | `python_arg_parser_impl` | `cgrep` | yes | 1,358 | 14,158 | 4448.3 | 1 |
| 2 | `python_arg_parser_impl` | `baseline` | yes | 7,949 | 25,997 | 18267.6 | 2 |
| 2 | `dispatch_key_set` | `cgrep` | yes | 2,295 | 22,519 | 10221.0 | 2 |
| 2 | `dispatch_key_set` | `baseline` | yes | 12,016 | 30,064 | 15519.6 | 2 |
| 2 | `cuda_graph` | `cgrep` | yes | 2,093 | 14,125 | 6014.5 | 1 |
| 2 | `cuda_graph` | `baseline` | yes | 4,598 | 16,630 | 6240.6 | 1 |
| 2 | `addmm_path` | `cgrep` | no | 43,184 | 203,056 | 59441.6 | 16 |
| 2 | `addmm_path` | `baseline` | yes | 12,273 | 30,321 | 18304.4 | 2 |

## Re-run

```bash
python3 scripts/benchmark_codex_agent_efficiency.py --repo /path/to/pytorch --cgrep-bin /path/to/cgrep
```
