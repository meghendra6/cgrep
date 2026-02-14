#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0

import json
import statistics
import subprocess
import tempfile
import time
from pathlib import Path


def run(cmd, cwd: Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        cmd,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=True,
    )


def measure_ms(cmd, cwd: Path, runs: int = 7) -> float:
    # Warm-up
    run(cmd, cwd)
    values = []
    for _ in range(runs):
        t0 = time.perf_counter()
        run(cmd, cwd)
        values.append((time.perf_counter() - t0) * 1000.0)
    return statistics.median(values)


def generate_repo(root: Path) -> None:
    src = root / "src"
    src.mkdir(parents=True, exist_ok=True)

    for i in range(180):
        code = f"""
pub struct Worker{i} {{}}

pub fn target_fn_{i}(x: i32) -> i32 {{
    x + {i}
}}

pub fn call_target_{i}(n: i32) -> i32 {{
    let mut acc = 0;
    for k in 0..n {{
        acc += target_fn_{i}(k);
    }}
    acc
}}
"""
        (src / f"mod_{i}.rs").write_text(code.strip() + "\n", encoding="utf-8")

    (src / "shared.rs").write_text(
        """
pub fn shared_target(value: i32) -> i32 {
    value * 2
}

pub fn invoke_shared() -> i32 {
    shared_target(7)
}
""".strip()
        + "\n",
        encoding="utf-8",
    )


def main() -> int:
    repo_root = Path(__file__).resolve().parents[1]
    binary = repo_root / "target" / "release" / "cgrep"
    if not binary.exists():
        print("release binary missing; run `cargo build --release` first")
        return 2

    with tempfile.TemporaryDirectory(prefix="cgrep-perf-") as tmp:
        work = Path(tmp)
        generate_repo(work)

        run([str(binary), "index", "--embeddings", "off"], cwd=work)

        metrics = {
            "search_index_ms": measure_ms(
                [str(binary), "--format", "json", "--compact", "search", "shared_target"], work
            ),
            "symbols_ms": measure_ms(
                [str(binary), "--format", "json", "--compact", "symbols", "shared_target"], work
            ),
            "definition_ms": measure_ms(
                [
                    str(binary),
                    "--format",
                    "json",
                    "--compact",
                    "definition",
                    "shared_target",
                ],
                work,
            ),
            "references_ms": measure_ms(
                [
                    str(binary),
                    "--format",
                    "json",
                    "--compact",
                    "references",
                    "shared_target",
                ],
                work,
            ),
            "callers_ms": measure_ms(
                [
                    str(binary),
                    "--format",
                    "json",
                    "--compact",
                    "callers",
                    "shared_target",
                ],
                work,
            ),
        }

        limits = []
        limits.append(
            (
                "definition_vs_symbols",
                metrics["definition_ms"] <= max(metrics["symbols_ms"] * 2.5, 120.0),
            )
        )
        limits.append(
            (
                "definition_vs_references",
                metrics["definition_ms"] <= max(metrics["references_ms"] * 3.0, 150.0),
            )
        )
        limits.append(
            (
                "references_vs_search",
                metrics["references_ms"] <= max(metrics["search_index_ms"] * 2.5, 150.0),
            )
        )
        limits.append(("absolute_definition", metrics["definition_ms"] <= 1500.0))
        limits.append(("absolute_references", metrics["references_ms"] <= 1500.0))
        limits.append(("absolute_callers", metrics["callers_ms"] <= 1500.0))

        payload = {
            "metrics_ms": {k: round(v, 2) for k, v in metrics.items()},
            "checks": [{name: ok} for name, ok in limits],
        }
        print(json.dumps(payload, indent=2))

        failed = [name for name, ok in limits if not ok]
        if failed:
            print("\nPerf gate failed:", ", ".join(failed))
            return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
