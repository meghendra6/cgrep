# 사용법

## 명령 요약

| 명령 | 설명 |
|---|---|
| `cgrep search <query> [path]` (`s`, `find`, `q`) | 전체 텍스트 검색 |
| `cgrep read <path>` (`rd`, `cat`, `view`) | 스마트 파일 읽기 (작은 파일은 전체, 큰 파일은 개요) |
| `cgrep map` (`mp`, `tree`) | 코드베이스 구조 맵 (파일 + 심볼 스켈레톤) |
| `cgrep symbols <name>` (`sym`, `sy`) | 심볼 검색 |
| `cgrep definition <name>` (`def`, `d`) | 정의 위치 조회 |
| `cgrep callers <function>` (`calls`, `c`) | 호출자 조회 |
| `cgrep references <name>` (`refs`, `r`) | 참조 조회 |
| `cgrep dependents <file>` (`deps`, `dep`) | 역의존 파일 조회 |
| `cgrep index` (`ix`, `i`) | 인덱스 생성/재생성 |
| `cgrep daemon <start|status|stop>` (`bg`) | 백그라운드 인덱싱 daemon 관리 |
| `cgrep status` (`st`) | basic/full 준비 상태 + 백그라운드 인덱스 상태 조회 |
| `cgrep mcp <serve|install|uninstall>` | MCP 서버 및 host 설정 연동 |
| `cgrep agent <...>` (`a`) | 에이전트 plan/locate/expand + 연동 설치 |
| `cgrep completions <shell>` | 셸 자동완성 생성 |

## grep/rg 전환 빠른 경로

```bash
# grep -R "token validation" src/
cgrep search "token validation" src/

# grep/rg + 수동 파일 열기 반복
cgrep d handle_auth
cgrep r UserService
cgrep rd src/auth.rs
cgrep mp -d 2
```

- 텍스트 검색은 `cgrep search`(또는 `cgrep s`)를 사용하세요.
- 옵션을 먼저 두는 형태도 그대로 지원합니다: `cgrep search -r --include '**/*.rs' needle src/`.
- 쿼리가 `-`로 시작하면 `search` 뒤에 `--`를 사용하세요 (예: `cgrep search -- --literal`).
- `-`로 시작하는 쿼리와 범위 옵션을 함께 쓸 때는 옵션/경로를 `--` 앞에 두세요
  (예: `cgrep search -p src -- --help`).
- grep 스타일 범위 옵션을 지원합니다: `-r/--recursive`, `--no-recursive`, `--include`, `--exclude-dir`.
- `--no-ignore`는 scan 모드를 강제하고 scan 시 `.gitignore`/`.ignore` 필터를 비활성화합니다.
- 명시적 플래그가 필요하면 기존처럼 `-p <path>`를 사용하면 됩니다.
- 빈/공백 쿼리는 모든 검색 모드에서 동일하게 거부됩니다 (`--regex` 포함).
- `cgrep read`에 빈 경로를 넘기면 에러를 반환합니다 (`Error: Path cannot be empty`).
- `search` 결과 `path`는 항상 round-trip 가능하도록 반환됩니다:
  워크스페이스 내부 스코프는 상대경로, 외부 스코프는 절대경로를 사용합니다.
- 기본적으로 `search`/`symbols`/`definition`/`references`/`callers`/`dependents`와 `agent locate|plan`은 호출 시점 auto-bootstrap + call-driven refresh를 수행하므로, 일반 사용에서 수동 `cgrep index`는 선택 사항입니다.
- CLI auto-index 변경 검사는 연속 명령 루프에서 반복 스캔 오버헤드를 줄이기 위해 debounce됩니다.
- `search --no-index`를 사용하면 항상 scan-only로 동작합니다.
- C/C++ 심볼 내비게이션에서는 `.h`를 C++ 헤더로 처리하며, `definition`에서 `TORCH_API Foo` 같은 매크로 포함 타입 선언을 더 안정적으로 찾습니다.

검증 예시:

```bash
cgrep search ""            # Error: Search query cannot be empty
cgrep search --regex ""    # Error: Search query cannot be empty
cgrep read ""              # Error: Path cannot be empty
```

## 단축 위주 사용 흐름

```bash
cgrep i                           # index
cgrep s "authentication flow"     # search
cgrep d handle_auth               # definition
cgrep r UserService               # references
cgrep c validate_token            # callers
cgrep dep src/auth.rs             # dependents
cgrep a p "trace auth middleware flow"
cgrep a l "token validation" -B tight -u
```

## 빠른 시작 (사람)

```bash
# 1) 선택: 워밍업 인덱스 생성(필수 아님)
cgrep index

# 2) 핵심 5개 명령
cgrep search "authentication flow" src/
cgrep d handle_auth
cgrep r UserService
cgrep rd src/auth.rs
cgrep mp -d 2

# 3) 선택: 언어/경로/변경파일 제한
cgrep search "token refresh" -t rust -p src/
cgrep search "retry logic" -u
```

