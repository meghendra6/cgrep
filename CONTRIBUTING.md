# Contributing to cgrep

Thanks for contributing.

## Development Setup

1. Install Rust stable.
2. Clone the repository.
3. Build once:

```bash
cargo build
```

## Required Checks Before PR

Run these locally before opening a pull request:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test -q
mkdocs build --strict
```

## Coding Guidelines

- Keep changes minimal and scoped to one goal.
- Add or update tests for behavior changes.
- Prefer deterministic output for agent-facing paths (`json2`, compact).
- Update docs when CLI behavior, defaults, or workflows change.

## Commit and PR Guidelines

- Use clear, action-oriented commit messages (for example: `fix(mcp): ...`).
- Split unrelated changes into separate commits.
- In PR descriptions, include:
  - What changed
  - Why it changed
  - Validation steps and results

## Reporting Bugs

Please include:

- cgrep version (`cgrep --version`)
- OS and architecture
- Exact command and input
- Expected vs actual result
- Minimal reproduction steps
