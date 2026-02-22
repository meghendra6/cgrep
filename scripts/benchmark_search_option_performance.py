#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0

"""Benchmark cgrep `search` option performance across practical scenarios.

This script benchmarks latency + payload size for representative `search`
option combinations on a target repository (for example, PyTorch).
"""

from __future__ import annotations

import argparse
import dataclasses
import datetime as dt
import json
import math
import os
import platform
import statistics
import subprocess
import time
from pathlib import Path
from typing import Any


@dataclasses.dataclass(frozen=True)
class Case:
    id: str
    scenario: str
    description: str
    query: str
    path: str
    args: tuple[str, ...]
    expected_markers: tuple[str, ...]
    requires_index: bool = True


CASES: list[Case] = [
    Case(
        id="default_autograd",
        scenario="autograd evaluate_function",
        description="keyword default from repo root",
        query="evaluate_function",
        path=".",
        args=(),
        expected_markers=("engine.cpp", "engine.h"),
    ),
    Case(
        id="path_scoped_autograd",
        scenario="autograd evaluate_function",
        description="keyword with explicit -p scope",
        query="evaluate_function",
        path="torch/csrc/autograd",
        args=(),
        expected_markers=("engine.cpp",),
    ),
    Case(
        id="type_cpp_parser",
        scenario="python arg parser",
        description="keyword with --type cpp",
        query="PythonArgParser",
        path="torch/csrc",
        args=("--type", "cpp"),
        expected_markers=("python_arg_parser.cpp", "python_arg_parser.h"),
    ),
    Case(
        id="glob_cpp_cuda",
        scenario="cuda graph",
        description="keyword with --glob *.cpp",
        query="CUDAGraph",
        path="torch/csrc",
        args=("--glob", "*.cpp"),
        expected_markers=("Graph.cpp",),
    ),
    Case(
        id="context_addmm",
        scenario="addmm call path",
        description="keyword with context lines",
        query="addmm_impl_cpu_",
        path="aten/src/ATen/native",
        args=("-C", "2"),
        expected_markers=("LinearAlgebra.cpp",),
    ),
    Case(
        id="budget_tight_dispatch",
        scenario="dispatch key set",
        description="keyword with tight budget",
        query="DispatchKeySet",
        path="c10/core",
        args=("-B", "tight"),
        expected_markers=("DispatchKeySet.h",),
    ),
    Case(
        id="budget_full_dispatch",
        scenario="dispatch key set",
        description="keyword with full budget",
        query="DispatchKeySet",
        path="c10/core",
        args=("-B", "full"),
        expected_markers=("DispatchKeySet.h",),
    ),
    Case(
        id="profile_fast_dispatch",
        scenario="dispatch key set",
        description="keyword with fast profile",
        query="DispatchKeySet",
        path="c10/core",
        args=("-P", "fast"),
        expected_markers=("DispatchKeySet.h",),
    ),
    Case(
        id="payload_compact_dispatch",
        scenario="dispatch key set",
        description="keyword with payload-focused options",
        query="DispatchKeySet",
        path="c10/core",
        args=("--path-alias", "--dedupe-context", "--suppress-boilerplate"),
        expected_markers=("DispatchKeySet.h",),
    ),
    Case(
        id="fuzzy_tensor_iterator",
        scenario="tensor iterator symbol lookup",
        description="fuzzy enabled on symbol lookup",
        query="TensorIterator",
        path="aten/src/ATen",
        args=("--fuzzy",),
        expected_markers=("TensorIterator",),
    ),
    Case(
        id="scan_no_index_autograd",
        scenario="autograd evaluate_function",
        description="scan mode via --no-index",
        query="evaluate_function",
        path="torch/csrc/autograd",
        args=("--no-index",),
        expected_markers=("engine.cpp",),
        requires_index=False,
    ),
    Case(
        id="scan_regex_addmm",
        scenario="addmm regex search",
        description="scan regex for addmm(",
        query=r"addmm\(",
        path="aten/src/ATen/native",
        args=("--regex", "--no-index"),
        expected_markers=("LinearAlgebra.cpp",),
        requires_index=False,
    ),
]


