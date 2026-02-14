# Installation

## Prerequisites

- Rust toolchain (stable)
- A writable project directory for `.cgrep/` index files

## Install from source

```bash
cargo install --path .
```

## Build manually

```bash
cargo build --release
cp target/release/cgrep ~/.local/bin/
```

## Verify install

```bash
cgrep --help
```

## First-time setup

```bash
# Build initial index in your repository
cgrep index

# Optional: generate shell completions
cgrep completions zsh
```
