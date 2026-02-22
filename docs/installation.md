# Installation

## Recommended (Release Binary)

```bash
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh | bash
cgrep --help
```

## Install a Specific Version

```bash
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh \
  | bash -s -- --version v1.5.2
```

## Custom Install Directory

```bash
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh \
  | bash -s -- --bin-dir ~/.local/bin
```

## Build From Source

```bash
cargo build --release
cp target/release/cgrep ~/.local/bin/
```

## First Commands

```bash
cgrep index          # optional warm-up
cgrep s "token validation" src/
cgrep d handle_auth
```

## macOS Gatekeeper (if blocked)

```bash
xattr -d com.apple.quarantine ~/.local/bin/cgrep
```

## Release Asset Check (Optional)

```bash
TAG=v1.5.2
ASSET="cgrep-${TAG}-aarch64-apple-darwin.tar.gz"
curl -LO "https://github.com/meghendra6/cgrep/releases/download/${TAG}/${ASSET}"
curl -LO "https://github.com/meghendra6/cgrep/releases/download/${TAG}/${ASSET}.sha256"
shasum -a 256 -c "${ASSET}.sha256"
```
