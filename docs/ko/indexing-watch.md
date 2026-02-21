# 인덱싱과 Watch

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

## Watch와 daemon

```bash
# 포그라운드 watch
cgrep watch
cgrep w

# daemon 모드 (백그라운드 watch 관리)
cgrep daemon start
cgrep daemon status
cgrep daemon stop

# 단축 별칭 형태
cgrep bg up
cgrep bg st
cgrep bg down
```

대형 저장소 저부하 예시:

```bash
cgrep watch --debounce 30 --min-interval 180 --max-batch-delay 240

# 단축 플래그 형태
cgrep w -d 30 -i 180 -b 240
```

적응형 모드 비활성화(고정 타이밍):

```bash
cgrep watch --no-adaptive
```

## 동작 참고

- 인덱스는 `.cgrep/` 아래에 저장
- 매니페스트는 `.cgrep/manifest/` 아래에 저장 (`version`, `v1.json`, 선택적 `root.hash`)
- 하위 디렉터리에서 검색해도 가장 가까운 상위 인덱스를 재사용
- 인덱싱은 기본적으로 `.gitignore`/`.ignore`를 존중 (`--include-ignored`로 전체 포함 가능)
- `--include-path <path>`로 ignore 경로 중 일부만 선택적으로 인덱싱 가능
- `--manifest-only`는 문서 재인덱싱 없이 `.cgrep/metadata.json`의 매니페스트/요약만 갱신
- `--no-manifest`는 매니페스트 경로를 비활성화하고 기존 증분 동작으로 폴백
- 재사용 캐시 루트:
  - macOS/Linux: `~/.cache/cgrep/indexes/`
  - Windows: `%LOCALAPPDATA%/cgrep/indexes/`
- 재사용 안전성:
  - 재사용 활성 중 stale/nonexistent 파일은 결과에서 필터링
  - 비호환/손상 스냅샷은 일반 인덱싱으로 폴백
- `status`는 `.cgrep/status.json`에서 결정적 준비/진행 필드를 읽어 표시
- watch는 기본적으로 적응형 backoff 사용 (`--no-adaptive`로 비활성화)
- watch 기본값은 백그라운드 운용 기준으로 조정 (`--min-interval 180`, 약 3분)
- watch는 인덱싱 가능한 확장자만 반응하고 temp/swap 파일은 건너뜀
- watch/daemon은 `.cgrep/metadata.json`의 최근 인덱스 프로필을 재사용
- 재사용 프로필은 최근 `cgrep index` 실행 옵션을 그대로 보존
- watch 재인덱싱은 변경 경로만 증분 처리(갱신/삭제)

## Watch 기본값

| 옵션 | 기본값 | 목적 |
|---|---:|---|
| `--debounce` | `15` | 이벤트 폭주가 가라앉을 때까지 대기 |
| `--min-interval` | `180` | 재인덱싱 사이 최소 간격 |
| `--max-batch-delay` | `180` | 이벤트가 계속 들어오면 강제 실행 |
| 적응형 모드 | `on` | 변경량/재인덱싱 비용에 따라 자동 backoff |

## 백그라운드 watch

```bash
# 백그라운드 실행 + 로그 기록
nohup cgrep watch > .cgrep/watch.log 2>&1 &

# 프로세스/로그 확인
pgrep -fl "cgrep watch"
tail -f .cgrep/watch.log

# 종료
pkill -f "cgrep watch"
```

대형 저장소에서는 아래 조합을 권장합니다.
- `--min-interval 180` 이상
- `--debounce 30` 이상
- 적응형 모드 기본값 유지

참고:
- 운영 런북: `docs/ko/operations.md`
