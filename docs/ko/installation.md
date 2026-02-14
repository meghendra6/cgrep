# 설치

## 사전 준비

- Rust 툴체인(stable)
- `.cgrep/` 인덱스를 쓸 수 있는 프로젝트 디렉터리

## 소스에서 설치

```bash
cargo install --path .
```

## 수동 빌드

```bash
cargo build --release
cp target/release/cgrep ~/.local/bin/
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