def run_cmd(cmd: list[str], cwd: Path, timeout_s: int) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
        timeout=timeout_s,
    )


def estimate_tokens(text: str) -> int:
    return (len(text) + 3) // 4


def percentile(values: list[float], p: float) -> float:
    if not values:
        return 0.0
    if p <= 0:
        return min(values)
    if p >= 100:
        return max(values)
    arr = sorted(values)
    pos = (len(arr) - 1) * (p / 100.0)
    lo = int(math.floor(pos))
    hi = int(math.ceil(pos))
    if lo == hi:
        return arr[lo]
    frac = pos - lo
    return arr[lo] * (1.0 - frac) + arr[hi] * frac


def json2_paths(payload: dict[str, Any]) -> list[str]:
    aliases = payload.get("meta", {}).get("path_aliases", {}) or {}
    out: list[str] = []
    for row in payload.get("results", []):
        raw = row.get("path")
        if not isinstance(raw, str):
            continue
        out.append(aliases.get(raw, raw))
    return out


def markers_met(payload: dict[str, Any], markers: tuple[str, ...]) -> bool:
    rows = payload.get("results", [])
    if not isinstance(rows, list) or not rows:
        return False
    chunks: list[str] = []
    for p in json2_paths(payload):
        chunks.append(p)
    for row in rows:
        if not isinstance(row, dict):
            continue
        snippet = row.get("snippet")
        if isinstance(snippet, str):
            chunks.append(snippet)
    haystack = " ".join(chunks).lower()
    return any(marker.lower() in haystack for marker in markers)


def build_search_cmd(binary: Path, case: Case, limit: int) -> list[str]:
    cmd = [
        str(binary),
        "--format",
        "json2",
        "--compact",
        "search",
        case.query,
        "-p",
        case.path,
        "--limit",
        str(limit),
    ]
    cmd.extend(case.args)
    return cmd


def run_case(
    *,
    repo_path: Path,
    binary: Path,
    case: Case,
    runs: int,
    warmup: int,
    limit: int,
    timeout_s: int,
) -> dict[str, Any]:
    latencies: list[float] = []
    payload_tokens: list[int] = []
    result_counts: list[int] = []
    success_count = 0
    errors: list[str] = []

    total = warmup + runs
    for idx in range(total):
        cmd = build_search_cmd(binary, case, limit)
        t0 = time.perf_counter()
        proc = run_cmd(cmd, cwd=repo_path, timeout_s=timeout_s)
        elapsed_ms = (time.perf_counter() - t0) * 1000.0
        if proc.returncode != 0:
            if idx >= warmup:
                errors.append((proc.stderr.strip() or proc.stdout.strip())[-500:])
            continue
        try:
            payload = json.loads(proc.stdout)
        except json.JSONDecodeError:
            if idx >= warmup:
                errors.append("json parse error")
            continue

        rows = payload.get("results", [])
        row_count = len(rows) if isinstance(rows, list) else 0
        ok = markers_met(payload, case.expected_markers)
        if idx >= warmup:
            latencies.append(elapsed_ms)
            payload_tokens.append(estimate_tokens(proc.stdout))
            result_counts.append(row_count)
            if ok:
                success_count += 1

    attempted = len(latencies)
    success_rate = (success_count / attempted) * 100.0 if attempted > 0 else 0.0
    return {
        "id": case.id,
        "scenario": case.scenario,
        "description": case.description,
        "query": case.query,
        "path": case.path,
        "args": list(case.args),
        "requires_index": case.requires_index,
        "attempted_runs": attempted,
        "success_count": success_count,
        "success_rate_percent": round(success_rate, 2),
        "latency_ms_p50": round(statistics.median(latencies), 2) if latencies else 0.0,
        "latency_ms_p95": round(percentile(latencies, 95.0), 2) if latencies else 0.0,
        "payload_tokens_p50": round(statistics.median(payload_tokens), 2) if payload_tokens else 0.0,
        "results_p50": round(statistics.median(result_counts), 2) if result_counts else 0.0,
        "errors": errors,
    }


def git_rev(path: Path) -> str:
    proc = run_cmd(["git", "rev-parse", "--short", "HEAD"], cwd=path, timeout_s=30)
    if proc.returncode != 0:
        return "unknown"
    return proc.stdout.strip() or "unknown"


