#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0

import argparse
import json
import math
import os
import statistics
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Callable, Optional


def run(cmd: list[str], cwd: Path, env: Optional[dict[str, str]] = None) -> subprocess.CompletedProcess:
    effective_env = os.environ.copy()
    if env:
        effective_env.update(env)
    return subprocess.run(
        cmd,
        cwd=cwd,
        env=effective_env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=True,
    )


def timed_ms(cmd: list[str], cwd: Path, env: Optional[dict[str, str]] = None) -> float:
    t0 = time.perf_counter()
    run(cmd, cwd, env)
    return (time.perf_counter() - t0) * 1000.0


def collect_metric_samples(runs: int, warmup: int, worker: Callable[[], float]) -> list[float]:
    for _ in range(max(0, warmup)):
        worker()
    values: list[float] = []
    for _ in range(runs):
        values.append(worker())
    return values


def percentile_nearest_rank(values: list[float], percentile: int) -> float:
    ordered = sorted(values)
    if not ordered:
        return 0.0
    if percentile <= 0:
        return ordered[0]
    if percentile >= 100:
        return ordered[-1]
    rank = max(1, math.ceil((percentile / 100.0) * len(ordered)))
    return ordered[rank - 1]


def summarize_latency_ms(samples: list[float]) -> dict[str, float]:
    return {
        "p50": round(statistics.median(samples), 2),
        "p95": round(percentile_nearest_rank(samples, 95), 2),
    }


def git(cwd: Path, *args: str) -> str:
    out = run(["git", *args], cwd)
    return out.stdout.strip()


def write_fixture(repo_root: Path, file_count: int) -> None:
    src = repo_root / "src"
    src.mkdir(parents=True, exist_ok=True)
    for i in range(file_count):
        marker = "reuse_probe_token" if i % 17 == 0 else "noise"
        body = "    let value = input + 1;\n" * 20
        content = (
            f"pub fn worker_{i}(input: i32) -> i32 {{\n"
            f"    // {marker}\n"
            f"{body}"
            f"    input + {i}\n"
            f"}}\n"
        )
        (src / f"mod_{i}.rs").write_text(content, encoding="utf-8")


def setup_origin(tmp_root: Path, file_count: int) -> Path:
    seed = tmp_root / "seed"
    seed.mkdir(parents=True, exist_ok=True)
    git(seed, "init")
    git(seed, "config", "user.email", "perf@example.com")
    git(seed, "config", "user.name", "Perf")
    write_fixture(seed, file_count)
    git(seed, "add", ".")
    git(seed, "commit", "-m", "seed")
    git(seed, "branch", "-M", "main")

    origin = tmp_root / "origin.git"
    git(seed, "init", "--bare", str(origin))
    git(seed, "remote", "add", "origin", str(origin))
    git(seed, "push", "-u", "origin", "main")
    git(origin, "symbolic-ref", "HEAD", "refs/heads/main")
    return origin


def clone_origin(origin: Path, dst: Path) -> None:
    git(dst.parent, "clone", str(origin), str(dst))
    git(dst, "checkout", "-B", "main", "origin/main")
    git(dst, "config", "user.email", "perf@example.com")
    git(dst, "config", "user.name", "Perf")


def seed_reuse_cache(binary: Path, origin: Path, cache_root: Path) -> None:
    seed_clone = cache_root.parent / "seed-clone"
    clone_origin(origin, seed_clone)
    env = {"CGREP_REUSE_CACHE_DIR": str(cache_root)}
    run([str(binary), "index", "--reuse", "strict", "--embeddings", "off"], seed_clone, env)


def supports_reuse_flag(binary: Path) -> bool:
    probe = subprocess.run(
        [str(binary), "index", "--help"],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        check=False,
    )
    return "--reuse" in (probe.stdout or "")


