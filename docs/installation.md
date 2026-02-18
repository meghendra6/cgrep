# Installation

## Quick Install (Recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh | bash
```

## Choose Your Install Path

### 1) Release binary (most users)

```bash
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh | bash
```

Pin a version:

```bash
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh \
  | bash -s -- --version <tag>
```

Custom binary directory:

```bash
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh \
  | bash -s -- --bin-dir ~/.local/bin
```

### 2) Install from source

Prerequisite: Rust stable toolchain

```bash
cargo install --path .
```

### 3) Build manually

```bash
cargo build --release
cp target/release/cgrep ~/.local/bin/
```

## Verify Install

```bash
cgrep --help
```

## First Run In A Repository

```bash
cgrep index
cgrep s "token validation" src/
```

## Optional: Shell Completions

```bash
cgrep completions zsh
```

## macOS Gatekeeper Note

If macOS blocks a downloaded binary:

```bash
xattr -d com.apple.quarantine ~/.local/bin/cgrep
```

## Manual Release Asset Verification

```bash
TAG=v1.4.6
ASSET="cgrep-${TAG}-aarch64-apple-darwin.tar.gz"
curl -LO "https://github.com/meghendra6/cgrep/releases/download/${TAG}/${ASSET}"
curl -LO "https://github.com/meghendra6/cgrep/releases/download/${TAG}/${ASSET}.sha256"
shasum -a 256 -c "${ASSET}.sha256"
```