def count_files(path: Path) -> int:
    proc = run_cmd(["git", "ls-files"], cwd=path, timeout_s=180)
    if proc.returncode != 0:
        return 0
    out = proc.stdout.strip()
    return 0 if not out else out.count("\n") + 1


def render_markdown(payload: dict[str, Any]) -> str:
    by_id = {row["id"]: row for row in payload["results"]}

    def pct_delta(base: str, target: str, key: str) -> float:
        b = float(by_id[base][key])
        t = float(by_id[target][key])
        if b <= 0:
            return 0.0
        return ((t - b) / b) * 100.0

    lines: list[str] = []
    lines.append("# PyTorch Search Option Performance Benchmark")
    lines.append("")
    lines.append(f"Generated: {payload['generated_at_utc']}")
    lines.append("")
    lines.append("> Snapshot note: values depend on repository state, cache state, and hardware.")
    lines.append("> Re-run with the same command before release/perf decisions.")
    lines.append("")
    lines.append("## Scope")
    lines.append("")
    lines.append("- Measures `cgrep search` latency and payload size for practical option/scenario pairs.")
    lines.append("- Includes indexed keyword paths and scan-mode (`--no-index`, `--regex`) paths.")
    lines.append("- Success means expected marker evidence appears in structured JSON results.")
    lines.append("")
    lines.append("## Environment")
    lines.append("")
    env = payload["environment"]
    cfg = payload["config"]
    lines.append(f"- OS: `{env['os']}`")
    lines.append(f"- Python: `{env['python']}`")
    lines.append(f"- cgrep commit: `{env['cgrep_commit']}`")
    lines.append(f"- pytorch commit: `{env['pytorch_commit']}`")
    lines.append(f"- PyTorch files (`git ls-files`): `{env['pytorch_file_count']}`")
    lines.append(f"- runs per case: `{cfg['runs']}` (warmup `{cfg['warmup']}`)")
    lines.append(f"- limit per search: `{cfg['limit']}`")
    lines.append(f"- index build time: `{payload['index']['duration_ms']:.2f} ms`")
    lines.append("")
    lines.append("## Case Results")
    lines.append("")
    lines.append("| Case | Scenario | Options | Success | P50 latency (ms) | P95 latency (ms) | P50 payload tokens | P50 results |")
    lines.append("|---|---|---|---:|---:|---:|---:|---:|")
    for row in payload["results"]:
        opt = " ".join(row["args"]) if row["args"] else "(default)"
        lines.append(
            f"| `{row['id']}` | {row['scenario']} | `{opt}` | {row['success_rate_percent']:.1f}% | "
            f"{row['latency_ms_p50']:.2f} | {row['latency_ms_p95']:.2f} | "
            f"{row['payload_tokens_p50']:.0f} | {row['results_p50']:.0f} |"
        )
    lines.append("")
    lines.append("## Highlights")
    lines.append("")
    root_to_scoped = pct_delta("default_autograd", "path_scoped_autograd", "latency_ms_p50")
    idx_to_scan = pct_delta("path_scoped_autograd", "scan_no_index_autograd", "latency_ms_p50")
    scan_to_regex = pct_delta("scan_no_index_autograd", "scan_regex_addmm", "latency_ms_p50")
    tight_to_full = pct_delta("budget_tight_dispatch", "budget_full_dispatch", "payload_tokens_p50")
    tight_to_fast = pct_delta("budget_tight_dispatch", "profile_fast_dispatch", "latency_ms_p50")
    lines.append(f"- Root search -> scoped search latency change: **{root_to_scoped:.1f}%** (`default_autograd` vs `path_scoped_autograd`).")
    lines.append(f"- Indexed scoped -> `--no-index` scan latency change: **{idx_to_scan:.1f}%**.")
    lines.append(f"- Scan -> regex scan latency change: **{scan_to_regex:.1f}%**.")
    lines.append(f"- `-B tight` -> `-B full` payload token change: **{tight_to_full:.1f}%**.")
    lines.append(f"- `-B tight` -> `-P fast` latency change: **{tight_to_fast:.1f}%**.")
    lines.append("")
    lines.append("## Re-run")
    lines.append("")
    lines.append("```bash")
    lines.append(
        "python3 scripts/benchmark_search_option_performance.py "
        "--repo /path/to/pytorch --cgrep-bin /path/to/cgrep"
    )
    lines.append("```")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description="Benchmark cgrep search options across scenarios")
    parser.add_argument("--repo", required=True, help="Path to local target repository (e.g., PyTorch)")
    parser.add_argument("--cgrep-bin", default="target/release/cgrep", help="Path to cgrep binary")
    parser.add_argument("--runs", type=int, default=5, help="Measured runs per case")
    parser.add_argument("--warmup", type=int, default=1, help="Warmup runs per case")
    parser.add_argument("--limit", type=int, default=20, help="Result limit per search")
    parser.add_argument("--timeout", type=int, default=120, help="Per search timeout seconds")
    parser.add_argument("--skip-index", action="store_true", help="Skip index build before benchmark")
    parser.add_argument(
        "--json-out",
        default="local/benchmarks/pytorch-search-options-performance.json",
        help="JSON output path",
    )
    parser.add_argument(
        "--md-out",
        default="docs/benchmarks/pytorch-search-options-performance.md",
        help="Markdown output path",
    )
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parents[1]
    repo_path = Path(args.repo).resolve()
    cgrep_bin = Path(args.cgrep_bin)
    if not cgrep_bin.is_absolute():
        cgrep_bin = (repo_root / cgrep_bin).resolve()

    if not repo_path.exists() or not (repo_path / ".git").exists():
        raise SystemExit(f"Invalid --repo path: {repo_path}")
    if not cgrep_bin.exists():
        raise SystemExit(f"cgrep binary not found: {cgrep_bin}")
    if args.runs <= 0:
        raise SystemExit("--runs must be > 0")
    if args.warmup < 0:
        raise SystemExit("--warmup must be >= 0")
    if args.limit <= 0:
        raise SystemExit("--limit must be > 0")

    index_duration_ms = 0.0
    index_returncode = 0
    index_stderr_tail = ""
    if not args.skip_index:
        t0 = time.perf_counter()
        proc = run_cmd(
            [str(cgrep_bin), "index", "--embeddings", "off"],
            cwd=repo_path,
            timeout_s=max(args.timeout, 1800),
        )
        index_duration_ms = (time.perf_counter() - t0) * 1000.0
        index_returncode = proc.returncode
        index_stderr_tail = proc.stderr[-1000:]
        if proc.returncode != 0:
            raise SystemExit(
                "index failed\n"
                f"stdout:\n{proc.stdout[-2000:]}\n"
                f"stderr:\n{proc.stderr[-2000:]}"
            )

    rows: list[dict[str, Any]] = []
    for case in CASES:
        row = run_case(
            repo_path=repo_path,
            binary=cgrep_bin,
            case=case,
            runs=args.runs,
            warmup=args.warmup,
            limit=args.limit,
            timeout_s=args.timeout,
        )
        rows.append(row)

    payload = {
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "environment": {
            "os": platform.platform(),
            "python": platform.python_version(),
            "cgrep_commit": git_rev(repo_root),
            "pytorch_commit": git_rev(repo_path),
            "pytorch_file_count": count_files(repo_path),
        },
        "config": {
            "repo": str(repo_path),
            "cgrep_bin": str(cgrep_bin),
            "runs": args.runs,
            "warmup": args.warmup,
            "limit": args.limit,
            "timeout_s": args.timeout,
            "case_count": len(CASES),
            "skip_index": args.skip_index,
        },
        "index": {
            "returncode": index_returncode,
            "duration_ms": round(index_duration_ms, 2),
            "stderr_tail": index_stderr_tail,
        },
        "results": rows,
    }

    json_out = (repo_root / args.json_out).resolve()
    md_out = (repo_root / args.md_out).resolve()
    json_out.parent.mkdir(parents=True, exist_ok=True)
    md_out.parent.mkdir(parents=True, exist_ok=True)
    json_out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    md_out.write_text(render_markdown(payload), encoding="utf-8")

    print(f"JSON: {json_out}")
    print(f"MD:   {md_out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