## 인덱스 플래그

```bash
# 기본 증분 인덱스 경로(매니페스트 사용)
cgrep index

# 매니페스트 기준 추가/수정/삭제 diff 출력
cgrep index --print-diff

# 문서 재인덱싱 없이 매니페스트 메타데이터만 갱신
cgrep index --manifest-only --print-diff

# 매니페스트 경로 비활성화(legacy 증분 경로 사용)
cgrep index --no-manifest

# 전체 인덱스를 백그라운드에서 빌드하고 즉시 반환
cgrep index --background

# 로컬 호환 캐시 스냅샷 재사용(HEAD 정확 일치)
cgrep index --reuse strict

# 로컬 근사 스냅샷 재사용
cgrep index --reuse auto

# 재사용 비활성화(기본값)
cgrep index --reuse off
```

## Status 가이드

```bash
# 사람 친화 출력
cgrep status

# 자동화/에이전트용 구조화 출력
cgrep --format json2 --compact status
```

`status`에는 재사용 시도 시 `reuse` 정보가 선택적으로 포함됩니다:
- `mode`, `decision`, `active`
- `source`, `snapshot_key`, `repo_key`, `reason` (가능한 경우)

## 검색 가이드

`search`(또는 별칭 `s`)를 명시적으로 사용하세요.

핵심 옵션:

```bash
cgrep search "<query>" \
  -p <path> \
  -r | --no-recursive \
  -m <limit> \
  -C <context> \
  -i | --ignore-case \
  --case-sensitive \
  -t <language> \
  --glob|--include <pattern> \
  -x, --exclude|--exclude-dir <pattern> \
  --no-ignore \
  -u, --changed [REV] \
  -M, --mode keyword|semantic|hybrid \
  --explain \
  -B, --budget tight|balanced|full|off \
  -P, --profile human|agent|fast  # 별칭: user/ai/quick
```

예시:

```bash
cgrep search "jwt decode" -m 10
cgrep s "retry backoff" -u
cgrep search -r --no-ignore "token validation" src/
cgrep s "controller middleware" -B tight -P agent
```

### 모드

```bash
cgrep search "token refresh" --mode keyword   # 기본값
cgrep search "token refresh" --mode semantic  # experimental, embeddings + index 필요
cgrep search "token refresh" --mode hybrid    # experimental, embeddings + index 필요
```

모드 참고:
- `keyword`는 인덱스가 있으면 인덱스를 사용하고, 없으면 scan으로 폴백
- `semantic/hybrid`는 **experimental**이며 인덱스가 반드시 필요하고 scan 폴백이 없음

하위 호환 별칭(권장하지 않음):
- `--keyword`, `--semantic`, `--hybrid` (대신 `--mode` 사용)

### Keyword 랭킹 + explain

```bash
# keyword 모드 상위 결과 점수 분해(결정적 출력)
cgrep --format json2 --compact search "target_fn" --explain
```

랭킹 참고:
- 다중 신호 랭킹은 `[ranking] enabled = true`일 때만 적용됩니다.
- 기본값(`enabled = false`)은 기존 keyword 정렬을 유지합니다.
- 쿼리 분류는 결정적 규칙을 사용합니다:
  - `identifier-like`: 공백 없는 단일 토큰이며 `[A-Za-z0-9_:. $]` 문자만 포함
  - `phrase-like`: 그 외(공백 포함)
- 동점 처리 순서:
  1. 최종 점수 (내림차순)
  2. path (오름차순)
  3. line (오름차순)
  4. snippet (오름차순)

`--explain`은 상위 K개 결과(`ranking.explain_top_k`, 기본 `5`)에 대해 다음 항목을 출력합니다:
- `bm25`
- `path_boost` (언어 필터 매치 가중치 포함)
- `symbol_boost`
- `changed_boost`
- `kind_boost`
- `penalties`
- `final_score`

### Budget 프리셋

| 프리셋 | 목적 |
|---|---|
| `tight` | 최소 페이로드 / 엄격한 토큰 제어 |
| `balanced` | 기본 에이전트 균형값 |
| `full` | 더 많은 컨텍스트, 더 큰 페이로드 |
| `off` | 프리셋 제한 비활성화 |

### 프로필

| 프로필 | 사용 목적 |
|---|---|
| `human` (`user`, `developer`, `dev`) | 사람이 읽기 좋은 출력 |
| `agent` (`ai`, `ai-agent`, `coding-agent`) | 구조화 + 토큰 효율 기본값 |
| `fast` (`quick`) | 빠른 탐색 |

시나리오별 빠른 레시피:

