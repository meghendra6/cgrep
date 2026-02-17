# PyTorch Codex Agent Efficiency Benchmark

Generated: 2026-02-17T03:09:42.221263+00:00

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
- cgrep commit: `c4a5cdd`
- pytorch commit: `b7abe8e3ab9`
- PyTorch files (`git ls-files`): `20437`

## Aggregate (All Cases)

| Mode | Cases | Success rate | Median billable tokens | P95 billable tokens | Median total tokens | Median duration (ms) | Median commands |
|---|---:|---:|---:|---:|---:|---:|---:|
| `baseline` | 12 | 100.0% | 6302 | 11110 | 26437 | 14519.6 | 2.0 |
| `cgrep` | 12 | 100.0% | 2866 | 11463 | 21802 | 9688.6 | 2.0 |

- Total billable tokens (baseline): **83,283**
- Total billable tokens (cgrep): **62,910**
- Billable token reduction: **24.5%**

## Per Scenario

| Run | Scenario | Mode | Success | Billable tokens | Total tokens | Duration (ms) | Commands |
|---:|---|---|---|---:|---:|---:|---:|
| 1 | `autograd_evaluate_function` | `baseline` | yes | 4,042 | 24,394 | 18336.5 | 2 |
| 1 | `autograd_evaluate_function` | `cgrep` | yes | 2,110 | 21,054 | 8745.9 | 2 |
| 1 | `tensor_iterator_impl` | `baseline` | yes | 7,651 | 29,795 | 14720.9 | 2 |
| 1 | `tensor_iterator_impl` | `cgrep` | yes | 9,448 | 22,248 | 8799.7 | 2 |
| 1 | `python_arg_parser_impl` | `baseline` | yes | 4,088 | 25,592 | 17009.3 | 2 |
| 1 | `python_arg_parser_impl` | `cgrep` | yes | 2,332 | 22,044 | 11907.0 | 2 |
| 1 | `dispatch_key_set` | `baseline` | yes | 8,179 | 29,811 | 14379.9 | 2 |
| 1 | `dispatch_key_set` | `cgrep` | yes | 12,026 | 24,058 | 9246.9 | 2 |
| 1 | `cuda_graph` | `baseline` | yes | 4,077 | 16,749 | 6198.8 | 1 |
| 1 | `cuda_graph` | `cgrep` | yes | 9,787 | 21,819 | 10130.2 | 2 |
| 1 | `addmm_path` | `baseline` | yes | 10,207 | 29,023 | 21858.1 | 2 |
| 1 | `addmm_path` | `cgrep` | yes | 2,762 | 21,706 | 13687.5 | 2 |
| 2 | `autograd_evaluate_function` | `cgrep` | yes | 1,277 | 20,989 | 7249.8 | 2 |
| 2 | `autograd_evaluate_function` | `baseline` | yes | 5,398 | 23,446 | 14659.2 | 2 |
| 2 | `tensor_iterator_impl` | `cgrep` | yes | 3,929 | 21,977 | 7318.9 | 2 |
| 2 | `tensor_iterator_impl` | `baseline` | yes | 10,759 | 28,807 | 13328.2 | 2 |
| 2 | `python_arg_parser_impl` | `cgrep` | yes | 2,568 | 21,384 | 10256.4 | 2 |
| 2 | `python_arg_parser_impl` | `baseline` | yes | 6,523 | 24,571 | 11860.4 | 2 |
| 2 | `dispatch_key_set` | `cgrep` | yes | 11,002 | 23,802 | 10237.1 | 2 |
| 2 | `dispatch_key_set` | `baseline` | yes | 11,538 | 27,282 | 10781.8 | 2 |
| 2 | `cuda_graph` | `cgrep` | yes | 2,969 | 21,785 | 12337.6 | 2 |
| 2 | `cuda_graph` | `baseline` | yes | 4,739 | 16,771 | 7886.6 | 1 |
| 2 | `addmm_path` | `cgrep` | yes | 2,700 | 21,516 | 8980.0 | 2 |
| 2 | `addmm_path` | `baseline` | yes | 6,082 | 27,970 | 15040.7 | 2 |

## Re-run

```bash
python3 scripts/benchmark_codex_agent_efficiency.py --repo /path/to/pytorch --cgrep-bin /path/to/cgrep
```
