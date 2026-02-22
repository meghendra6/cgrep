# cgrep 문서

개발자와 AI 코딩 에이전트를 위한 로컬 우선 코드 검색 문서입니다.

현재 릴리즈: **v1.5.2**

## 2분 시작

```bash
# 설치
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh | bash

# 검색 + 탐색
cgrep s "token validation" src/
cgrep d handle_auth
cgrep read src/auth.rs
```

## 목적별 문서

| 목적 | 문서 |
|---|---|
| 빠른 설치 | [설치](./installation.md) |
| 일상 명령 익히기 | [사용법](./usage.md) |
| AI 에이전트 연동 | [에이전트 워크플로](./agent.md) |
| MCP 연동 | [MCP](./mcp.md) |
| 코딩 중 인덱스 유지 | [인덱싱과 Daemon](./indexing-watch.md) |
| 문제 해결 | [문제 해결](./troubleshooting.md) |

## AI 에이전트 설정 (필수/선택)

```bash
# 1회 설치 (하나 선택)
cgrep agent install codex
cgrep agent install claude-code
cgrep agent install cursor
cgrep agent install copilot
cgrep agent install opencode
```

- 필수: 설치 후 에이전트 세션 재시작 1회
- 일반 사용에서 불필요: 수동 `cgrep index`, 상시 daemon 실행
- 선택: 장시간/고변경 세션에서 daemon 사용
- CLI 조회 예시는 [agent.md](./agent.md) 참고

## 벤치마크 스냅샷 (PyTorch, Codex, runs=2)

- 날짜: **2026-02-22 (UTC)**
- baseline billable tokens: **151,466**
- cgrep billable tokens: **69,874**
- 절감률: **53.9%**

리포트:
- [Codex 에이전트 효율](../benchmarks/pytorch-codex-agent-efficiency.md)
- [검색 옵션 성능](../benchmarks/pytorch-search-options-performance.md)
- [에이전트 토큰 효율](../benchmarks/pytorch-agent-token-efficiency.md)

## 언어

- 영어 문서 허브: [../index.md](../index.md)
- 저장소 README (EN/KO/中文): [README.md](https://github.com/meghendra6/cgrep/blob/main/README.md)
