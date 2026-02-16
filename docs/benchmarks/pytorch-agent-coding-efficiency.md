# PyTorch AI Agent Coding Readiness Benchmark

Generated: 2026-02-16T11:58:18.234788+00:00

## What This Measures

This benchmark reuses the same PyTorch scenarios and evaluates them as **coding tasks**.
A task is considered "ready" when retrieved context satisfies all completion marker groups (enough evidence to start patching).

Workflows compared:
1. **Baseline:** `grep` locate + incremental snippet expansion tiers
2. **cgrep:** `agent locate` + incremental `agent expand` ID tiers

## Environment

- OS: `macOS-26.2-arm64-arm-64bit`
- cgrep commit: `b2d160d`
- pytorch commit: `b7abe8e3ab9`
- PyTorch files (`git ls-files`): `20437`
- Tokenizer: `tiktoken:cl100k_base`

## Task Matrix

| Scenario | Coding task | Completion markers |
|---|---|---|
| `autograd_evaluate_function` | Patch autograd evaluate_function flow and verify the implementation file + autograd context. | evaluate_function; engine.cpp / autograd/ |
| `tensor_iterator_impl` | Prepare a TensorIterator behavior change by locating the core declaration and implementation paths. | TensorIterator; TensorIterator.h / TensorIterator.cpp |
| `python_arg_parser_impl` | Implement a parser-related fix by gathering PythonArgParser definition and source implementation. | PythonArgParser; python_arg_parser.h / python_arg_parser.cpp |
| `dispatch_key_set` | Refactor DispatchKeySet logic with confidence by finding its representation and core references. | DispatchKeySet; DispatchKeySet.h / c10/core/ |
| `cuda_graph` | Make a CUDAGraph code-path update by collecting implementation and CUDA path context. | CUDAGraph; CUDAGraph.cpp / cuda/ |
| `addmm_path` | Modify addmm behavior by locating native implementation and addmm_out call path. | addmm( / addmm; LinearAlgebra.cpp / addmm_out / native/ |

## Results

| Task | Baseline ready | cgrep ready | Baseline attempts | cgrep attempts | Baseline tokens-to-ready | cgrep tokens-to-ready | Reduction | Baseline latency (ms) | cgrep latency (ms) |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|
| autograd_evaluate_function | yes | yes | 1 | 1 | 7,681 | 939 | 87.8% | 1853.05 | 20.75 |
| tensor_iterator_impl | yes | yes | 1 | 1 | 43,265 | 1,026 | 97.6% | 1151.19 | 20.03 |
| python_arg_parser_impl | yes | yes | 1 | 1 | 6,742 | 1,001 | 85.2% | 1081.20 | 21.84 |
| dispatch_key_set | yes | yes | 1 | 1 | 43,738 | 1,028 | 97.6% | 1034.56 | 21.78 |
| cuda_graph | yes | yes | 1 | 1 | 11,361 | 1,018 | 91.0% | 1047.23 | 21.18 |
| addmm_path | yes | yes | 1 | 1 | 15,692 | 1,148 | 92.7% | 1538.34 | 21.11 |

## Aggregate

- Tasks ready (baseline): **6/6** (100.0%)
- Tasks ready (cgrep): **6/6** (100.0%)
- Baseline total tokens-to-ready: **128,479**
- cgrep total tokens-to-ready: **6,160**
- Token reduction to readiness: **95.2%**
- Baseline avg tokens per task: **21413**
- cgrep avg tokens per task: **1027**
- Baseline avg attempts per task: **1.00**
- cgrep avg attempts per task: **1.00**
- Baseline avg latency per task: **1284.26ms**
- cgrep avg latency per task: **21.12ms**

## Re-run

```bash
python3 scripts/benchmark_agent_token_efficiency.py --repo /path/to/pytorch
```
