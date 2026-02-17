# PyTorch Codex Agent Efficiency Benchmark

Generated: 2026-02-17T16:21:10.604715+00:00

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
- cgrep commit: `42e7d3d`
- pytorch commit: `b7abe8e3ab9`
- PyTorch files (`git ls-files`): `20437`

## Aggregate (All Cases)

| Mode | Cases | Success rate | Median billable tokens | P95 billable tokens | Median total tokens | Median duration (ms) | Median commands |
|---|---:|---:|---:|---:|---:|---:|---:|
| `baseline` | 12 | 91.7% | 11566 | 38115 | 28270 | 17543.2 | 2.0 |
| `cgrep` | 12 | 100.0% | 8589 | 17351 | 16492 | 8179.9 | 1.0 |

- Total billable tokens (baseline): **167,409**
- Total billable tokens (cgrep): **89,967**
- Billable token reduction: **46.3%**

## Per Scenario

| Run | Scenario | Mode | Success | Billable tokens | Total tokens | Duration (ms) | Commands |
|---:|---|---|---|---:|---:|---:|---:|
| 1 | `autograd_evaluate_function` | `baseline` | yes | 4,705 | 26,977 | 16375.2 | 2 |
| 1 | `autograd_evaluate_function` | `cgrep` | yes | 1,845 | 15,925 | 5185.4 | 1 |
| 1 | `tensor_iterator_impl` | `baseline` | yes | 11,868 | 18,908 | 11832.5 | 1 |
| 1 | `tensor_iterator_impl` | `cgrep` | yes | 2,508 | 24,396 | 7718.4 | 2 |
| 1 | `python_arg_parser_impl` | `baseline` | yes | 11,593 | 42,569 | 51073.6 | 3 |
| 1 | `python_arg_parser_impl` | `cgrep` | yes | 8,159 | 15,967 | 7919.3 | 1 |
| 1 | `dispatch_key_set` | `baseline` | yes | 17,994 | 32,714 | 14408.5 | 2 |
| 1 | `dispatch_key_set` | `cgrep` | yes | 16,528 | 16,528 | 8440.5 | 1 |
| 1 | `cuda_graph` | `baseline` | yes | 4,714 | 18,794 | 7157.7 | 1 |
| 1 | `cuda_graph` | `cgrep` | yes | 1,479 | 16,455 | 8450.2 | 1 |
| 1 | `addmm_path` | `baseline` | yes | 20,257 | 34,337 | 22479.3 | 2 |
| 1 | `addmm_path` | `cgrep` | yes | 1,631 | 24,543 | 7519.8 | 2 |
| 2 | `autograd_evaluate_function` | `cgrep` | yes | 1,960 | 16,040 | 6422.9 | 1 |
| 2 | `autograd_evaluate_function` | `baseline` | yes | 4,539 | 26,299 | 11440.8 | 2 |
| 2 | `tensor_iterator_impl` | `cgrep` | yes | 9,570 | 24,418 | 10077.3 | 2 |
| 2 | `tensor_iterator_impl` | `baseline` | yes | 11,540 | 32,660 | 18711.2 | 2 |
| 2 | `python_arg_parser_impl` | `cgrep` | yes | 18,357 | 25,397 | 11736.9 | 2 |
| 2 | `python_arg_parser_impl` | `baseline` | yes | 15,483 | 29,563 | 25813.2 | 2 |
| 2 | `dispatch_key_set` | `cgrep` | yes | 9,019 | 16,059 | 12646.0 | 1 |
| 2 | `dispatch_key_set` | `baseline` | no | 0 | 0 | 360010.8 | 0 |
| 2 | `cuda_graph` | `cgrep` | yes | 9,294 | 16,334 | 7284.9 | 1 |
| 2 | `cuda_graph` | `baseline` | yes | 4,774 | 18,854 | 9841.7 | 1 |
| 2 | `addmm_path` | `cgrep` | yes | 9,617 | 24,849 | 10797.1 | 2 |
| 2 | `addmm_path` | `baseline` | yes | 59,942 | 139,558 | 41361.0 | 8 |

## Re-run

```bash
python3 scripts/benchmark_codex_agent_efficiency.py --repo /path/to/pytorch --cgrep-bin /path/to/cgrep
```
