# 설치

## 빠른 설치 (권장)

```bash
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh | bash
```

## 설치 방법 선택

### 1) 릴리즈 바이너리 설치 (대부분 사용자 권장)

```bash
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh | bash
```

특정 버전 고정:

```bash
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh \
  | bash -s -- --version <tag>
```

설치 경로 지정:

```bash
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh \
  | bash -s -- --bin-dir ~/.local/bin
```

### 2) 소스에서 설치

사전 조건: Rust stable 툴체인

```bash
cargo install --path .
```

### 3) 수동 빌드

```bash
cargo build --release
cp target/release/cgrep ~/.local/bin/
```

## 설치 확인

```bash
cgrep --help
```

## 저장소에서 첫 실행

```bash
cgrep index
cgrep s "token validation" src/
```

## 선택: 셸 자동완성

```bash
cgrep completions zsh
```

## macOS Gatekeeper 참고

다운로드한 바이너리가 차단되면:

```bash
xattr -d com.apple.quarantine ~/.local/bin/cgrep
```

## 릴리즈 자산 수동 검증

```bash
TAG=v1.4.6
ASSET="cgrep-${TAG}-aarch64-apple-darwin.tar.gz"
curl -LO "https://github.com/meghendra6/cgrep/releases/download/${TAG}/${ASSET}"
curl -LO "https://github.com/meghendra6/cgrep/releases/download/${TAG}/${ASSET}.sha256"
shasum -a 256 -c "${ASSET}.sha256"
```
