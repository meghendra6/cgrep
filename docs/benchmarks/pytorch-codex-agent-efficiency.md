# PyTorch Codex Agent Efficiency Benchmark

Generated: 2026-02-18T11:05:53.965690+00:00

## What This Measures

- Real `codex exec` runs on a local PyTorch repository.
- Baseline mode: grep/sed/cat style retrieval.
- cgrep mode: cgrep-based retrieval commands.
- Primary metric: Codex provider-reported billable tokens (`input - cached_input + output`).

## Environment

- OS: `macOS-26.2-arm64-arm-64bit`
- Python: `3.9.6`
- codex model: `gpt-5-codex`
- reasoning effort: `medium`
- runs per scenario/mode: `1`
- cgrep commit: `d63fb9f`
- pytorch commit: `b7abe8e3ab9`
- PyTorch files (`git ls-files`): `20437`

## Aggregate (All Cases)

| Mode | Cases | Success rate | Median billable tokens | P95 billable tokens | Median total tokens | Median duration (ms) | Median commands |
|---|---:|---:|---:|---:|---:|---:|---:|
| `baseline` | 6 | 100.0% | 14730 | 24930 | 29580 | 15009.0 | 2.0 |
| `cgrep` | 6 | 100.0% | 2667 | 7159 | 16190 | 7513.5 | 1.0 |

- Total billable tokens (baseline): **89,764**
- Total billable tokens (cgrep): **21,092**
- Billable token reduction: **76.5%**

## Per Scenario

| Run | Scenario | Mode | Success | Billable tokens | Total tokens | Duration (ms) | Commands |
|---:|---|---|---|---:|---:|---:|---:|
| 1 | `autograd_evaluate_function` | `baseline` | yes | 3,135 | 26,815 | 14385.9 | 2 |
| 1 | `autograd_evaluate_function` | `cgrep` | yes | 2,065 | 16,145 | 5030.0 | 1 |
| 1 | `tensor_iterator_impl` | `baseline` | yes | 9,792 | 30,912 | 19237.6 | 2 |
| 1 | `tensor_iterator_impl` | `cgrep` | yes | 3,351 | 24,471 | 8432.3 | 2 |
| 1 | `python_arg_parser_impl` | `baseline` | yes | 21,208 | 28,248 | 15632.0 | 2 |
| 1 | `python_arg_parser_impl` | `cgrep` | yes | 1,935 | 16,015 | 5932.1 | 1 |
| 1 | `dispatch_key_set` | `baseline` | yes | 18,299 | 32,379 | 13301.5 | 2 |
| 1 | `dispatch_key_set` | `cgrep` | yes | 8,428 | 16,236 | 6594.7 | 1 |
| 1 | `cuda_graph` | `baseline` | yes | 11,160 | 18,840 | 6369.9 | 1 |
| 1 | `cuda_graph` | `cgrep` | yes | 2,044 | 16,124 | 8684.6 | 1 |
| 1 | `addmm_path` | `baseline` | yes | 26,170 | 33,210 | 19421.8 | 2 |
| 1 | `addmm_path` | `cgrep` | yes | 3,269 | 24,389 | 10046.9 | 2 |

## Re-run

```bash
python3 scripts/benchmark_codex_agent_efficiency.py --repo /path/to/pytorch --cgrep-bin /path/to/cgrep
```
