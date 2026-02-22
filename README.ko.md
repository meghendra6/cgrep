# cgrep (한국어)

[English](./README.en.md) | [한국어](./README.ko.md) | [中文](./README.zh.md)

개발자와 AI 코딩 에이전트를 위한 로컬 우선 코드 검색 도구입니다.

`grep`은 문자열이 나온 위치를 찾고, `cgrep`은 실제 구현 지점을 빠르게 좁혀줍니다.

## 왜 cgrep인가

- Tantivy 인덱스 기반의 빠른 로컬 검색 (클라우드 의존 없음)
- 코드 탐색 명령: `definition`, `references`, `callers`, `read`, `map`
- 에이전트 친화 출력: `--format json2 --compact`
- Codex, Claude Code, Cursor, Copilot 등 MCP 연동 지원

## 30초 설치

```bash
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh | bash
cgrep --help
```

## 2분 시작

```bash
# 선택: 워밍업
cgrep index

# 일상 검색 흐름
cgrep s "token validation" src/
cgrep d handle_auth
cgrep r UserService
cgrep read src/auth.rs
cgrep map --depth 2
```

## AI 코딩 에이전트

### 1) 1회 설치 (사용 호스트 선택)

```bash
cgrep agent install codex
cgrep agent install claude-code
cgrep agent install cursor
cgrep agent install copilot
cgrep agent install opencode
```

### 2) 필수/선택 작업 구분

- 필수: 설치 후 현재 에이전트 세션을 한 번 재시작
- 일반 사용에서 불필요: 수동 `cgrep index`, `cgrep daemon start`
- 선택: 변경이 매우 잦은 장시간 세션에서는 daemon 실행이 유리

### 선택: CLI 조회 예시

```bash
ID=$(cgrep agent locate "where token validation happens" --compact | jq -r '.results[0].id')
cgrep agent expand --id "$ID" -C 8 --compact
cgrep --format json2 --compact agent plan "trace authentication middleware flow"
```

## 인덱싱 규칙 (간단 버전)

- 가끔 사용: `search/definition/read`를 바로 실행 (auto bootstrap)
- 활발한 코딩 세션: `cgrep daemon start` 후 `cgrep daemon stop`
- semantic/hybrid 검색: experimental, embeddings 인덱스 필요

## 벤치마크 스냅샷 (PyTorch, Codex, runs=2)

- 날짜: **2026-02-22 (UTC)**
- baseline billable tokens: **151,466**
- cgrep billable tokens: **69,874**
- 절감률: **53.9%**

상세 리포트: [`docs/benchmarks/pytorch-codex-agent-efficiency.md`](./docs/benchmarks/pytorch-codex-agent-efficiency.md)

## 문서

- 문서 사이트: <https://meghendra6.github.io/cgrep/>
- 설치: [`docs/installation.md`](./docs/installation.md)
- 사용법: [`docs/usage.md`](./docs/usage.md)
- 에이전트: [`docs/agent.md`](./docs/agent.md)
- MCP: [`docs/mcp.md`](./docs/mcp.md)
- 인덱싱/daemon: [`docs/indexing-watch.md`](./docs/indexing-watch.md)
- 문제 해결: [`docs/troubleshooting.md`](./docs/troubleshooting.md)

## 릴리즈

- 현재 버전: **v1.5.2**
- 변경 이력: [`CHANGELOG.md`](./CHANGELOG.md)