```bash
# 사용자 중심 인터랙티브 워크플로
cgrep s "auth refresh token" -P user -C 2

# AI 코딩 에이전트 워크플로 (결정적 + compact)
cgrep s "auth refresh token" -P ai -B tight --format json2 --compact
```

### 고급 옵션

```bash
cgrep search --help-advanced
```

자주 쓰는 고급 플래그:
- `--no-index`, `--fuzzy`
- `--explain`
- `--agent-cache`, `--cache-ttl`
- `--context-pack`
- `--max-chars-per-snippet`, `--max-context-chars`, `--max-total-chars`
- `--path-alias`, `--dedupe-context`, `--suppress-boilerplate`

## Read 가이드

```bash
cgrep read src/auth.rs
cgrep read src/auth.rs --section 120-220
cgrep read docs/ko/usage.md --section "검색 가이드"
cgrep read src/auth.rs --full
```

참고:
- `read`는 비어 있지 않은 파일 경로가 필요합니다.
- `--section`은 라인 범위(`start-end`) 또는 마크다운 헤딩을 받을 수 있습니다.
- `--full`을 주면 스마트 개요 모드를 끄고 파일 전체를 출력합니다.

## Map 가이드

```bash
cgrep map
cgrep map -d 2
cgrep map -p src -d 3
```

참고:
- 기본 깊이는 `3`입니다.
- 후속 `search/read` 전에 `-p`로 하위 트리를 먼저 좁히세요.

## Definition 가이드 (선택 튜닝)

```bash
cgrep definition <name> \
  -p <path> \
  -m <limit>
```

참고:
- 대부분의 저장소에서는 `cgrep d <name>`만으로 충분합니다.
- `-p`는 특정 하위 경로로 의도적으로 범위를 제한할 때만 사용하세요.
- `-m`은 반환 정의 개수를 더 엄격히 제한할 때 사용하세요 (기본값: `20`).

## 출력 형식

전역 플래그:

```bash
--format text|json|json2
--compact
```

형식 요약:
- `text`: 사람이 읽기 쉬운 형식
- `json`: 단순 배열/객체 페이로드
- `json2`: 자동화/에이전트용 구조화 페이로드

## 결정적 `json2`/`--compact` 계약

정렬 규칙:
- 최상위 필드는 struct 정의 순서대로 안정적으로 출력됩니다.
- `results[]`는 결정적 정렬 규칙을 따릅니다.
- search 동점 처리 순서:
  1. score (내림차순)
  2. path (오름차순)
  3. line (오름차순)
  4. snippet (오름차순)
- agent plan 후보 동점 처리 순서:
  1. locate score (내림차순)
  2. path (오름차순)
  3. line (오름차순)
  4. id (오름차순)

필수/선택 필드 정책:
- 명령별 스키마의 필수 필드(`meta`, `results`, `steps`, `candidates`, `result`)는 항상 존재합니다.
- 선택 필드는 값이 있을 때만 출력되며, 기본적으로 `null` 대신 생략됩니다.
  예시:
  - search: `context_before`, `context_after`, `explain`
  - status: `reuse`
  - agent plan: `diagnostics`, `error`
- 파서는 필드 이름 기준으로 처리하고, 위치 기반 가정은 피하세요.

ID 안정성:
- 동일한 저장소/쿼리/옵션/상태에서는 search/agent ID가 안정적으로 유지됩니다.
- 결정성 보장은 정렬/필드 존재성 기준이며, `elapsed_ms` 같은 요청 시점 통계는 정보성 필드입니다.

## 마이그레이션 및 호환성 노트

추가된 플래그(기본 동작은 명시적으로 켜지지 않으면 유지):
- search:
  - `--explain`
- index:
  - `--background`
  - `--reuse off|strict|auto` (기본값 `off`)
  - `--manifest-only`
  - `--print-diff`
  - `--no-manifest`
- agent:
  - `agent plan`
  - `agent plan --max-steps`
  - `agent plan --max-candidates`

호환성 보장:
- 기존 별칭(`s`, `d`, `r`, `c`, `dep`, `i`, `a l`, `a x`)은 계속 동작합니다.
- deprecated 모드 별칭(`--keyword`, `--semantic`, `--hybrid`)도 계속 허용됩니다.
- `json2` 스키마는 additive 방식이며, 선택 필드 추가는 기존 필수 필드를 깨지 않습니다.

새로운 `.cgrep/` 아티팩트:
- `.cgrep/status.json`
- `.cgrep/reuse-state.json`
- `.cgrep/manifest/`, `.cgrep/metadata.json` (증분/재사용 메타데이터)

## 지원 언어

AST 심볼 추출:
- typescript, tsx, javascript, python, rust, go, c, cpp, java, ruby

인덱스/스캔 확장자:
- rs, ts, tsx, js, jsx, py, go, java, c, cpp, h, hpp, cs, rb, php, swift
- kt, kts, scala, lua, md, txt, json, yaml, toml