def measure_for_binary(
    binary: Path, runs: int, warmup: int, file_count: int
) -> tuple[dict[str, float], dict[str, dict[str, float]]]:
    with tempfile.TemporaryDirectory(prefix="cgrep-m5-perf-") as tmp:
        scenario = Path(tmp)
        origin = setup_origin(scenario, file_count)
        cache_root = scenario / "cache"
        cache_root.mkdir(parents=True, exist_ok=True)
        reuse_supported = supports_reuse_flag(binary)
        if reuse_supported:
            seed_reuse_cache(binary, origin, cache_root)

        counter = {"value": 0}

        def next_clone_dir(name: str) -> Path:
            counter["value"] += 1
            dst = scenario / f"{name}-{counter['value']}"
            return dst

        env = {"CGREP_REUSE_CACHE_DIR": str(cache_root)}

        def index_worker(reuse_mode: str) -> float:
            clone = next_clone_dir(f"idx-{reuse_mode}")
            clone_origin(origin, clone)
            if reuse_supported:
                cmd = [str(binary), "index", "--reuse", reuse_mode, "--embeddings", "off"]
            else:
                cmd = [str(binary), "index", "--embeddings", "off"]
            return timed_ms(cmd, clone, env)

        def first_search_worker(reuse_mode: str) -> float:
            clone = next_clone_dir(f"search-{reuse_mode}")
            clone_origin(origin, clone)
            if reuse_supported:
                index_cmd = [str(binary), "index", "--reuse", reuse_mode, "--embeddings", "off"]
            else:
                index_cmd = [str(binary), "index", "--embeddings", "off"]
            run(index_cmd, clone, env)
            return timed_ms(
                [
                    str(binary),
                    "--format",
                    "json2",
                    "--compact",
                    "search",
                    "reuse_probe_token",
                    "--limit",
                    "100",
                ],
                clone,
                env,
            )

        off_index = collect_metric_samples(runs, warmup, lambda: index_worker("off"))
        if reuse_supported:
            strict_index = collect_metric_samples(runs, warmup, lambda: index_worker("strict"))
            auto_index = collect_metric_samples(runs, warmup, lambda: index_worker("auto"))
            strict_first_search = collect_metric_samples(
                runs, warmup, lambda: first_search_worker("strict")
            )
            auto_first_search = collect_metric_samples(
                runs, warmup, lambda: first_search_worker("auto")
            )
        else:
            strict_index = off_index[:]
            auto_index = off_index[:]
            strict_first_search = collect_metric_samples(
                runs, warmup, lambda: first_search_worker("off")
            )
            auto_first_search = strict_first_search[:]

        off_summary = summarize_latency_ms(off_index)
        strict_summary = summarize_latency_ms(strict_index)
        auto_summary = summarize_latency_ms(auto_index)
        strict_search_summary = summarize_latency_ms(strict_first_search)
        auto_search_summary = summarize_latency_ms(auto_first_search)

        metrics = {
            "reuse_off_index_ms": off_summary["p50"],
            "reuse_strict_index_ms": strict_summary["p50"],
            "reuse_auto_index_ms": auto_summary["p50"],
            "first_search_after_strict_ms": strict_search_summary["p50"],
            "first_search_after_auto_ms": auto_search_summary["p50"],
        }
        percentiles = {
            "reuse_off_index_ms": off_summary,
            "reuse_strict_index_ms": strict_summary,
            "reuse_auto_index_ms": auto_summary,
            "first_search_after_strict_ms": strict_search_summary,
            "first_search_after_auto_ms": auto_search_summary,
        }
        return metrics, percentiles


def regression_pct(before: float, after: float) -> float:
    if before <= 0:
        return 0.0
    return ((after - before) / before) * 100.0


def relative_pct(reference: float, value: float) -> float:
    if reference <= 0:
        return 0.0
    return ((value - reference) / reference) * 100.0


def main() -> int:
    parser = argparse.ArgumentParser(description="Index warm-start reuse performance gate")
    parser.add_argument("--baseline-bin", required=True, help="Path to baseline cgrep binary")
    parser.add_argument("--candidate-bin", required=True, help="Path to candidate cgrep binary")
    parser.add_argument("--runs", type=int, default=3, help="Measured runs per metric")
    parser.add_argument("--warmup", type=int, default=1, help="Warmup runs before measurements")
    parser.add_argument("--files", type=int, default=1200, help="Synthetic source files per run")
    parser.add_argument("--json-out", default="", help="Optional path to write JSON report")
    args = parser.parse_args()

    if args.runs < 1:
        print("--runs must be >= 1")
        return 2
    if args.warmup < 0:
        print("--warmup must be >= 0")
        return 2
    if args.files < 1:
        print("--files must be >= 1")
        return 2

    baseline_bin = Path(args.baseline_bin).resolve()
    candidate_bin = Path(args.candidate_bin).resolve()
    if not baseline_bin.exists():
        print(f"baseline binary not found: {baseline_bin}")
        return 2
    if not candidate_bin.exists():
        print(f"candidate binary not found: {candidate_bin}")
        return 2

    baseline, baseline_percentiles = measure_for_binary(
        baseline_bin, args.runs, args.warmup, args.files
    )
    candidate, candidate_percentiles = measure_for_binary(
        candidate_bin, args.runs, args.warmup, args.files
    )

    regressions = {
        "reuse_off_index_ms": round(
            regression_pct(baseline["reuse_off_index_ms"], candidate["reuse_off_index_ms"]), 2
        ),
        "candidate_strict_vs_off_pct": round(
            relative_pct(candidate["reuse_off_index_ms"], candidate["reuse_strict_index_ms"]), 2
        ),
        "candidate_auto_vs_off_pct": round(
            relative_pct(candidate["reuse_off_index_ms"], candidate["reuse_auto_index_ms"]), 2
        ),
    }

    limits = {
        "reuse_off_index_ms": 5.0,
        "candidate_strict_vs_off_pct": 10.0,
        "candidate_auto_vs_off_pct": 10.0,
    }

    payload = {
        "runs": args.runs,
        "warmup": args.warmup,
        "files": args.files,
        "methodology": {
            "latency_p50": "median",
            "latency_p95": "nearest-rank",
            "measured_runs_per_metric": args.runs,
            "warmup_runs_per_metric": args.warmup,
        },
        "baseline": baseline,
        "candidate": candidate,
        "percentiles": {
            "baseline": baseline_percentiles,
            "candidate": candidate_percentiles,
        },
        "regression_pct": regressions,
        "limits": limits,
    }
    print(json.dumps(payload, indent=2))

    if args.json_out:
        out_path = Path(args.json_out)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")

    failed = [key for key, limit in limits.items() if regressions.get(key, 0.0) > limit]
    if failed:
        print(f"\nPerf gate failed: {', '.join(failed)}")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
