# 인덱싱과 Daemon

## 인덱싱

```bash
# 인덱스 재생성
cgrep index --force

# 인덱싱에서 경로 제외
cgrep index -e vendor/ -e dist/

# ignore 규칙은 유지하고 특정 ignore 경로만 포함
cgrep index -e graph_mode -e eager_mode/torch-rbln/third_party/pytorch --include-path .venv --high-memory

# ignore 경로 전체 포함(기본 동작 비활성화)
cgrep index --include-ignored

# 임베딩 모드
cgrep index --embeddings auto
cgrep index --embeddings precompute
# semantic/hybrid 검색 모드는 experimental이며 embeddings 인덱스가 필요합니다

# 매니페스트 제어(증분 경로)
cgrep index --print-diff
cgrep index --manifest-only --print-diff
cgrep index --no-manifest

# 백그라운드 전체 인덱스 빌드
cgrep index --background

# 로컬 호환 스냅샷 재사용
cgrep index --reuse strict
cgrep index --reuse auto
cgrep index --reuse off
```

## Daemon

```bash
# daemon 모드 (백그라운드 인덱싱 관리)
cgrep daemon start
cgrep daemon status
cgrep daemon stop

# 단축 별칭 형태
cgrep bg up
cgrep bg st
cgrep bg down
```

## `index --background`와 `daemon` 선택 기준

- `cgrep index --background`:
  - 1회성 비동기 빌드
  - 빌드가 끝나면 worker가 종료됨
  - 파일 변경 감시를 계속 유지하지 않음
- `cgrep daemon start`:
  - 장기 실행되는 관리형 프로세스
  - 파일 변경을 계속 추적하며 증분 재인덱싱 수행
  - 작업 종료 시 `cgrep daemon stop`으로 명시적으로 중지

대형 저장소 저부하 예시:

```bash
cgrep daemon start --debounce 30 --min-interval 180 --max-batch-delay 240
```

적응형 모드 비활성화(고정 타이밍):

```bash
cgrep daemon start --no-adaptive
```

## 시나리오별 권장 흐름

| 시나리오 | 권장 명령 | 이유 |
|---|---|---|
| 가끔 검색/읽기 | `cgrep search ...` (사전 작업 없음) | auto-bootstrap + 호출 시 refresh로 일반 사용을 처리 |
| 코딩 세션 진행 중(사용자/에이전트) | `cgrep daemon start` → `cgrep daemon stop` | 파일 변경이 계속되는 동안 인덱스를 hot 상태로 유지 |
| CI/1회성 사전 빌드 | `cgrep index --background` | daemon 상주 없이 비동기 인덱스 빌드 |
| semantic/hybrid 실험 | `cgrep index --embeddings precompute` | experimental semantic/hybrid 모드를 위한 embeddings 준비 |

## 동작 참고

- 인덱스는 `.cgrep/` 아래에 저장
- 매니페스트는 `.cgrep/manifest/` 아래에 저장 (`version`, `v1.json`, 선택적 `root.hash`)
- 하위 디렉터리에서 검색해도 가장 가까운 상위 인덱스를 재사용
- 기본적으로 search/symbol 계열 명령은 호출 시점 auto-bootstrap + call-driven refresh를 수행하므로, daemon은 항상 뜨거운 인덱스가 필요할 때만 선택적으로 사용하면 됩니다.
- 인덱싱은 기본적으로 `.gitignore`/`.ignore`를 존중 (`--include-ignored`로 전체 포함 가능)
- `--include-path <path>`로 ignore 경로 중 일부만 선택적으로 인덱싱 가능
- `--manifest-only`는 문서 재인덱싱 없이 `.cgrep/metadata.json`의 매니페스트/요약만 갱신
- `--no-manifest`는 매니페스트 경로를 비활성화하고 기존 증분 동작으로 폴백
- 재사용 캐시 루트:
  - macOS/Linux: `~/.cache/cgrep/indexes/`
  - Windows: `%LOCALAPPDATA%/cgrep/indexes/`
- 재사용 스냅샷 레이아웃:
  - `repo_key/snapshot_key/tantivy/`
  - `repo_key/snapshot_key/symbols/`
  - `repo_key/snapshot_key/manifest/`
  - `repo_key/snapshot_key/metadata.json`
- 재사용 안전성:
  - 재사용 활성 중 stale/nonexistent 파일은 결과에서 필터링
  - 비호환/손상 스냅샷은 일반 인덱싱으로 폴백
- `status`는 `.cgrep/status.json`에서 결정적 준비/진행 필드를 읽어 표시
- daemon은 기본적으로 적응형 backoff 사용 (`--no-adaptive`로 비활성화)
- daemon 기본값은 백그라운드 운용 기준으로 조정 (`--min-interval 180`, 약 3분)
- daemon은 인덱싱 가능한 확장자만 반응하고 temp/swap 파일은 건너뜀
- daemon 재인덱싱 트리거:
  - 인덱싱 가능한 파일의 생성/수정/삭제/이름 변경
  - 사용자/에디터/AI 코딩 에이전트가 만든 코드 변경
  - 브랜치 전환으로 워킹트리에 반영되는 추적 파일 변화
- daemon은 `.cgrep/metadata.json`의 최근 인덱스 프로필을 재사용
- 재사용 프로필은 최근 `cgrep index` 실행 옵션을 그대로 보존
- daemon 재인덱싱은 변경 경로만 증분 처리(갱신/삭제)
- 대규모 변경 배치(예: 큰 브랜치 전환)에서는 daemon이 자동으로 bulk 증분 갱신 경로로 전환해 이벤트/메모리 오버헤드를 낮춥니다.
- bulk 전환 임계값은 인덱스 파일 수의 약 25%를 기준으로 자동 산정되며, `1500..12000` 범위로 클램프됩니다.
- daemon은 주기적인 전체 재인덱싱 루프를 돌리지 않으며, 새 이벤트가 없으면 idle 상태를 유지합니다.

## Daemon 기본값

| 옵션 | 기본값 | 목적 |
|---|---:|---|
| `--debounce` | `15` | 이벤트 폭주가 가라앉을 때까지 대기 |
| `--min-interval` | `180` | 재인덱싱 사이 최소 간격 |
| `--max-batch-delay` | `180` | 이벤트가 계속 들어오면 강제 실행 |
| 적응형 모드 | `on` | 변경량/재인덱싱 비용에 따라 자동 backoff |

## 관리형 daemon 수명주기

```bash
cgrep daemon start
cgrep daemon status
cgrep daemon stop
```

대형 저장소에서는 아래 조합을 권장합니다.
- `--min-interval 180` 이상
- `--debounce 30` 이상
- 적응형 모드 기본값 유지

참고:
- 운영 런북: `docs/ko/operations.md`
