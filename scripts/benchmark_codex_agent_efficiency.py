#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0

"""Benchmark Codex agent token efficiency on a local PyTorch checkout.

This benchmark runs real `codex exec` sessions in two modes:
1) baseline: grep/sed/cat style retrieval
2) cgrep: cgrep-based retrieval

It records provider usage telemetry from Codex events (`turn.completed.usage`).
"""

from __future__ import annotations

import argparse
import dataclasses
import datetime as dt
import json
import math
import os
import platform
import re
import statistics
import subprocess
import tempfile
import textwrap
import time
from pathlib import Path
from typing import Any


@dataclasses.dataclass(frozen=True)
class Scenario:
    id: str
    objective: str
    coding_task: str
    grep_pattern: str
    cgrep_commands: tuple[str, ...]
    completion_groups: tuple[tuple[str, ...], ...]


SCENARIOS: list[Scenario] = [
    Scenario(
        id="autograd_evaluate_function",
        objective="Find where autograd engine evaluate_function is implemented and inspected.",
        coding_task="Patch autograd evaluate_function flow and verify the implementation file + autograd context.",
        grep_pattern="evaluate_function",
        cgrep_commands=(
            "d Engine::evaluate_function --format json --compact",
            "d evaluate_function --format json --compact",
        ),
        completion_groups=(("evaluate_function",), ("engine.cpp", "autograd/")),
    ),
    Scenario(
        id="tensor_iterator_impl",
        objective="Find TensorIterator definition and major implementation usage points.",
        coding_task="Prepare a TensorIterator behavior change by locating the core declaration and implementation paths.",
        grep_pattern="TensorIterator",
        cgrep_commands=(
            "d TensorIteratorBase --format json --compact",
            "d TensorIteratorBase::reorder_dimensions --format json --compact",
        ),
        completion_groups=(("TensorIterator",), ("TensorIterator.h", "TensorIterator.cpp")),
    ),
    Scenario(
        id="python_arg_parser_impl",
        objective="Locate PythonArgParser implementation and usage points.",
        coding_task="Implement a parser-related fix by gathering PythonArgParser definition and source implementation.",
        grep_pattern="PythonArgParser",
        cgrep_commands=(
            "d PythonArgParser --format json --compact",
            's "check_deprecated python_arg_parser" --format json2 --compact',
        ),
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
        cgrep_commands=(
            "d DispatchKeySet --format json --compact",
            "d getRuntimeDispatchKeySet --format json --compact",
        ),
        completion_groups=(("DispatchKeySet",), ("DispatchKeySet.h", "c10/core/")),
    ),
    Scenario(
        id="cuda_graph",
        objective="Locate CUDAGraph implementation-related code quickly.",
        coding_task="Make a CUDAGraph code-path update by collecting implementation and CUDA path context.",
        grep_pattern="CUDAGraph",
        cgrep_commands=(
            "d CUDAGraph --format json --compact",
            's "CUDAGraph.cpp" --format json2 --compact',
        ),
        completion_groups=(("CUDAGraph",), ("CUDAGraph.cpp", "cuda/")),
    ),
    Scenario(
        id="addmm_path",
        objective="Find addmm implementation and call sites.",
        coding_task="Modify addmm behavior by locating native implementation and addmm_out call path.",
        grep_pattern=r"addmm\(",
        cgrep_commands=(
            "d addmm_out_cpu --format json --compact",
            "d addmm_impl_cpu_ --format json --compact",
        ),
        completion_groups=(("addmm(", "addmm"), ("LinearAlgebra.cpp", "addmm_out", "native/")),
    ),
]


@dataclasses.dataclass
class CodexRun:
    success: bool
    duration_ms: float
    objective_met: bool
    markers_met: bool
    command_policy_ok: bool
    command_count: int
    input_tokens: int
    cached_input_tokens: int
    output_tokens: int
    total_tokens: int
    billable_tokens: int
    commands: list[str]
    disallowed_commands: list[str]
    final_json: dict[str, Any]
    errors: list[str]


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


