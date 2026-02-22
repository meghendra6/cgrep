#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0

"""Benchmark AI-agent token efficiency to scenario completion on a local PyTorch checkout.

Compares two workflows for common coding-agent tasks:
1) Baseline (no cgrep): grep locate + incremental snippet expansion
2) cgrep workflow: agent locate + incremental agent expand

Outputs machine-readable JSON and a Markdown report.
"""

from __future__ import annotations

import argparse
import dataclasses
import datetime as dt
import json
import math
import os
import platform
import subprocess
import time
from pathlib import Path
from typing import Any


@dataclasses.dataclass(frozen=True)
class Scenario:
    id: str
    objective: str
    coding_task: str
    grep_pattern: str
    cgrep_query: str
    completion_groups: tuple[tuple[str, ...], ...]


@dataclasses.dataclass
class CommandRun:
    command: list[str]
    returncode: int
    duration_ms: float
    stdout: str
    stderr: str


SCENARIOS: list[Scenario] = [
    Scenario(
        id="autograd_evaluate_function",
        objective="Find where autograd engine evaluate_function is implemented and inspected.",
        coding_task="Patch autograd evaluate_function flow and verify the implementation file + autograd context.",
        grep_pattern="evaluate_function",
        cgrep_query="evaluate_function engine.cpp autograd",
        completion_groups=(("evaluate_function",), ("engine.cpp", "autograd/")),
    ),
    Scenario(
        id="tensor_iterator_impl",
        objective="Find TensorIterator definition and major implementation usage points.",
        coding_task="Prepare a TensorIterator behavior change by locating the core declaration and implementation paths.",
        grep_pattern="TensorIterator",
        cgrep_query="TensorIterator TensorIterator.h TensorIterator.cpp",
        completion_groups=(("TensorIterator",), ("TensorIterator.h", "TensorIterator.cpp")),
    ),
    Scenario(
        id="python_arg_parser_impl",
        objective="Locate PythonArgParser implementation and usage points.",
        coding_task="Implement a parser-related fix by gathering PythonArgParser definition and source implementation.",
        grep_pattern="PythonArgParser",
        cgrep_query="PythonArgParser python_arg_parser.h python_arg_parser.cpp",
        completion_groups=(
            ("PythonArgParser",),
            ("python_arg_parser.h", "python_arg_parser.cpp"),
        ),
    ),
    Scenario(
        id="dispatch_key_set",
        objective="Understand DispatchKeySet representation and references.",
        coding_task="Refactor DispatchKeySet logic with confidence by finding its representation and core references.",
        grep_pattern="DispatchKeySet",
        cgrep_query="where DispatchKeySet is defined and referenced",
        completion_groups=(("DispatchKeySet",), ("DispatchKeySet.h", "c10/core/")),
    ),
    Scenario(
        id="cuda_graph",
        objective="Locate CUDAGraph implementation-related code quickly.",
        coding_task="Make a CUDAGraph code-path update by collecting implementation and CUDA path context.",
        grep_pattern="CUDAGraph",
        cgrep_query="where CUDAGraph is implemented",
        completion_groups=(("CUDAGraph",), ("CUDAGraph.cpp", "cuda/")),
    ),
    Scenario(
        id="addmm_path",
        objective="Find addmm implementation and call sites.",
        coding_task="Modify addmm behavior by locating native implementation and addmm_out call path.",
        grep_pattern=r"addmm\(",
        cgrep_query="addmm LinearAlgebra.cpp addmm_out",
        completion_groups=(("addmm(", "addmm"), ("LinearAlgebra.cpp", "addmm_out", "native/")),
    ),
]


def run_cmd(cmd: list[str], cwd: Path, timeout_s: int = 300) -> CommandRun:
    t0 = time.perf_counter()
    proc = subprocess.run(
        cmd,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
        timeout=timeout_s,
    )
    duration_ms = (time.perf_counter() - t0) * 1000.0
    return CommandRun(
        command=cmd,
        returncode=proc.returncode,
        duration_ms=duration_ms,
        stdout=proc.stdout,
        stderr=proc.stderr,
    )


def load_tokenizer() -> tuple[str, Any]:
    try:
        import tiktoken

        enc = tiktoken.get_encoding("cl100k_base")

        def _count_tokens(text: str) -> int:
            return len(enc.encode(text, disallowed_special=()))

        return "tiktoken:cl100k_base", _count_tokens
    except Exception:

        def _approx_tokens(text: str) -> int:
            # Approximation frequently used for English-heavy source context.
            return int(math.ceil(len(text.encode("utf-8")) / 4.0))

        return "approx:bytes_div_4", _approx_tokens


