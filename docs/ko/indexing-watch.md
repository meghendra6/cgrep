# 인덱싱과 Watch

## 인덱싱

```bash
# 인덱스 재생성
cgrep index --force

# 인덱싱에서 경로 제외
cgrep index -e vendor/ -e dist/

# 임베딩 모드
cgrep index --embeddings auto
cgrep index --embeddings precompute
```

## Watch와 daemon

```bash
# 포그라운드 watch
cgrep watch

# daemon 모드 (백그라운드 watch 관리)
cgrep daemon start
cgrep daemon status
cgrep daemon stop
```

대형 저장소 저부하 예시:

```bash
cgrep watch --debounce 30 --min-interval 180 --max-batch-delay 240
```

적응형 모드 비활성화(고정 타이밍):

```bash
cgrep watch --no-adaptive
```

## 동작 참고

- 인덱스는 `.cgrep/` 아래에 저장
- 하위 디렉터리에서 검색해도 가장 가까운 상위 인덱스를 재사용
- 인덱싱은 `.gitignore`를 무시, scan 모드는 `.gitignore`를 존중
- watch는 기본적으로 적응형 backoff 사용 (`--no-adaptive`로 비활성화)
- watch 기본값은 백그라운드 운용 기준으로 조정 (`--min-interval 180`, 약 3분)
- watch는 인덱싱 가능한 확장자만 반응하고 temp/swap 파일은 건너뜀
- watch는 초기/증분 인덱싱 모두에서 `[index].exclude_paths`를 존중
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
