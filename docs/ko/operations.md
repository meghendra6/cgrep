# 운영 가이드

## 범위

이 문서는 운영 단계 점검 항목(준비 상태, 백그라운드 인덱싱, 재사용 진단, 안전 정리)을 다룹니다.

## `.cgrep/` 런타임 아티팩트

- `.cgrep/index/`: Tantivy 검색 인덱스 파일.
- `.cgrep/symbols/`: 추출된 심볼 아티팩트.
- `.cgrep/manifest/`: 매니페스트 메타데이터(`version`, `v1.json`, 선택적 `root.hash`).
- `.cgrep/metadata.json`: 인덱스 프로필/증분 메타데이터.
- `.cgrep/status.json`: basic/full 준비 상태와 백그라운드 진행률.
- `.cgrep/reuse-state.json`: 마지막 재사용 판단/폴백 사유.
- `.cgrep/watch.pid`, `.cgrep/watch.log`: daemon PID/로그.
- `.cgrep/background-index.log`: 백그라운드 인덱스 워커 로그.

## 준비 상태, status, 검색 통계

```bash
# 사람 친화 출력
cgrep status

# 결정적 machine payload
cgrep --format json2 --compact status

# 검색 요청 통계(meta) 확인
cgrep --format json2 --compact search "sanity check" -m 5
```

`status` 주요 필드:
- 준비 상태: `basic_ready`, `full_ready`
- 빌드 단계/카운터: `phase`, `progress.total|processed|failed`
- daemon 상태: `running|stale`, `pid`, `pid_file`, `log_file`
- 재사용 진단(존재 시): `decision`, `source`, `snapshot_key`, `reason`

검색 `json2.meta` 통계:
- `elapsed_ms`
- `files_with_matches`
- `total_matches`
- 페이로드 카운터(`payload_chars`, `payload_tokens_estimate`)

## 백그라운드 인덱싱 수명주기

```bash
# 비동기 전체 인덱스 빌드 시작
cgrep index --background

# 상태 모니터링
cgrep --format json2 --compact status

# 관리형 인덱싱 daemon
cgrep daemon start
cgrep daemon status
cgrep daemon stop
```

명령 역할 구분:
- `cgrep index --background`는 1회성 비동기 빌드 명령입니다(지속 감시 없음).
- `cgrep daemon start`는 `cgrep daemon stop`까지 파일 변경 추적 + 증분 재인덱싱을 계속 수행합니다.
- 대규모 변경 이벤트 폭주 시 daemon은 업데이트를 묶고 자동으로 bulk 증분 갱신 경로로 전환할 수 있습니다.
- bulk 전환 임계값은 인덱스 파일 수의 약 25%를 기준으로 자동 산정되며, `1500..12000` 범위로 클램프됩니다.

동작 보장:
- 백그라운드 모드는 `--background`에서만 활성화(기본 동작 유지).
- status 업데이트는 atomic하며 중단 복구 안전성을 가집니다.
- stale pid 상태는 status 점검 시 복구됩니다.

## 재사용 진단 및 마이그레이션 노트

추가된 인덱싱 플래그:
- `cgrep index --background`
- `cgrep index --reuse off|strict|auto` (기본값 `off`)
- `cgrep index --manifest-only`
- `cgrep index --print-diff`
- `cgrep index --no-manifest`

마이그레이션 가이드:
- 기존 스크립트는 기본값(`--reuse off`)으로 기존 동작을 유지합니다.
- 자동화에서 기존 동작 고정을 원하면 `--reuse off`를 명시하세요.
- 재사용을 켜면 `.cgrep/reuse-state.json`이 생성/갱신됩니다(선택적 아티팩트로 취급).
- `status`의 `reuse` 필드는 선택 필드이며, 재사용 미시도 시 없을 수 있습니다.

## 안전 정리(수동만)

파괴적 정리는 자동으로 실행되지 않습니다.

필요 시 수동으로만 수행하세요:

```bash
# 프로젝트 로컬 인덱스 아티팩트만 제거
rm -rf .cgrep/index .cgrep/symbols .cgrep/manifest

# 재사용 진단 상태만 제거
rm -f .cgrep/reuse-state.json
```

공용 재사용 캐시(`~/.cache/cgrep/indexes/` 또는 Windows `%LOCALAPPDATA%/cgrep/indexes/`)는 다른 worktree 영향 여부를 확인한 뒤 수동 삭제하세요.

## Doctor 흐름

저장소 건강 상태 점검:

```bash
bash scripts/doctor.sh .
```

해당 점검은 비파괴적이며, 통합 파일/설정 누락 여부를 보고합니다.
