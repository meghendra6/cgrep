#!/usr/bin/env bash

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CGREP_BIN="${CGREP_BIN:-cgrep}"

require_command() {
  local name="$1"
  if ! command -v "$name" >/dev/null 2>&1; then
    echo "missing required command: $name" >&2
    exit 2
  fi
}

if [[ "$CGREP_BIN" == */* ]]; then
  if [[ ! -x "$CGREP_BIN" ]]; then
    echo "CGREP_BIN is not executable: $CGREP_BIN" >&2
    exit 2
  fi
else
  require_command "$CGREP_BIN"
fi

require_command git
require_command python3
require_command bash

TMP_DIR="$(mktemp -d -t cgrep-validate-XXXXXX)"
FIXTURE="$TMP_DIR/repo"
trap 'rm -rf "$TMP_DIR"' EXIT

mkdir -p "$FIXTURE/src" "$FIXTURE/docs"

cat > "$FIXTURE/src/lib.rs" <<'EOF'
pub fn token_validation_marker(input: &str) -> bool {
    input.starts_with("tok_")
}
EOF

cat > "$FIXTURE/src/flow.rs" <<'EOF'
pub fn run_auth_flow() {
    if crate::token_validation_marker("tok_seed") {
        println!("ok");
    }
}
EOF

cat > "$FIXTURE/docs/guide.md" <<'EOF'
authentication middleware retry flow guide
EOF

git -C "$FIXTURE" init -q
git -C "$FIXTURE" config user.email "ci@example.com"
git -C "$FIXTURE" config user.name "CI"
git -C "$FIXTURE" add .
git -C "$FIXTURE" commit -q -m "seed"

echo "[validate] core index/search"
"$CGREP_BIN" index --path "$FIXTURE" --embeddings off --reuse off >/dev/null
"$CGREP_BIN" --format json2 --compact search "token_validation_marker" -p "$FIXTURE/src" -m 5 > "$TMP_DIR/search_1.json"
"$CGREP_BIN" --format json2 --compact search "token_validation_marker" -p "$FIXTURE/src" -m 5 > "$TMP_DIR/search_2.json"

python3 - "$TMP_DIR/search_1.json" "$TMP_DIR/search_2.json" <<'PY'
import json
import sys

p1, p2 = sys.argv[1], sys.argv[2]
with open(p1, "r", encoding="utf-8") as fh:
    first = json.load(fh)
with open(p2, "r", encoding="utf-8") as fh:
    second = json.load(fh)

for key in ("files_with_matches", "total_matches", "payload_chars", "payload_tokens_estimate"):
    if key not in first["meta"]:
        raise SystemExit(f"missing search.meta field: {key}")

if first["results"] != second["results"]:
    raise SystemExit("search results are not deterministic across identical runs")
PY

echo "[validate] incremental update flow"
cat > "$FIXTURE/src/lib.rs" <<'EOF'
pub fn token_validation_marker(input: &str) -> bool {
    input.starts_with("tok_") && !input.is_empty()
}
EOF

"$CGREP_BIN" index --path "$FIXTURE" --embeddings off --reuse off --print-diff > "$TMP_DIR/print_diff.txt"
if ! grep -q "src/lib.rs" "$TMP_DIR/print_diff.txt"; then
  echo "expected src/lib.rs in --print-diff output" >&2
  exit 1
fi

echo "[validate] agent plan flow"
"$CGREP_BIN" --format json2 --compact agent plan "token_validation_marker" --path "$FIXTURE" --max-steps 6 --max-candidates 4 > "$TMP_DIR/plan_1.json"
"$CGREP_BIN" --format json2 --compact agent plan "token_validation_marker" --path "$FIXTURE" --max-steps 6 --max-candidates 4 > "$TMP_DIR/plan_2.json"
cmp "$TMP_DIR/plan_1.json" "$TMP_DIR/plan_2.json"

python3 - "$TMP_DIR/plan_1.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as fh:
    payload = json.load(fh)

for field in ("meta", "steps", "candidates"):
    if field not in payload:
        raise SystemExit(f"agent plan missing field: {field}")

if payload["meta"].get("stage") != "plan":
    raise SystemExit("agent plan meta.stage must be 'plan'")
PY

echo "[validate] status/stats/doctor flows"
"$CGREP_BIN" --format json2 --compact status --path "$FIXTURE" > "$TMP_DIR/status.json"

python3 - "$TMP_DIR/status.json" "$TMP_DIR/search_1.json" <<'PY'
import json
import sys

status_path, search_path = sys.argv[1], sys.argv[2]
with open(status_path, "r", encoding="utf-8") as fh:
    status = json.load(fh)
with open(search_path, "r", encoding="utf-8") as fh:
    search = json.load(fh)

result = status.get("result", {})
for field in ("phase", "basic_ready", "full_ready", "progress"):
    if field not in result:
        raise SystemExit(f"status.result missing field: {field}")

progress = result["progress"]
for field in ("total", "processed", "failed"):
    if field not in progress:
        raise SystemExit(f"status.result.progress missing field: {field}")

meta = search.get("meta", {})
for field in ("files_with_matches", "total_matches", "elapsed_ms"):
    if field not in meta:
        raise SystemExit(f"search.meta missing stats field: {field}")
PY

bash "$REPO_ROOT/scripts/doctor.sh" "$REPO_ROOT" >/dev/null

echo "[validate] docs link check"
python3 - "$REPO_ROOT" <<'PY'
import pathlib
import re
import sys
import warnings

warnings.filterwarnings("ignore", message="Possible nested set at position")

root = pathlib.Path(sys.argv[1]).resolve()
targets = [
    "README.md",
    "docs/index.md",
    "docs/usage.md",
    "docs/agent.md",
    "docs/configuration.md",
    "docs/indexing-watch.md",
    "docs/operations.md",
    "docs/development.md",
    "docs/troubleshooting.md",
    "docs/ko/index.md",
    "docs/ko/usage.md",
    "docs/ko/agent.md",
    "docs/ko/configuration.md",
    "docs/ko/indexing-watch.md",
    "docs/ko/operations.md",
    "docs/ko/development.md",
]

link_re = re.compile(r"!?\\[[^]]*\\]\\(([^)]+)\\)")
missing = []

for rel_path in targets:
    path = root / rel_path
    if not path.exists():
        missing.append(f"{rel_path}: file missing")
        continue
    text = path.read_text(encoding="utf-8")
    in_fence = False
    filtered_lines = []
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("```"):
            in_fence = not in_fence
            continue
        if not in_fence:
            filtered_lines.append(line)
    filtered = "\n".join(filtered_lines)
    for match in link_re.finditer(filtered):
        target = match.group(1).strip()
        if not target or target.startswith("#"):
            continue
        if target.startswith(("http://", "https://", "mailto:")):
            continue
        if target.startswith("<") and target.endswith(">"):
            inner = target[1:-1]
            if inner.startswith(("http://", "https://", "mailto:")):
                continue
            target = inner
        file_part = target.split("#", 1)[0].split("?", 1)[0]
        candidate = (path.parent / file_part).resolve()
        if not candidate.exists():
            missing.append(f"{rel_path}: missing link target -> {target}")

if missing:
    raise SystemExit("\n".join(missing))
PY

echo "[validate] all checks passed"
