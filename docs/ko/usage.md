# 사용법

## 핵심 명령

| 명령 | 용도 |
|---|---|
| `cgrep s "query" [path]` | 텍스트/코드 검색 |
| `cgrep d <symbol>` | 정의 위치 조회 |
| `cgrep r <symbol>` | 참조 조회 |
| `cgrep c <function>` | 호출자 조회 |
| `cgrep symbols <name>` | 심볼 검색 |
| `cgrep read <file>` | 파일 스마트 읽기 |
| `cgrep map --depth 2` | 코드베이스 구조 맵 |
| `cgrep dep <file>` | 역의존 파일 조회 |
| `cgrep status` | 인덱스 + daemon 상태 확인 |

## 일상 작업 흐름

```bash
# 1) 후보 파일 찾기
cgrep s "authentication middleware" src/

# 2) 구현으로 바로 이동
cgrep d handle_auth

# 3) 영향 범위 확인
cgrep r handle_auth
cgrep c handle_auth

# 4) 필요한 문맥만 읽기
cgrep read src/auth.rs
```

## 검색 범위 줄이기 (가장 중요)

```bash
# 경로 제한
cgrep s "DispatchKeySet" -p c10/core

# 파일 타입 제한
cgrep s "token refresh" -t rust

# 변경 파일만 검색 (기본: HEAD 기준)
cgrep s "retry" -u

# 문맥 라인 추가
cgrep s "evaluate_function" -C 2

# 결과 개수 제한
cgrep s "TensorIterator" -m 10
```

## 에이전트 친화 출력

```bash
# 결정적 compact payload
cgrep --format json2 --compact s "PythonArgParser" -p torch/csrc/utils

# 점수 분해(keyword 모드)
cgrep --format json2 --compact s "target_fn" --explain
```

## 인덱싱 동작 (간단 정리)

- `search/read/definition/...` 실행 시 인덱스가 없으면 자동 bootstrap 됩니다.
- 필요하면 `cgrep index`로 미리 인덱스를 만들 수 있습니다.
- 긴 세션에서 인덱스를 계속 최신으로 유지하려면 `cgrep daemon start`를 사용하세요.

## 주의 사항

- 빈 쿼리는 거부됩니다.
- 쿼리가 `-`로 시작하면 `--`를 사용하세요.

```bash
cgrep s -- --help
```

- `semantic`, `hybrid`는 experimental이며 embeddings 인덱스가 필요합니다.

## 다음 문서

- 에이전트 워크플로: [agent.md](./agent.md)
- MCP 연동: [mcp.md](./mcp.md)
- 인덱싱/daemon: [indexing-watch.md](./indexing-watch.md)
- 문제 해결: [troubleshooting.md](./troubleshooting.md)
