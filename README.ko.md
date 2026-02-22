# cgrep (한국어)

[English](./README.en.md) | [한국어](./README.ko.md) | [中文](./README.zh.md)

사람과 AI 코딩 에이전트를 위한 로컬 우선 코드 검색 도구입니다.

`grep`은 텍스트를 찾고, `cgrep`은 구현 의도를 찾습니다.

## 왜 cgrep인가

- Tantivy 인덱스 기반의 빠른 로컬 검색 (클라우드 의존 없음)
- 코드 탐색 명령: `definition`, `references`, `callers`, `read`, `map`
- 에이전트 친화 출력: `--format json2 --compact`
- Codex/Claude/Cursor/VS Code용 MCP 연동

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

## AI 코딩 에이전트용

```bash
# Codex용 가이드 + MCP 설정
cgrep agent install codex

# 저토큰 2단계 조회
ID=$(cgrep agent locate "where token validation happens" --compact | jq -r '.results[0].id')
cgrep agent expand --id "$ID" -C 8 --compact

# 결정적 plan 출력
cgrep --format json2 --compact agent plan "trace authentication middleware flow"
```

## 인덱싱 선택 규칙

- 가끔 사용: `search/definition/read`를 바로 실행 (auto bootstrap)
- 활발한 코딩 세션: `cgrep daemon start` 후 종료 시 `cgrep daemon stop`
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
