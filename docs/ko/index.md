# cgrep 문서 (한국어)

사람과 AI 에이전트를 위한 로컬 우선 코드 검색/탐색 문서 허브입니다.

현재 릴리즈: **v1.5.2**

## 목적별 시작점

| 하고 싶은 일 | 문서 |
|---|---|
| 빠르게 설치하고 첫 명령 실행 | [설치](./installation.md) |
| 일상 검색/탐색 명령 익히기 | [사용법](./usage.md) |
| 저토큰 에이전트 조회 흐름 적용 | [에이전트 워크플로](./agent.md) |
| 에디터/호스트 MCP 연동 | [MCP](./mcp.md) |
| 대형 저장소 인덱스 운용 | [인덱싱과 Daemon](./indexing-watch.md) |
| 백그라운드/reuse 운영 점검 | [운영 가이드](./operations.md) |
| 기본값/프로필 튜닝 | [설정](./configuration.md) |
| semantic/hybrid 검색 사용 (experimental) | [임베딩](./embeddings.md) |
| 자주 발생하는 문제 해결 | [문제 해결](./troubleshooting.md) |
| 빌드/테스트/성능 검증 | [개발](./development.md) |

## 자주 쓰는 흐름

### 사용자 흐름 (2분)

```bash
# 선택: 워밍업 인덱스
cgrep index
cgrep s "token validation" src/
cgrep d handle_auth
cgrep read src/auth.rs
```

### 에이전트 흐름 (저토큰 조회)

```bash
ID=$(cgrep agent locate "where token validation happens" --compact | jq -r '.results[0].id')
cgrep agent expand --id "$ID" -C 8 --compact
cgrep --format json2 --compact agent plan "trace authentication middleware flow"
```

## 인덱싱 선택 빠른 가이드

| 상황 | 권장 선택 |
|---|---|
| 바로 검색 시작 | `search/read/definition`을 바로 실행 (auto-bootstrap) |
| 코딩 중 인덱스를 계속 hot 유지 | 세션 동안 `cgrep daemon start`, 종료 시 `cgrep daemon stop` |
| 1회성 비동기 사전 빌드 | `cgrep index --background` |
| semantic/hybrid 실험 | embeddings 인덱스 빌드 (`cgrep index --embeddings precompute`) |

## 벤치마크 문서

- [에이전트 토큰 효율 벤치마크 (PyTorch, 영문)](../benchmarks/pytorch-agent-token-efficiency.md)
- [Codex 에이전트 효율 벤치마크 (PyTorch, 영문)](../benchmarks/pytorch-codex-agent-efficiency.md)
- [검색 옵션 성능 벤치마크 (PyTorch, 영문)](../benchmarks/pytorch-search-options-performance.md)
- 최신 Codex 스냅샷(UTC 2026-02-22): cgrep 청구 토큰 **107,990**, baseline **158,242** 대비 **31.8% 감소** (`runs=1`).
- 동일 일자 반복 측정 변동폭이 큽니다(총 토큰 기준 **-135.4% ~ +31.8%**). 의사결정에는 다회 실행 median을 권장합니다.
- 최신 측정 수치는 각 벤치마크 문서에 유지됩니다.

## 언어/사이트

- 영어 문서 허브: [../index.md](../index.md)
- 공식 문서 사이트: <https://meghendra6.github.io/cgrep/>
- 저장소 README: [README.md](https://github.com/meghendra6/cgrep/blob/main/README.md)

## 관련 파일

- 변경 이력: [CHANGELOG.md](https://github.com/meghendra6/cgrep/blob/main/CHANGELOG.md)
- 비교 문서: [COMPARISON.md](https://github.com/meghendra6/cgrep/blob/main/COMPARISON.md)
- 기여 가이드: [CONTRIBUTING.md](https://github.com/meghendra6/cgrep/blob/main/CONTRIBUTING.md)
- 보안 정책: [SECURITY.md](https://github.com/meghendra6/cgrep/blob/main/SECURITY.md)