def git_rev(path: Path) -> str:
    proc = run_cmd(["git", "rev-parse", "--short", "HEAD"], cwd=path, timeout_s=30)
    if proc.returncode != 0:
        return "unknown"
    return proc.stdout.strip() or "unknown"


def count_files(path: Path) -> int:
    proc = run_cmd(["git", "ls-files"], cwd=path, timeout_s=120)
    if proc.returncode != 0:
        return 0
    out = proc.stdout.strip()
    return 0 if not out else out.count("\n") + 1


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


def median(values: list[float]) -> float:
    return statistics.median(values) if values else 0.0


def marker_groups_met(final_json: dict[str, Any], groups: tuple[tuple[str, ...], ...]) -> bool:
    evidence = final_json.get("evidence", [])
    chunks: list[str] = []
    if isinstance(evidence, list):
        for row in evidence:
            if not isinstance(row, dict):
                continue
            path = row.get("path")
            reason = row.get("reason")
            if isinstance(path, str):
                chunks.append(path)
            if isinstance(reason, str):
                chunks.append(reason)
    normalized = " ".join(chunks).lower()
    for group in groups:
        if not any(marker.lower() in normalized for marker in group):
            return False
    return True


def is_bootstrap_agents_command(cmd: str) -> bool:
    normalized = cmd.lower()
    return ("agents.md" in normalized) and (
        "find .. -name agents.md" in normalized
        or "cat agents.md" in normalized
        or "cat ./agents.md" in normalized
    )


def disallowed_for_mode(mode: str, cmd: str) -> bool:
    normalized = cmd.lower()
    if is_bootstrap_agents_command(normalized):
        return False
    if mode == "baseline":
        if " --help" in normalized:
            return True
        return (" cgrep" in f" {normalized}") or re.search(r"(^|\s)rg(\s|$)", normalized) is not None
    if mode == "cgrep":
        if " --help" in normalized:
            return True
        if re.search(r"(^|\s)cgrep(\s|$)", normalized) is None and "/cgrep" not in normalized:
            return True
        match = re.search(r"(?:^|\s)(?:[\w./-]*cgrep)\s+([\w-]+)", normalized)
        if match is None:
            return True
        subcommand = match.group(1)
        if subcommand not in {"s", "search", "d", "definition"}:
            return True
        if re.search(r"(^|\s)agent(\s|$)", normalized) is not None:
            return True
        if re.search(r"(^|\s)read(\s|$)", normalized) is not None:
            return True
        if re.search(r"(^|\s)rg(\s|$)", normalized) is not None:
            return True
        if re.search(r"(^|\s)grep(\s|$)", normalized) is not None:
            return True
        if re.search(r"(^|\s)find(\s|$)", normalized) is not None:
            return True
    return False


def schema_file() -> Path:
    schema = {
        "type": "object",
        "properties": {
            "objective_met": {"type": "boolean"},
            "evidence": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "line": {"type": "integer"},
                        "reason": {"type": "string"},
                    },
                    "required": ["path", "line", "reason"],
                    "additionalProperties": False,
                },
            },
        },
        "required": ["objective_met", "evidence"],
        "additionalProperties": False,
    }
    fd, raw = tempfile.mkstemp(prefix="codex-bench-schema-", suffix=".json")
    os.close(fd)
    path = Path(raw)
    path.write_text(json.dumps(schema), encoding="utf-8")
    return path


