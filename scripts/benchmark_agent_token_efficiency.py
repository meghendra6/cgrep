#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0

"""Benchmark AI-agent token efficiency on a local PyTorch checkout.

Compares two workflows for common coding-agent tasks:
1) Baseline (no cgrep): grep locate + manual file snippet extraction
2) cgrep workflow: agent locate + agent expand

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
    grep_pattern: str
    cgrep_query: str


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
        grep_pattern="evaluate_function",
        cgrep_query="where evaluate_function is implemented in autograd engine",
    ),
    Scenario(
        id="tensor_iterator_impl",
        objective="Find TensorIterator definition and major implementation usage points.",
        grep_pattern="TensorIterator",
        cgrep_query="where TensorIterator is defined and used in native ops",
    ),
    Scenario(
        id="python_arg_parser_impl",
        objective="Locate PythonArgParser implementation and usage points.",
        grep_pattern="PythonArgParser",
        cgrep_query="where PythonArgParser is implemented and used",
    ),
    Scenario(
        id="dispatch_key_set",
        objective="Understand DispatchKeySet representation and references.",
        grep_pattern="DispatchKeySet",
        cgrep_query="where DispatchKeySet is defined and referenced",
    ),
    Scenario(
        id="cuda_graph",
        objective="Locate CUDAGraph implementation-related code quickly.",
        grep_pattern="CUDAGraph",
        cgrep_query="where CUDAGraph is implemented",
    ),
    Scenario(
        id="addmm_path",
        objective="Find addmm implementation and call sites.",
        grep_pattern=r"addmm\(",
        cgrep_query="where addmm is implemented and called",
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


def build_baseline_payload(
    repo_path: Path,
    scenario: Scenario,
    grep_run: CommandRun,
    grep_max_matches: int,
    max_unique_files: int,
    max_windows_per_file: int,
    context_lines: int,
    max_payload_chars: int,
) -> tuple[str, dict[str, Any]]:
    matches = parse_grep_matches(grep_run.stdout, max_matches=grep_max_matches)

    unique_files: list[str] = []
    lines_by_file: dict[str, list[int]] = {}
    for rel, line_no, _snippet in matches:
        if rel not in lines_by_file:
            if len(unique_files) >= max_unique_files:
                continue
            unique_files.append(rel)
            lines_by_file[rel] = []
        if len(lines_by_file[rel]) < max_windows_per_file:
            lines_by_file[rel].append(line_no)

    sections: list[str] = []
    sections.append(f"Task: {scenario.objective}")
    sections.append("")
    sections.append("=== Baseline locate output (grep) ===")
    sections.append("\n".join(grep_run.stdout.splitlines()[:200]))
    sections.append("")
    sections.append("=== Baseline snippet expansion ===")

    snippet_count = 0
    for rel in unique_files:
        abs_path = repo_path / rel
        lines = safe_read_lines(abs_path)
        if not lines:
            continue
        for line_no in lines_by_file.get(rel, []):
            start = max(1, line_no - context_lines)
            end = min(len(lines), line_no + context_lines)
            body = "\n".join(lines[start - 1 : end])
            sections.append(f"--- {rel}:{start}-{end} ---")
            sections.append(body)
            sections.append("")
            snippet_count += 1

    payload = "\n".join(sections)
    truncated = False
    if len(payload) > max_payload_chars:
        payload = payload[:max_payload_chars] + "\n\n[TRUNCATED]"
        truncated = True

    meta = {
        "grep_match_count": len(matches),
        "unique_files_expanded": len(unique_files),
        "snippet_windows": snippet_count,
        "truncated": truncated,
    }
    return payload, meta


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


def build_cgrep_payload(
    cgrep_bin: Path,
    repo_path: Path,
    scenario: Scenario,
    locate_limit: int,
    expand_ids: int,
    expand_context: int,
) -> tuple[str, dict[str, Any], CommandRun, CommandRun | None]:
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

    ids = extract_ids_from_locate(locate_run.stdout, max_ids=expand_ids)
    expand_run: CommandRun | None = None

    if ids:
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
        expand_run = run_cmd(expand_cmd, cwd=repo_path)

    payload_parts = [
        f"Task: {scenario.objective}",
        "",
        "=== cgrep locate ===",
        locate_run.stdout.strip(),
        "",
        "=== cgrep expand ===",
        (expand_run.stdout.strip() if expand_run is not None else "[no ids from locate]"),
    ]
    payload = "\n".join(payload_parts)

    meta = {
        "locate_returncode": locate_run.returncode,
        "expand_returncode": (expand_run.returncode if expand_run else None),
        "locate_ids": ids,
        "locate_duration_ms": locate_run.duration_ms,
        "expand_duration_ms": (expand_run.duration_ms if expand_run else 0.0),
    }
    return payload, meta, locate_run, expand_run


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
    lines.append("## What This Measures")
    lines.append("")
    lines.append("1. **Baseline (without cgrep):** `grep` locate + manual snippet expansion from multiple files.")
    lines.append("2. **With cgrep:** `agent locate` + `agent expand` (tight budget, compact JSON).")
    lines.append("3. **Primary metric:** token volume sent to an AI coding agent for task completion.")
    lines.append("4. **Tokenizer:** OpenAI `cl100k_base` when available (fallback: byte/4 approximation).")
    lines.append("")
    lines.append("## Environment")
    lines.append("")
    lines.append(f"- OS: `{payload['environment']['os']}`")
    lines.append(f"- cgrep commit: `{payload['environment']['cgrep_commit']}`")
    lines.append(f"- pytorch commit: `{payload['environment']['pytorch_commit']}`")
    lines.append(f"- PyTorch files (`git ls-files`): `{payload['environment']['pytorch_file_count']}`")
    lines.append(f"- Tokenizer: `{payload['config']['tokenizer']}`")
    lines.append("")
    lines.append("## Results")
    lines.append("")
    lines.append("| Scenario | Baseline tokens | cgrep tokens | Reduction | Baseline latency (ms) | cgrep latency (ms) |")
    lines.append("|---|---:|---:|---:|---:|---:|")
    for r in rows:
        lines.append(
            f"| {r['objective']} | "
            f"{r['baseline_tokens']:,} | {r['cgrep_tokens']:,} | "
            f"{r['token_reduction_percent']:.1f}% | "
            f"{r['baseline_total_latency_ms']:.2f} | {r['cgrep_total_latency_ms']:.2f} |"
        )
    lines.append("")
    lines.append("## Aggregate")
    lines.append("")
    lines.append(f"- One-time index build: **{s['index_build_ms']/1000.0:.2f}s**")
    lines.append(f"- Baseline total tokens: **{s['baseline_total_tokens']:,}**")
    lines.append(f"- cgrep total tokens: **{s['cgrep_total_tokens']:,}**")
    lines.append(f"- Token reduction: **{s['token_reduction_percent']:.1f}%**")
    lines.append(f"- Token compression ratio (baseline/cgrep): **{s['token_compression_x']:.2f}x**")
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
    parser = argparse.ArgumentParser(description="Benchmark AI-agent token efficiency on PyTorch")
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
    parser.add_argument("--baseline-max-files", type=int, default=8)
    parser.add_argument("--baseline-max-windows-per-file", type=int, default=2)
    parser.add_argument("--baseline-context-lines", type=int, default=20)
    parser.add_argument("--baseline-max-chars", type=int, default=180000)

    parser.add_argument("--locate-limit", type=int, default=12)
    parser.add_argument("--expand-ids", type=int, default=6)
    parser.add_argument("--expand-context", type=int, default=8)

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

        baseline_payload, baseline_meta = build_baseline_payload(
            repo_path=repo_path,
            scenario=sc,
            grep_run=grep_run,
            grep_max_matches=args.grep_max_matches,
            max_unique_files=args.baseline_max_files,
            max_windows_per_file=args.baseline_max_windows_per_file,
            context_lines=args.baseline_context_lines,
            max_payload_chars=args.baseline_max_chars,
        )

        cgrep_payload, cgrep_meta, locate_run, expand_run = build_cgrep_payload(
            cgrep_bin=cgrep_bin,
            repo_path=repo_path,
            scenario=sc,
            locate_limit=args.locate_limit,
            expand_ids=args.expand_ids,
            expand_context=args.expand_context,
        )

        baseline_tokens = count_tokens(baseline_payload)
        cgrep_tokens = count_tokens(cgrep_payload)

        reduction_percent = 0.0
        if baseline_tokens > 0:
            reduction_percent = ((baseline_tokens - cgrep_tokens) / baseline_tokens) * 100.0

        cgrep_latency = locate_run.duration_ms + (expand_run.duration_ms if expand_run else 0.0)

        scenario_results.append(
            {
                "id": sc.id,
                "objective": sc.objective,
                "baseline_tokens": baseline_tokens,
                "cgrep_tokens": cgrep_tokens,
                "token_reduction_percent": reduction_percent,
                "baseline_total_latency_ms": grep_run.duration_ms,
                "cgrep_total_latency_ms": cgrep_latency,
                "baseline": {
                    "grep_command": grep_run.command,
                    "grep_returncode": grep_run.returncode,
                    "grep_duration_ms": grep_run.duration_ms,
                    **baseline_meta,
                },
                "cgrep": {
                    "locate_command": locate_run.command,
                    "locate_returncode": locate_run.returncode,
                    "locate_duration_ms": locate_run.duration_ms,
                    "expand_command": (expand_run.command if expand_run else []),
                    "expand_returncode": (expand_run.returncode if expand_run else None),
                    "expand_duration_ms": (expand_run.duration_ms if expand_run else 0.0),
                    **cgrep_meta,
                },
            }
        )

    baseline_total_tokens = sum(x["baseline_tokens"] for x in scenario_results)
    cgrep_total_tokens = sum(x["cgrep_tokens"] for x in scenario_results)
    token_reduction = 0.0
    if baseline_total_tokens > 0:
        token_reduction = ((baseline_total_tokens - cgrep_total_tokens) / baseline_total_tokens) * 100.0

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
            "locate_limit": args.locate_limit,
            "expand_ids": args.expand_ids,
            "expand_context": args.expand_context,
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
            "baseline_total_tokens": baseline_total_tokens,
            "cgrep_total_tokens": cgrep_total_tokens,
            "token_reduction_percent": token_reduction,
            "token_compression_x": compression,
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
        f"Token reduction: {token_reduction:.1f}% "
        f"({baseline_total_tokens:,} -> {cgrep_total_tokens:,}, {compression:.2f}x)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
