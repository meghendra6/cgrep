# 설치

## 사전 준비

- Rust 툴체인(stable)
- `.cgrep/` 인덱스를 쓸 수 있는 프로젝트 디렉터리

## 소스에서 설치

```bash
cargo install --path .
```

## GitHub 릴리즈 바이너리 설치 (대부분 사용자 권장)

```bash
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh \
  | bash
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

## 수동 빌드

```bash
cargo build --release
cp target/release/cgrep ~/.local/bin/
```

## 릴리즈 자산 수동 검증

```bash
# 예시 (macOS Apple Silicon)
TAG=v1.4.6
ASSET="cgrep-${TAG}-aarch64-apple-darwin.tar.gz"
curl -LO "https://github.com/meghendra6/cgrep/releases/download/${TAG}/${ASSET}"
curl -LO "https://github.com/meghendra6/cgrep/releases/download/${TAG}/${ASSET}.sha256"
shasum -a 256 -c "${ASSET}.sha256"
```

## macOS Gatekeeper 참고

브라우저로 받은 바이너리는 quarantine 속성 때문에 차단될 수 있습니다.

```bash
xattr -d com.apple.quarantine ~/.local/bin/cgrep
```

## 설치 확인

```bash
cgrep --help
```

## 최초 설정

```bash
# 저장소에서 초기 인덱스 생성
cgrep index

# 선택: 셸 완성 스크립트 생성
cgrep completions zsh
```