def build_prompt(mode: str, scenario: Scenario, cgrep_bin: Path) -> str:
    groups = "; ".join(" OR ".join(group) for group in scenario.completion_groups)
    if mode == "baseline":
        rules = (
            "- Do NOT use cgrep or rg.\n"
            "- Use only grep-based retrieval.\n"
            f"- First command MUST be:\n"
            f"  grep -R -n -I -E --exclude-dir=.git -m 200 -e '{scenario.grep_pattern}' .\n"
            "- If needed, run at most one additional grep command with a narrower pattern/path.\n"
            "- Do not run `--help`.\n"
            "- Do not edit files.\n"
            "- Keep commands minimal."
        )
    else:
        cgrep_steps = []
        for idx, command in enumerate(scenario.cgrep_commands, start=1):
            cgrep_steps.append(f"{idx}. {cgrep_bin} {command}")
        cgrep_plan = "\n".join(cgrep_steps)
        rules = (
            f"- Use only this cgrep binary: {cgrep_bin}\n"
            "- Allowed subcommands are only `search`/`s` and `definition`/`d`.\n"
            "- Start with command 1 and execute commands in order.\n"
            "- Run the next command only when evidence is still insufficient.\n"
            "- Do not run commands outside this list:\n"
            f"{cgrep_plan}\n"
            "- Stop as soon as marker groups are satisfied.\n"
            "- Do NOT wrap commands with `bash -lc`.\n"
            "- Do NOT use `cgrep agent`, `cgrep read`, grep/rg/find, or any `--help` command.\n"
            "- Do not edit files.\n"
            "- If evidence is still insufficient after the listed commands, return `objective_met: false` instead of running extra commands."
        )
    return textwrap.dedent(
        f"""
        You are running a retrieval benchmark on a local PyTorch checkout.

        Objective:
        {scenario.objective}

        Coding task context:
        {scenario.coding_task}

        Rules:
        {rules}

        Success marker groups:
        {groups}

        Return only JSON that matches the provided schema.
        Evidence must be concise and include concrete file paths and line numbers.
        """
    ).strip()


def run_codex_mode(
    *,
    repo_path: Path,
    cgrep_bin: Path,
    mode: str,
    scenario: Scenario,
    model: str,
    reasoning_effort: str,
    timeout_s: int,
) -> CodexRun:
    schema_path = schema_file()
    prompt = build_prompt(mode, scenario, cgrep_bin)
    cmd = [
        "codex",
        "exec",
        "--json",
        "--full-auto",
        "--skip-git-repo-check",
        "-C",
        str(repo_path),
        "-c",
        f'model_reasoning_effort="{reasoning_effort}"',
        "-m",
        model,
        "--output-schema",
        str(schema_path),
        prompt,
    ]

    errors: list[str] = []
    command_items: list[str] = []
    disallowed_commands: list[str] = []
    input_tokens = 0
    cached_input_tokens = 0
    output_tokens = 0
    final_json: dict[str, Any] = {}
    objective_met = False
    markers_met = False
    command_policy_ok = False
    command_count = 0

    started = time.perf_counter()
    try:
        proc = run_cmd(cmd, cwd=repo_path, timeout_s=timeout_s)
    except subprocess.TimeoutExpired:
        duration_ms = (time.perf_counter() - started) * 1000.0
        schema_path.unlink(missing_ok=True)
        return CodexRun(
            success=False,
            duration_ms=duration_ms,
            objective_met=False,
            markers_met=False,
            command_policy_ok=False,
            command_count=0,
            input_tokens=0,
            cached_input_tokens=0,
            output_tokens=0,
            total_tokens=0,
            billable_tokens=0,
            commands=[],
            disallowed_commands=[],
            final_json={},
            errors=[f"timeout after {timeout_s}s"],
        )
    duration_ms = (time.perf_counter() - started) * 1000.0

    if proc.returncode != 0:
        if proc.stderr.strip():
            errors.append(proc.stderr.strip()[-2000:])

    for line in proc.stdout.splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        etype = event.get("type")
        if etype == "error":
            message = event.get("message")
            if isinstance(message, str):
                errors.append(message)
        elif etype == "turn.completed":
            usage = event.get("usage", {})
            if isinstance(usage, dict):
                input_tokens = int(usage.get("input_tokens", 0) or 0)
                cached_input_tokens = int(usage.get("cached_input_tokens", 0) or 0)
                output_tokens = int(usage.get("output_tokens", 0) or 0)
        elif etype == "item.completed":
            item = event.get("item", {})
            if not isinstance(item, dict):
                continue
            item_type = item.get("type")
            if item_type == "command_execution":
                command = item.get("command")
                if isinstance(command, str):
                    command_items.append(command)
                    if disallowed_for_mode(mode, command):
                        disallowed_commands.append(command)
            elif item_type == "agent_message":
                text = item.get("text")
                if isinstance(text, str) and text.strip():
                    try:
                        final_json = json.loads(text)
                    except json.JSONDecodeError:
                        errors.append("agent_message was not valid JSON")

    objective_met = bool(final_json.get("objective_met")) if final_json else False
    markers_met = marker_groups_met(final_json, scenario.completion_groups) if final_json else False
    command_policy_ok = len(disallowed_commands) == 0
    command_count = sum(1 for c in command_items if not is_bootstrap_agents_command(c))
    billable_tokens = (input_tokens - cached_input_tokens) + output_tokens
    total_tokens = input_tokens + output_tokens

    schema_path.unlink(missing_ok=True)

    success = (
        proc.returncode == 0
        and bool(final_json)
        and objective_met
        and markers_met
        and command_policy_ok
    )
    return CodexRun(
        success=success,
        duration_ms=duration_ms,
        objective_met=objective_met,
        markers_met=markers_met,
        command_policy_ok=command_policy_ok,
        command_count=command_count,
        input_tokens=input_tokens,
        cached_input_tokens=cached_input_tokens,
        output_tokens=output_tokens,
        total_tokens=total_tokens,
        billable_tokens=billable_tokens,
        commands=command_items,
        disallowed_commands=disallowed_commands,
        final_json=final_json,
        errors=errors,
    )


