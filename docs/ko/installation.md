# 설치

## 권장 설치 (릴리즈 바이너리)

```bash
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh | bash
cgrep --help
```

## 특정 버전 설치

```bash
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh \
  | bash -s -- --version v1.5.2
```

## 설치 경로 지정

```bash
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh \
  | bash -s -- --bin-dir ~/.local/bin
```

## 소스 빌드 설치

```bash
cargo build --release
cp target/release/cgrep ~/.local/bin/
```

## 첫 실행 명령

```bash
cgrep index          # 선택: 워밍업
cgrep s "token validation" src/
cgrep d handle_auth
```

## macOS Gatekeeper 차단 시

```bash
xattr -d com.apple.quarantine ~/.local/bin/cgrep
```

## 릴리즈 자산 무결성 확인 (선택)

```bash
TAG=v1.5.2
ASSET="cgrep-${TAG}-aarch64-apple-darwin.tar.gz"
curl -LO "https://github.com/meghendra6/cgrep/releases/download/${TAG}/${ASSET}"
curl -LO "https://github.com/meghendra6/cgrep/releases/download/${TAG}/${ASSET}.sha256"
shasum -a 256 -c "${ASSET}.sha256"
```