def parse_grep_matches(stdout: str, max_matches: int) -> list[tuple[str, int, str]]:
    out: list[tuple[str, int, str]] = []
    for line in stdout.splitlines():
        parts = line.split(":", 2)
        if len(parts) < 3:
            continue
        path, line_no, snippet = parts
        try:
            n = int(line_no)
        except ValueError:
            continue
        out.append((path, n, snippet))
        if len(out) >= max_matches:
            break
    return out


def safe_read_lines(path: Path) -> list[str] | None:
    try:
        return path.read_text(encoding="utf-8", errors="ignore").splitlines()
    except Exception:
        return None


def parse_tiers(raw: str) -> list[int]:
    values: list[int] = []
    seen: set[int] = set()
    for chunk in raw.split(","):
        item = chunk.strip()
        if not item:
            continue
        n = int(item)
        if n <= 0:
            raise ValueError(f"tier value must be > 0 (got {n})")
        if n not in seen:
            seen.add(n)
            values.append(n)
    if not values:
        raise ValueError("at least one tier value is required")
    return values


def missing_completion_groups(text: str, groups: tuple[tuple[str, ...], ...]) -> list[str]:
    normalized = text.lower()
    missing: list[str] = []
    for group in groups:
        if not any(marker.lower() in normalized for marker in group):
            missing.append(" | ".join(group))
    return missing


def prepare_baseline_snippets(
    repo_path: Path,
    grep_run: CommandRun,
    grep_max_matches: int,
    max_windows_per_file: int,
    context_lines: int,
    max_unique_files: int,
) -> tuple[list[str], dict[str, str], dict[str, int], dict[str, Any]]:
    matches = parse_grep_matches(grep_run.stdout, max_matches=grep_max_matches)

    ordered_files: list[str] = []
    lines_by_file: dict[str, list[int]] = {}
    for rel, line_no, _snippet in matches:
        if rel not in lines_by_file:
            if len(ordered_files) >= max_unique_files:
                continue
            ordered_files.append(rel)
            lines_by_file[rel] = []
        if len(lines_by_file[rel]) < max_windows_per_file:
            lines_by_file[rel].append(line_no)

    sections_by_file: dict[str, str] = {}
    windows_by_file: dict[str, int] = {}
    for rel in ordered_files:
        abs_path = repo_path / rel
        lines = safe_read_lines(abs_path)
        if not lines:
            sections_by_file[rel] = ""
            windows_by_file[rel] = 0
            continue
        sections: list[str] = []
        snippet_count = 0
        for line_no in lines_by_file.get(rel, []):
            start = max(1, line_no - context_lines)
            end = min(len(lines), line_no + context_lines)
            body = "\n".join(lines[start - 1 : end])
            sections.append(f"--- {rel}:{start}-{end} ---")
            sections.append(body)
            sections.append("")
            snippet_count += 1
        sections_by_file[rel] = "\n".join(sections)
        windows_by_file[rel] = snippet_count

    prep_meta = {
        "grep_match_count": len(matches),
        "unique_files_available": len(ordered_files),
    }
    return ordered_files, sections_by_file, windows_by_file, prep_meta


def run_cgrep_locate(
    cgrep_bin: Path,
    repo_path: Path,
    scenario: Scenario,
    locate_limit: int,
    max_ids: int,
) -> tuple[CommandRun, list[str]]:
    locate_cmd = [
        str(cgrep_bin),
        "agent",
        "locate",
        scenario.cgrep_query,
        "--format",
        "json2",
        "--compact",
        "--budget",
        "tight",
        "--mode",
        "keyword",
        "--limit",
        str(locate_limit),
    ]
    locate_run = run_cmd(locate_cmd, cwd=repo_path)
    ids = extract_ids_from_locate(locate_run.stdout, max_ids=max_ids)
    return locate_run, ids


def run_cgrep_expand(
    cgrep_bin: Path,
    repo_path: Path,
    ids: list[str],
    expand_context: int,
) -> CommandRun | None:
    if not ids:
        return None
    expand_cmd = [
        str(cgrep_bin),
        "agent",
        "expand",
        "--format",
        "json2",
        "--compact",
        "-C",
        str(expand_context),
    ]
    for rid in ids:
        expand_cmd.extend(["--id", rid])
    return run_cmd(expand_cmd, cwd=repo_path)