def aggregate_mode(rows: list[dict[str, Any]]) -> dict[str, Any]:
    if not rows:
        return {
            "cases": 0,
            "successes": 0,
            "success_rate_percent": 0.0,
            "median_total_tokens": 0.0,
            "median_billable_tokens": 0.0,
            "p95_billable_tokens": 0.0,
            "median_duration_ms": 0.0,
            "median_commands": 0.0,
            "total_billable_tokens": 0,
        }
    success_count = sum(1 for r in rows if r["success"])
    total_tokens = [float(r["total_tokens"]) for r in rows]
    billable_tokens = [float(r["billable_tokens"]) for r in rows]
    durations = [float(r["duration_ms"]) for r in rows]
    commands = [float(r["command_count"]) for r in rows]
    return {
        "cases": len(rows),
        "successes": success_count,
        "success_rate_percent": (success_count / len(rows)) * 100.0,
        "median_total_tokens": median(total_tokens),
        "median_billable_tokens": median(billable_tokens),
        "p95_billable_tokens": percentile(billable_tokens, 95.0),
        "median_duration_ms": median(durations),
        "median_commands": median(commands),
        "total_billable_tokens": int(sum(billable_tokens)),
    }


def render_markdown(payload: dict[str, Any]) -> str:
    summary_all = payload["summary"]["all_cases"]
    rows = payload["results"]
    lines: list[str] = []
    lines.append("# PyTorch Codex Agent Efficiency Benchmark")
    lines.append("")
    lines.append(f"Generated: {payload['generated_at_utc']}")
    lines.append("")
    lines.append("## What This Measures")
    lines.append("")
    lines.append("- Real `codex exec` runs on a local PyTorch repository.")
    lines.append("- Baseline mode: grep/sed/cat style retrieval.")
    lines.append("- cgrep mode: cgrep-based retrieval commands.")
    lines.append("- Primary metric: Codex provider-reported billable tokens (`input - cached_input + output`).")
    lines.append("")
    lines.append("## Environment")
    lines.append("")
    lines.append(f"- OS: `{payload['environment']['os']}`")
    lines.append(f"- Python: `{payload['environment']['python']}`")
    lines.append(f"- codex model: `{payload['config']['model']}`")
    lines.append(f"- reasoning effort: `{payload['config']['reasoning_effort']}`")
    lines.append(f"- runs per scenario/mode: `{payload['config']['runs']}`")
    lines.append(f"- cgrep commit: `{payload['environment']['cgrep_commit']}`")
    lines.append(f"- pytorch commit: `{payload['environment']['pytorch_commit']}`")
    lines.append(f"- PyTorch files (`git ls-files`): `{payload['environment']['pytorch_file_count']}`")
    lines.append("")
    lines.append("## Aggregate (All Cases)")
    lines.append("")
    lines.append("| Mode | Cases | Success rate | Median billable tokens | P95 billable tokens | Median total tokens | Median duration (ms) | Median commands |")
    lines.append("|---|---:|---:|---:|---:|---:|---:|---:|")
    for mode in ("baseline", "cgrep"):
        row = summary_all[mode]
        lines.append(
            f"| `{mode}` | {row['cases']} | {row['success_rate_percent']:.1f}% | "
            f"{row['median_billable_tokens']:.0f} | {row['p95_billable_tokens']:.0f} | "
            f"{row['median_total_tokens']:.0f} | {row['median_duration_ms']:.1f} | {row['median_commands']:.1f} |"
        )
    lines.append("")
    baseline_all = summary_all["baseline"]
    cgrep_all = summary_all["cgrep"]
    if baseline_all["total_billable_tokens"] > 0:
        reduction_all = (
            (baseline_all["total_billable_tokens"] - cgrep_all["total_billable_tokens"])
            / baseline_all["total_billable_tokens"]
        ) * 100.0
    else:
        reduction_all = 0.0
    lines.append(f"- Total billable tokens (baseline): **{baseline_all['total_billable_tokens']:,}**")
    lines.append(f"- Total billable tokens (cgrep): **{cgrep_all['total_billable_tokens']:,}**")
    lines.append(f"- Billable token reduction: **{reduction_all:.1f}%**")
    lines.append("")
    lines.append("## Per Scenario")
    lines.append("")
    lines.append("| Run | Scenario | Mode | Success | Billable tokens | Total tokens | Duration (ms) | Commands |")
    lines.append("|---:|---|---|---|---:|---:|---:|---:|")
    for row in rows:
        lines.append(
            f"| {row['run_index']} | `{row['scenario_id']}` | `{row['mode']}` | {'yes' if row['success'] else 'no'} | "
            f"{row['billable_tokens']:,} | {row['total_tokens']:,} | {row['duration_ms']:.1f} | "
            f"{row['command_count']} |"
        )
    lines.append("")
    lines.append("## Re-run")
    lines.append("")
    lines.append("```bash")
    lines.append(
        "python3 scripts/benchmark_codex_agent_efficiency.py "
        "--repo /path/to/pytorch "
        "--cgrep-bin /path/to/cgrep"
    )
    lines.append("```")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description="Benchmark Codex-based agent retrieval efficiency on PyTorch")
    parser.add_argument("--repo", required=True, help="Path to local PyTorch repository")
    parser.add_argument("--cgrep-bin", default="target/release/cgrep", help="Path to cgrep binary")
    parser.add_argument("--model", default="gpt-5-codex", help="Codex model name")
    parser.add_argument(
        "--reasoning-effort",
        default="medium",
        choices=["minimal", "low", "medium", "high"],
        help="Codex reasoning effort",
    )
    parser.add_argument("--runs", type=int, default=3, help="Number of runs per scenario/mode")
    parser.add_argument("--timeout", type=int, default=600, help="Per Codex run timeout seconds")
    parser.add_argument("--skip-index", action="store_true", help="Skip cgrep index rebuild in target repo")
    parser.add_argument("--json-out", default="local/benchmarks/pytorch-codex-agent-efficiency.json")
    parser.add_argument("--md-out", default="docs/benchmarks/pytorch-codex-agent-efficiency.md")
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

    # Build index once for cgrep mode consistency.
    index_duration_ms = 0.0
    index_returncode = 0
    index_stderr_tail = ""
    if not args.skip_index:
        t0 = time.perf_counter()
        index_run = run_cmd(
            [str(cgrep_bin), "index", "--embeddings", "off", "--force"],
            cwd=repo_path,
            timeout_s=max(args.timeout, 1200),
        )
        index_duration_ms = (time.perf_counter() - t0) * 1000.0
        index_returncode = index_run.returncode
        index_stderr_tail = index_run.stderr[-1000:]
        if index_run.returncode != 0:
            raise SystemExit(
                "cgrep index failed\n"
                f"stdout:\n{index_run.stdout[-2000:]}\n"
                f"stderr:\n{index_run.stderr[-2000:]}"
            )

    results: list[dict[str, Any]] = []
    for run_index in range(1, args.runs + 1):
        # Alternate order to reduce warm-cache/order bias.
        mode_order = ("baseline", "cgrep") if run_index % 2 == 1 else ("cgrep", "baseline")
        for scenario in SCENARIOS:
            for mode in mode_order:
                run = run_codex_mode(
                    repo_path=repo_path,
                    cgrep_bin=cgrep_bin,
                    mode=mode,
                    scenario=scenario,
                    model=args.model,
                    reasoning_effort=args.reasoning_effort,
                    timeout_s=args.timeout,
                )
                results.append(
                    {
                        "run_index": run_index,
                        "scenario_id": scenario.id,
                        "scenario_objective": scenario.objective,
                        "coding_task": scenario.coding_task,
                        "mode": mode,
                        "success": run.success,
                        "objective_met": run.objective_met,
                        "markers_met": run.markers_met,
                        "command_policy_ok": run.command_policy_ok,
                        "command_count": run.command_count,
                        "input_tokens": run.input_tokens,
                        "cached_input_tokens": run.cached_input_tokens,
                        "output_tokens": run.output_tokens,
                        "total_tokens": run.total_tokens,
                        "billable_tokens": run.billable_tokens,
                        "duration_ms": run.duration_ms,
                        "commands": run.commands,
                        "disallowed_commands": run.disallowed_commands,
                        "errors": run.errors,
                        "final_json": run.final_json,
                    }
                )

    baseline_rows = [r for r in results if r["mode"] == "baseline"]
    cgrep_rows = [r for r in results if r["mode"] == "cgrep"]
    summary = {
        "all_cases": {
            "baseline": aggregate_mode(baseline_rows),
            "cgrep": aggregate_mode(cgrep_rows),
        },
    }

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
            "model": args.model,
            "reasoning_effort": args.reasoning_effort,
            "runs": args.runs,
            "timeout_s": args.timeout,
            "scenario_count": len(SCENARIOS),
            "skip_index": args.skip_index,
        },
        "index": {
            "returncode": index_returncode,
            "duration_ms": index_duration_ms,
            "stderr_tail": index_stderr_tail,
        },
        "results": results,
        "summary": summary,
    }

    json_out = (repo_root / args.json_out).resolve()
    md_out = (repo_root / args.md_out).resolve()
    json_out.parent.mkdir(parents=True, exist_ok=True)
    md_out.parent.mkdir(parents=True, exist_ok=True)
    json_out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    md_out.write_text(render_markdown(payload), encoding="utf-8")

    print(f"JSON: {json_out}")
    print(f"MD:   {md_out}")
    baseline_total = summary["all_cases"]["baseline"]["total_billable_tokens"]
    cgrep_total = summary["all_cases"]["cgrep"]["total_billable_tokens"]
    print(
        "Billable tokens (baseline -> cgrep): "
        f"{baseline_total:,} -> {cgrep_total:,}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
