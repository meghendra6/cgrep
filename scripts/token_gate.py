#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0

import json
import math
import statistics
import subprocess
import tempfile
import time
from pathlib import Path


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )


def percentile(values: list[float], p: float) -> float:
    if not values:
        return 0.0
    arr = sorted(values)
    if p <= 0:
        return arr[0]
    if p >= 100:
        return arr[-1]
    pos = (len(arr) - 1) * (p / 100.0)
    lo = int(math.floor(pos))
    hi = int(math.ceil(pos))
    if lo == hi:
        return arr[lo]
    frac = pos - lo
    return arr[lo] * (1.0 - frac) + arr[hi] * frac


def estimate_tokens(text: str) -> int:
    return (len(text) + 3) // 4


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def generate_repo(root: Path) -> None:
    write(root / ".gitignore", "target/\n")
    write(
        root / "src/auth.rs",
        "\n".join(
            [
                "pub fn validate_token(input: &str) -> bool {",
                "    input.starts_with(\"tok_\")",
                "}",
                "pub fn refresh_token(input: &str) -> String {",
                "    format!(\"{}-refresh\", input)",
                "}",
            ]
        )
        + "\n",
    )
    write(
        root / "src/dispatch.rs",
        "\n".join(
            [
                "pub struct DispatchKeySet;",
                "pub fn get_runtime_dispatch_key_set() -> DispatchKeySet {",
                "    DispatchKeySet",
                "}",
            ]
        )
        + "\n",
    )

    noisy_line = "validate_token noisy target payload\n"
    for i in range(250):
        write(root / "target" / f"noise_{i}.rs", noisy_line * 40)


def resolved_paths(payload: dict) -> list[str]:
    aliases = payload.get("meta", {}).get("path_aliases", {}) or {}
    out: list[str] = []
    for row in payload.get("results", []):
        raw = row.get("path")
        if not isinstance(raw, str):
            continue
        out.append(aliases.get(raw, raw))
    return out


def main() -> int:
    repo_root = Path(__file__).resolve().parents[1]
    binary = repo_root / "target" / "release" / "cgrep"
    if not binary.exists():
        print("release binary missing; run `cargo build --release` first")
        return 2

    with tempfile.TemporaryDirectory(prefix="cgrep-token-gate-") as tmp:
        work = Path(tmp)
        generate_repo(work)

        index_cmd = [str(binary), "index", "--embeddings", "off"]
        indexed = run(index_cmd, work)
        if indexed.returncode != 0:
            print(indexed.stderr.strip() or indexed.stdout.strip())
            return 1

        scenarios = [
            ("validate_token", "src/auth.rs"),
            ("refresh_token", "src/auth.rs"),
            ("DispatchKeySet", "src/dispatch.rs"),
            ("get_runtime_dispatch_key_set", "src/dispatch.rs"),
        ]

        reductions: list[float] = []
        baseline_latencies: list[float] = []
        cgrep_latencies: list[float] = []
        misses: list[str] = []

        for query, expected_path in scenarios:
            t0 = time.perf_counter()
            baseline = run(["grep", "-R", "-n", "-I", query, "."], work)
            baseline_ms = (time.perf_counter() - t0) * 1000.0
            if baseline.returncode not in (0, 1):
                print(baseline.stderr.strip() or baseline.stdout.strip())
                return 1

            t1 = time.perf_counter()
            cgrep = run(
                [
                    str(binary),
                    "--format",
                    "json2",
                    "--compact",
                    "search",
                    query,
                    "-B",
                    "balanced",
                    "--path-alias",
                    "--dedupe-context",
                    "--suppress-boilerplate",
                ],
                work,
            )
            cgrep_ms = (time.perf_counter() - t1) * 1000.0
            if cgrep.returncode != 0:
                print(cgrep.stderr.strip() or cgrep.stdout.strip())
                return 1

            baseline_tokens = estimate_tokens(baseline.stdout)
            cgrep_tokens = estimate_tokens(cgrep.stdout)
            if baseline_tokens <= 0:
                baseline_tokens = 1
            reduction = 1.0 - (cgrep_tokens / baseline_tokens)
            reductions.append(reduction)
            baseline_latencies.append(baseline_ms)
            cgrep_latencies.append(cgrep_ms)

            payload = json.loads(cgrep.stdout)
            paths = resolved_paths(payload)
            if not any(expected_path in p for p in paths):
                misses.append(f"{query} -> {expected_path}")

        p95_reduction = percentile(reductions, 95) * 100.0
        p95_baseline_latency = percentile(baseline_latencies, 95)
        p95_cgrep_latency = percentile(cgrep_latencies, 95)
        latency_ratio = (
            (p95_cgrep_latency / p95_baseline_latency)
            if p95_baseline_latency > 0
            else float("inf")
        )

        summary = {
            "p95_reduction_percent": round(p95_reduction, 2),
            "p95_baseline_latency_ms": round(p95_baseline_latency, 2),
            "p95_cgrep_latency_ms": round(p95_cgrep_latency, 2),
            "latency_ratio": round(latency_ratio, 3),
            "misses": misses,
        }
        print(json.dumps(summary, indent=2))

        if misses:
            print("\nToken gate failed: missing expected evidence.")
            return 1
        if p95_reduction < 30.0:
            print("\nToken gate failed: P95 token reduction is below 30%.")
            return 1
        if latency_ratio > 1.2:
            print("\nToken gate failed: P95 latency regression is above 20%.")
            return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
