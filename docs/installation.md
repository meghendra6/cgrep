# Installation

## Prerequisites

- Rust toolchain (stable)
- A writable project directory for `.cgrep/` index files

## Install from source

```bash
cargo install --path .
```

## Install from GitHub release (recommended for most users)

```bash
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh \
  | bash
```

Pin a specific version:

```bash
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh \
  | bash -s -- --version v1.4.1
```

Custom install directory:

```bash
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh \
  | bash -s -- --bin-dir ~/.local/bin
```

## Build manually

```bash
cargo build --release
cp target/release/cgrep ~/.local/bin/
```

## Manual release asset verification

```bash
# Example (macOS Apple Silicon)
curl -LO https://github.com/meghendra6/cgrep/releases/download/v1.4.1/cgrep-v1.4.1-aarch64-apple-darwin.tar.gz
curl -LO https://github.com/meghendra6/cgrep/releases/download/v1.4.1/cgrep-v1.4.1-aarch64-apple-darwin.tar.gz.sha256
shasum -a 256 -c cgrep-v1.4.1-aarch64-apple-darwin.tar.gz.sha256
```

## macOS Gatekeeper note

Downloaded binaries can be quarantined by the browser.

```bash
xattr -d com.apple.quarantine ~/.local/bin/cgrep
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