def extract_ids_from_locate(stdout: str, max_ids: int) -> list[str]:
    try:
        obj = json.loads(stdout)
    except json.JSONDecodeError:
        return []

    results: list[dict[str, Any]] = []
    if isinstance(obj, dict) and isinstance(obj.get("results"), list):
        results = [r for r in obj["results"] if isinstance(r, dict)]
    elif isinstance(obj, list):
        results = [r for r in obj if isinstance(r, dict)]

    ids: list[str] = []
    for row in results:
        rid = row.get("id")
        if isinstance(rid, str) and rid:
            ids.append(rid)
        if len(ids) >= max_ids:
            break
    return ids


def git_rev(path: Path) -> str:
    proc = subprocess.run(
        ["git", "rev-parse", "--short", "HEAD"],
        cwd=path,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    if proc.returncode == 0:
        return proc.stdout.strip()
    return "unknown"


def count_files(path: Path) -> int:
    proc = subprocess.run(
        ["git", "ls-files"],
        cwd=path,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        return 0
    out = proc.stdout.strip()
    return 0 if not out else out.count("\n") + 1


def run_index(cgrep_bin: Path, repo_path: Path, timeout_s: int) -> CommandRun:
    cmd = [str(cgrep_bin), "index", "--embeddings", "off", "--force"]
    return run_cmd(cmd, cwd=repo_path, timeout_s=timeout_s)


def render_markdown(payload: dict[str, Any]) -> str:
    s = payload["summary"]
    rows = payload["scenario_results"]

    lines: list[str] = []
    lines.append("# PyTorch AI Agent Token Efficiency Benchmark")
    lines.append("")
    lines.append(f"Generated: {payload['generated_at_utc']}")
    lines.append("")
    lines.append("> Snapshot note: benchmark outputs vary by repository state and model behavior.")
    lines.append("> Compare trends over repeated runs rather than relying on one run.")
    lines.append("")
    lines.append("## What This Measures")
    lines.append("")
    lines.append("1. **Baseline (without cgrep):** `grep` locate + incremental snippet expansion tiers.")
    lines.append("2. **With cgrep:** `agent locate` once + incremental `agent expand` ID tiers.")
    lines.append("3. **Completion rule:** scenario is complete when each marker-group has at least one match in cumulative tool outputs.")
    lines.append("4. **Primary metric:** cumulative tokens consumed until completion (`tokens-to-complete`).")
    lines.append("5. **Tokenizer:** OpenAI `cl100k_base` when available (fallback: byte/4 approximation).")
    lines.append("")
    lines.append("## Environment")
    lines.append("")
    lines.append(f"- OS: `{payload['environment']['os']}`")
    lines.append(f"- cgrep commit: `{payload['environment']['cgrep_commit']}`")
    lines.append(f"- pytorch commit: `{payload['environment']['pytorch_commit']}`")
    lines.append(f"- PyTorch files (`git ls-files`): `{payload['environment']['pytorch_file_count']}`")
    lines.append(f"- Tokenizer: `{payload['config']['tokenizer']}`")
    lines.append(f"- Baseline file tiers: `{payload['config']['baseline_file_tiers']}`")
    lines.append(f"- cgrep expand tiers: `{payload['config']['cgrep_expand_tiers']}`")
    lines.append("")
    lines.append("## Results")
    lines.append("")
    lines.append("| Scenario | Representative coding task | Baseline done | cgrep done | Baseline attempts | cgrep attempts | Baseline tokens-to-complete | cgrep tokens-to-complete | Reduction | Baseline latency (ms) | cgrep latency (ms) |")
    lines.append("|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|")
    for r in rows:
        reduction = f"{r['token_reduction_percent_to_completion']:.1f}%"
        lines.append(
            f"| {r['objective']} | "
            f"{r['coding_task']} | "
            f"{'yes' if r['baseline_completed'] else 'no'} | "
            f"{'yes' if r['cgrep_completed'] else 'no'} | "
            f"{r['baseline_attempts']} | {r['cgrep_attempts']} | "
            f"{r['baseline_tokens_to_completion']:,} | {r['cgrep_tokens_to_completion']:,} | "
            f"{reduction} | "
            f"{r['baseline_latency_ms_to_completion']:.2f} | {r['cgrep_latency_ms_to_completion']:.2f} |"
        )
    lines.append("")
    lines.append("## Aggregate")
    lines.append("")
    lines.append(f"- One-time index build: **{s['index_build_ms']/1000.0:.2f}s**")
    lines.append(f"- Scenarios completed (baseline): **{s['baseline_completed_scenarios']}/{s['scenario_count']}**")
    lines.append(f"- Scenarios completed (cgrep): **{s['cgrep_completed_scenarios']}/{s['scenario_count']}**")
    lines.append(f"- Baseline tokens-to-complete (total): **{s['baseline_total_tokens_to_completion']:,}**")
    lines.append(f"- cgrep tokens-to-complete (total): **{s['cgrep_total_tokens_to_completion']:,}**")
    lines.append(f"- Token reduction (to completion): **{s['token_reduction_percent_to_completion']:.1f}%**")
    lines.append(f"- Token compression ratio (baseline/cgrep): **{s['token_compression_x_to_completion']:.2f}x**")
    lines.append(f"- Baseline total latency to completion: **{s['baseline_total_latency_ms_to_completion']:.2f}ms**")
    lines.append(f"- cgrep total latency to completion: **{s['cgrep_total_latency_ms_to_completion']:.2f}ms**")
    lines.append("")
    lines.append("## Re-run")
    lines.append("")
    lines.append("```bash")
    lines.append(
        "python3 scripts/benchmark_agent_token_efficiency.py "
        "--repo /path/to/pytorch"
    )
    lines.append("```")
    lines.append("")
    lines.append("## Periodic Tracking")
    lines.append("")
    lines.append("```bash")
    lines.append(
        "python3 scripts/benchmark_agent_token_efficiency.py "
        "--repo /path/to/pytorch "
        "--history-dir local/benchmarks/history"
    )
    lines.append("```")
    lines.append("")
    lines.append("```cron")
    lines.append(
        "0 3 * * 1 cd /path/to/cgrep && "
        "python3 scripts/benchmark_agent_token_efficiency.py "
        "--repo /path/to/pytorch --history-dir local/benchmarks/history"
    )
    lines.append("```")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    repo_default = os.environ.get("PYTORCH_REPO", "")
    parser = argparse.ArgumentParser(description="Benchmark AI-agent token efficiency to completion on PyTorch")
    parser.add_argument("--repo", default=repo_default, help="Path to PyTorch repository (or set PYTORCH_REPO)")
    parser.add_argument("--cgrep-bin", default="target/release/cgrep", help="Path to cgrep binary")
    parser.add_argument("--json-out", default="local/benchmarks/pytorch-agent-token-efficiency.json")
    parser.add_argument("--md-out", default="docs/benchmarks/pytorch-agent-token-efficiency.md")
    parser.add_argument("--history-dir", default="", help="Optional timestamped JSON snapshot directory")
    parser.add_argument("--timeout", type=int, default=300, help="Per command timeout seconds")
    parser.add_argument("--index-timeout", type=int, default=3600, help="Index timeout seconds")
    parser.add_argument("--skip-index", action="store_true", help="Skip index rebuild")

    parser.add_argument("--grep-max-matches", type=int, default=300)
    parser.add_argument("--rg-max-matches", dest="grep_max_matches", type=int, help=argparse.SUPPRESS)
    parser.add_argument("--baseline-max-files", type=int, default=16)
    parser.add_argument("--baseline-max-windows-per-file", type=int, default=2)
    parser.add_argument("--baseline-context-lines", type=int, default=20)
    parser.add_argument("--baseline-max-chars", type=int, default=180000)
    parser.add_argument(
        "--baseline-file-tiers",
        default="2,4,6,8,12",
        help="Comma-separated file expansion tiers for baseline completion loop",
    )

    parser.add_argument("--locate-limit", type=int, default=12)
    parser.add_argument("--expand-ids", type=int, default=8)
    parser.add_argument("--expand-context", type=int, default=8)
    parser.add_argument(
        "--cgrep-expand-tiers",
        default="1,2,4,6,8",
        help="Comma-separated ID expansion tiers for cgrep completion loop",
    )

    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parents[1]
    if not args.repo:
        raise SystemExit("Set --repo /path/to/pytorch (or PYTORCH_REPO).")
    repo_path = Path(args.repo).resolve()
    cgrep_bin = (repo_root / args.cgrep_bin).resolve()

    if not repo_path.exists() or not (repo_path / ".git").exists():
        raise SystemExit(f"Invalid repo path: {repo_path}")
    if not cgrep_bin.exists():
        raise SystemExit(f"cgrep binary not found: {cgrep_bin}. Run `cargo build --release`.")

    try:
        baseline_tiers = parse_tiers(args.baseline_file_tiers)
        cgrep_tiers = parse_tiers(args.cgrep_expand_tiers)
    except ValueError as exc:
        raise SystemExit(f"Invalid tier config: {exc}") from exc

    if max(baseline_tiers) > args.baseline_max_files:
        raise SystemExit(
            f"baseline tier max ({max(baseline_tiers)}) exceeds --baseline-max-files ({args.baseline_max_files})"
        )
    if max(cgrep_tiers) > args.expand_ids:
        raise SystemExit(
            f"cgrep tier max ({max(cgrep_tiers)}) exceeds --expand-ids ({args.expand_ids})"
        )

    tokenizer_name, count_tokens = load_tokenizer()

    if args.skip_index:
        index_run = CommandRun([], 0, 0.0, "", "")
    else:
        index_run = run_index(cgrep_bin=cgrep_bin, repo_path=repo_path, timeout_s=args.index_timeout)
        if index_run.returncode != 0:
            raise SystemExit(
                "Indexing failed\n"
                f"stdout:\n{index_run.stdout[-2000:]}\n"
                f"stderr:\n{index_run.stderr[-2000:]}"
            )

    scenario_results: list[dict[str, Any]] = []
    for sc in SCENARIOS:
        scenario_start = time.perf_counter()
        grep_cmd = [
            "grep",
            "-R",
            "-n",
            "-I",
            "-E",
            "--color=never",
            "--exclude-dir=.git",
            "-m",
            str(args.grep_max_matches),
            "-e",
            sc.grep_pattern,
            ".",
        ]
        grep_run = run_cmd(grep_cmd, cwd=repo_path, timeout_s=args.timeout)
        ordered_files, sections_by_file, windows_by_file, baseline_prep_meta = prepare_baseline_snippets(
            repo_path=repo_path,
            grep_run=grep_run,
            grep_max_matches=args.grep_max_matches,
            max_windows_per_file=args.baseline_max_windows_per_file,
            context_lines=args.baseline_context_lines,
            max_unique_files=args.baseline_max_files,
        )
        baseline_attempts: list[dict[str, Any]] = []
        baseline_context_parts: list[str] = []
        baseline_tokens_to_completion = 0
        baseline_latency_to_completion_ms = 0.0
        baseline_completed = False
        baseline_missing = [" | ".join(group) for group in sc.completion_groups]
        previous_file_cap = 0
        locate_preview = "\n".join(grep_run.stdout.splitlines()[:200])
        for idx, tier in enumerate(baseline_tiers, start=1):
            capped_tier = min(tier, len(ordered_files))
            new_files = ordered_files[previous_file_cap:capped_tier]
            added_windows = sum(windows_by_file.get(rel, 0) for rel in new_files)
            attempt_parts: list[str] = []
            if idx == 1:
                attempt_parts.extend(
                    [
                        f"Task: {sc.objective}",
                        "",
                        "=== Baseline locate output (grep) ===",
                        locate_preview,
                        "",
                    ]
                )
            attempt_parts.append(f"=== Baseline snippet expansion tier {capped_tier} ===")
            if new_files:
                for rel in new_files:
                    body = sections_by_file.get(rel, "").strip()
                    if body:
                        attempt_parts.append(body)
            else:
                attempt_parts.append("[no additional files to expand]")

            attempt_payload = "\n".join(attempt_parts).strip()
            if len(attempt_payload) > args.baseline_max_chars:
                attempt_payload = attempt_payload[: args.baseline_max_chars] + "\n\n[TRUNCATED]"

            attempt_tokens = count_tokens(attempt_payload)
            baseline_tokens_to_completion += attempt_tokens
            baseline_latency_to_completion_ms += grep_run.duration_ms if idx == 1 else 0.0
            baseline_context_parts.append(attempt_payload)
            baseline_context = "\n\n".join(baseline_context_parts)
            baseline_missing = missing_completion_groups(baseline_context, sc.completion_groups)
            baseline_completed = len(baseline_missing) == 0
            baseline_attempts.append(
                {
                    "tier": capped_tier,
                    "added_files": len(new_files),
                    "added_windows": added_windows,
                    "attempt_tokens": attempt_tokens,
                    "cumulative_tokens": baseline_tokens_to_completion,
                    "completed": baseline_completed,
                    "missing_markers": baseline_missing,
                }
            )
            previous_file_cap = capped_tier
            if baseline_completed or previous_file_cap >= len(ordered_files):
                break

        locate_limit_effective = max(args.locate_limit, max(cgrep_tiers), args.expand_ids)
        locate_run, locate_ids = run_cgrep_locate(
            cgrep_bin=cgrep_bin,
            repo_path=repo_path,
            scenario=sc,
            locate_limit=locate_limit_effective,
            max_ids=args.expand_ids,
        )
        cgrep_attempts: list[dict[str, Any]] = []
        cgrep_context_parts: list[str] = []
        cgrep_tokens_to_completion = 0
        cgrep_latency_to_completion_ms = 0.0
        cgrep_completed = False
        cgrep_missing = [" | ".join(group) for group in sc.completion_groups]
        expanded_so_far = 0
        for idx, tier in enumerate(cgrep_tiers, start=1):
            capped_tier = min(tier, len(locate_ids))
            next_ids = locate_ids[expanded_so_far:capped_tier]
            attempt_parts = []
            if idx == 1:
                attempt_parts.extend(
                    [
                        f"Task: {sc.objective}",
                        "",
                        "=== cgrep locate ===",
                        locate_run.stdout.strip(),
                        "",
                    ]
                )
                cgrep_latency_to_completion_ms += locate_run.duration_ms
            attempt_parts.append(f"=== cgrep expand tier {capped_tier} ===")
            expand_run = run_cgrep_expand(
                cgrep_bin=cgrep_bin,
                repo_path=repo_path,
                ids=next_ids,
                expand_context=args.expand_context,
            )
            if expand_run is not None:
                cgrep_latency_to_completion_ms += expand_run.duration_ms
                if expand_run.returncode == 0:
                    attempt_parts.append(expand_run.stdout.strip())
                else:
                    attempt_parts.append(
                        f"[expand failed rc={expand_run.returncode}] {expand_run.stderr[-400:]}"
                    )
            else:
                attempt_parts.append("[no additional ids to expand]")

            attempt_payload = "\n".join(attempt_parts).strip()
            attempt_tokens = count_tokens(attempt_payload)
            cgrep_tokens_to_completion += attempt_tokens
            cgrep_context_parts.append(attempt_payload)
            cgrep_context = "\n\n".join(cgrep_context_parts)
            cgrep_missing = missing_completion_groups(cgrep_context, sc.completion_groups)
            cgrep_completed = len(cgrep_missing) == 0
            cgrep_attempts.append(
                {
                    "tier": capped_tier,
                    "added_ids": len(next_ids),
                    "attempt_tokens": attempt_tokens,
                    "cumulative_tokens": cgrep_tokens_to_completion,
                    "completed": cgrep_completed,
                    "missing_markers": cgrep_missing,
                    "expand_returncode": (expand_run.returncode if expand_run else None),
                    "expand_duration_ms": (expand_run.duration_ms if expand_run else 0.0),
                }
            )
            expanded_so_far = capped_tier
            if cgrep_completed or expanded_so_far >= len(locate_ids):
                break

        reduction_percent = 0.0
        if baseline_tokens_to_completion > 0:
            reduction_percent = (
                (baseline_tokens_to_completion - cgrep_tokens_to_completion)
                / baseline_tokens_to_completion
            ) * 100.0

        scenario_results.append(
            {
                "id": sc.id,
                "objective": sc.objective,
                "coding_task": sc.coding_task,
                "completion_groups": [list(group) for group in sc.completion_groups],
                "baseline_tokens_to_completion": baseline_tokens_to_completion,
                "cgrep_tokens_to_completion": cgrep_tokens_to_completion,
                "token_reduction_percent_to_completion": reduction_percent,
                "baseline_latency_ms_to_completion": baseline_latency_to_completion_ms,
                "cgrep_latency_ms_to_completion": cgrep_latency_to_completion_ms,
                "baseline_attempts": len(baseline_attempts),
                "cgrep_attempts": len(cgrep_attempts),
                "baseline_completed": baseline_completed,
                "cgrep_completed": cgrep_completed,
                "baseline_missing_markers": baseline_missing,
                "cgrep_missing_markers": cgrep_missing,
                "scenario_duration_ms": (time.perf_counter() - scenario_start) * 1000.0,
                "baseline": {
                    "grep_command": grep_run.command,
                    "grep_returncode": grep_run.returncode,
                    "grep_duration_ms": grep_run.duration_ms,
                    "tiers": baseline_tiers,
                    "attempts": baseline_attempts,
                    **baseline_prep_meta,
                },
                "cgrep": {
                    "locate_command": locate_run.command,
                    "locate_returncode": locate_run.returncode,
                    "locate_duration_ms": locate_run.duration_ms,
                    "locate_ids": locate_ids,
                    "tiers": cgrep_tiers,
                    "attempts": cgrep_attempts,
                },
            }
        )

    baseline_total_tokens = sum(x["baseline_tokens_to_completion"] for x in scenario_results)
    cgrep_total_tokens = sum(x["cgrep_tokens_to_completion"] for x in scenario_results)
    baseline_total_latency = sum(x["baseline_latency_ms_to_completion"] for x in scenario_results)
    cgrep_total_latency = sum(x["cgrep_latency_ms_to_completion"] for x in scenario_results)
    baseline_completed_scenarios = sum(1 for x in scenario_results if x["baseline_completed"])
    cgrep_completed_scenarios = sum(1 for x in scenario_results if x["cgrep_completed"])

    token_reduction = 0.0
    if baseline_total_tokens > 0:
        token_reduction = (
            (baseline_total_tokens - cgrep_total_tokens) / baseline_total_tokens
        ) * 100.0

    compression = math.inf if cgrep_total_tokens == 0 else baseline_total_tokens / cgrep_total_tokens

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
            "tokenizer": tokenizer_name,
            "grep_max_matches": args.grep_max_matches,
            "baseline_max_files": args.baseline_max_files,
            "baseline_max_windows_per_file": args.baseline_max_windows_per_file,
            "baseline_context_lines": args.baseline_context_lines,
            "baseline_max_chars": args.baseline_max_chars,
            "baseline_file_tiers": baseline_tiers,
            "locate_limit": args.locate_limit,
            "locate_limit_effective": max(args.locate_limit, max(cgrep_tiers), args.expand_ids),
            "expand_ids": args.expand_ids,
            "expand_context": args.expand_context,
            "cgrep_expand_tiers": cgrep_tiers,
            "completion_marker_strategy": "at_least_one_match_per_group_in_cumulative_outputs",
            "scenario_count": len(SCENARIOS),
        },
        "index": {
            "command": index_run.command,
            "returncode": index_run.returncode,
            "duration_ms": index_run.duration_ms,
            "stderr_tail": index_run.stderr[-1000:],
            "stdout_tail": index_run.stdout[-1000:],
        },
        "scenario_results": scenario_results,
        "summary": {
            "index_build_ms": index_run.duration_ms,
            "scenario_count": len(SCENARIOS),
            "baseline_completed_scenarios": baseline_completed_scenarios,
            "cgrep_completed_scenarios": cgrep_completed_scenarios,
            "baseline_total_tokens_to_completion": baseline_total_tokens,
            "cgrep_total_tokens_to_completion": cgrep_total_tokens,
            "token_reduction_percent_to_completion": token_reduction,
            "token_compression_x_to_completion": compression,
            "baseline_total_latency_ms_to_completion": baseline_total_latency,
            "cgrep_total_latency_ms_to_completion": cgrep_total_latency,
        },
    }

    json_out = (repo_root / args.json_out).resolve()
    md_out = (repo_root / args.md_out).resolve()
    json_out.parent.mkdir(parents=True, exist_ok=True)
    md_out.parent.mkdir(parents=True, exist_ok=True)

    json_out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    md_out.write_text(render_markdown(payload), encoding="utf-8")

    if args.history_dir:
        history_dir = (repo_root / args.history_dir).resolve()
        history_dir.mkdir(parents=True, exist_ok=True)
        stamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        history_file = history_dir / f"pytorch-agent-token-efficiency-{stamp}.json"
        history_file.write_text(json.dumps(payload, indent=2), encoding="utf-8")

    print(f"JSON: {json_out}")
    print(f"MD:   {md_out}")
    print(
        f"Token reduction (to completion): {token_reduction:.1f}% "
        f"({baseline_total_tokens:,} -> {cgrep_total_tokens:,}, {compression:.2f}x)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
