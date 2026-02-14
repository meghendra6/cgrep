# cgrep 문서 (한국어)

`grep`은 텍스트를 찾고, `cgrep`은 코드 의도를 찾습니다.

실제 저장소에서 사람과 AI 에이전트가 함께 작업할 때를 위한 로컬 우선 코드 검색 도구입니다.
현재 릴리즈: **v1.4.1**.

v1.4.1 핵심:
- 자주 쓰는 명령 중심의 단축 CLI.
- `claude-code`, `codex`, `copilot`, `cursor`, `opencode` 에이전트 설치 지원.
- Cursor 포함 MCP 호스트 설치 지원 (`cgrep mcp install cursor`).

- 문서 사이트: <https://meghendra6.github.io/cgrep/>
- 저장소 README: [README.md](https://github.com/meghendra6/cgrep/blob/main/README.md)
- 영어 문서 허브: [../index.md](../index.md)

## 왜 cgrep인가

- AI 코딩 루프에 맞춘 구조: 작고 결정적인 `json2` 출력 + 2단계 `agent locate/expand`.
- 코드 구조 중심 탐색: `definition`, `references`, `callers`, `dependents`, `map`, `read`.
- CLI 사용성: `s`, `d`, `r`, `c`, `dep`, `i`, `a l` 같은 짧은 별칭 제공.
- 로컬 우선 운영: 빠른 검색, 프라이버시 보호, 클라우드 의존 없음.

## 벤치마크 스냅샷 (PyTorch)

2026년 2월 14일 기준, 구현 추적 시나리오 6개를 측정했습니다.
완료 기준: 각 시나리오가 충족될 때까지 반복 조회를 수행합니다.

| 지표 | 기준 (`grep`) | cgrep (`agent locate/expand`) | 개선 |
|---|---:|---:|---:|
| 완료까지 필요한 토큰 합계 | 127,665 | 6,153 | **95.2% 감소** |
| 작업당 평균 완료 토큰 | 21,278 | 1,026 | **20.75x 축소** |
| 완료까지 평균 검색 지연 | 1,321.3 ms | 22.7 ms | **약 58.2x 향상** |

자세한 방법/결과: [Agent Token Efficiency 벤치마크](../benchmarks/pytorch-agent-token-efficiency.md)

## 문서 시작점

| 문서 | 설명 |
|---|---|
| [설치](./installation.md) | 설치와 첫 실행 |
| [사용법](./usage.md) | CLI 명령과 검색 옵션 |
| [에이전트 워크플로](./agent.md) | 2단계 `locate` / `expand` 흐름 |
| [MCP](./mcp.md) | MCP 서버 모드와 harness 사용법 |
| [인덱싱과 감시](./indexing-watch.md) | 인덱싱, watch, daemon 운용 |
| [설정](./configuration.md) | `.cgreprc.toml` 설정과 우선순위 |
| [임베딩](./embeddings.md) | semantic/hybrid 모드 설정과 튜닝 |
| [에이전트 토큰 효율 벤치마크(영문)](../benchmarks/pytorch-agent-token-efficiency.md) | PyTorch 기준 토큰 절감 효과 측정 |
| [문제 해결](./troubleshooting.md) | 자주 발생하는 문제와 해결 |
| [개발](./development.md) | 빌드, 테스트, 검증 명령 |

## 빠른 링크

- 변경 이력: [CHANGELOG.md](https://github.com/meghendra6/cgrep/blob/main/CHANGELOG.md)
- 비교 문서: [COMPARISON.md](https://github.com/meghendra6/cgrep/blob/main/COMPARISON.md)
- Harness 배경: <https://blog.can.ac/2026/02/12/the-harness-problem/>
