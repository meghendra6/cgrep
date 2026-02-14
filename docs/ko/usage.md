# 사용법

## 명령 요약

| 명령 | 설명 |
|---|---|
| `cgrep search <query>` (`s`, `find`, `q`) | 전체 텍스트 검색 |
| `cgrep read <path>` (`rd`, `cat`, `view`) | 스마트 파일 읽기 (작은 파일은 전체, 큰 파일은 개요) |
| `cgrep map` (`mp`, `tree`) | 코드베이스 구조 맵 (파일 + 심볼 스켈레톤) |
| `cgrep symbols <name>` (`sym`, `sy`) | 심볼 검색 |
| `cgrep definition <name>` (`def`, `d`) | 정의 위치 조회 |
| `cgrep callers <function>` (`calls`, `c`) | 호출자 조회 |
| `cgrep references <name>` (`refs`, `r`) | 참조 조회 |
| `cgrep dependents <file>` (`deps`, `dep`) | 역의존 파일 조회 |
| `cgrep index` (`ix`, `i`) | 인덱스 생성/재생성 |
| `cgrep watch` (`wt`, `w`) | 파일 변경 감시 후 재인덱싱 |
| `cgrep daemon <start|status|stop>` (`bg`) | 백그라운드 watch daemon 관리 |
| `cgrep mcp <serve|install|uninstall>` | MCP 서버 및 host 설정 연동 |
| `cgrep agent <...>` (`a`) | 에이전트 locate/expand + 연동 설치 |
| `cgrep completions <shell>` | 셸 자동완성 생성 |

## 단축 위주 사용 흐름

```bash
cgrep i                           # index
cgrep s "authentication flow"     # search
cgrep d handle_auth               # definition
cgrep r UserService               # references
cgrep c validate_token            # callers
cgrep dep src/auth.rs             # dependents
cgrep a l "token validation" -B tight -u
```

## 빠른 시작 (사람)

```bash
# 1) 인덱스 생성
cgrep index

# 2) 기본 검색
cgrep search "authentication flow"

# 3) 언어/경로 제한
cgrep search "token refresh" -t rust -p src/

# 4) 변경 파일만 검색
cgrep search "retry logic" -u

# 5) 심볼/탐색 명령
cgrep symbols UserService -T class
cgrep d handle_auth
cgrep c validate_token -M auto
cgrep r UserService -M auto

# 6) 의존성 조회
cgrep dep src/auth.rs

# 7) 스마트 파일 읽기 / 맵
cgrep rd src/auth.rs
cgrep rd README.md -s "## Configuration"
cgrep mp -d 2
```

## 검색 가이드

핵심 옵션:

```bash
cgrep search "<query>" \
  -p <path> \
  -m <limit> \
  -C <context> \
  -t <language> \
  --glob <pattern> \
  -x, --exclude <pattern> \
  -u, --changed [REV] \
  -M, --mode keyword|semantic|hybrid \
  -B, --budget tight|balanced|full|off \
  -P, --profile human|agent|fast
```

예시:

```bash
cgrep search "jwt decode" -m 10
cgrep s "retry backoff" -u
cgrep s "controller middleware" -B tight -P agent
```

### 모드

```bash
cgrep search "token refresh" --mode keyword   # 기본값
cgrep search "token refresh" --mode semantic  # embeddings + index 필요
cgrep search "token refresh" --mode hybrid    # embeddings + index 필요
```

모드 참고:
- `keyword`는 인덱스가 있으면 인덱스를 사용하고, 없으면 scan으로 폴백
- `semantic/hybrid`는 인덱스가 반드시 필요하며 scan 폴백 없음

하위 호환 별칭(권장하지 않음):
- `--keyword`, `--semantic`, `--hybrid` (대신 `--mode` 사용)

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
| `human` | 사람이 읽기 좋은 출력 |
| `agent` | 구조화 + 토큰 효율 기본값 |
| `fast` | 빠른 탐색 |

### 고급 옵션

```bash
cgrep search --help-advanced
```

자주 쓰는 고급 플래그:
- `--no-index`, `--fuzzy`
- `--agent-cache`, `--cache-ttl`
- `--context-pack`
- `--max-chars-per-snippet`, `--max-context-chars`, `--max-total-chars`
- `--path-alias`, `--dedupe-context`, `--suppress-boilerplate`

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

## 지원 언어

AST 심볼 추출:
- typescript, tsx, javascript, python, rust, go, c, cpp, java, ruby

인덱스/스캔 확장자:
- rs, ts, tsx, js, jsx, py, go, java, c, cpp, h, hpp, cs, rb, php, swift
- kt, kts, scala, lua, md, txt, json, yaml, toml
