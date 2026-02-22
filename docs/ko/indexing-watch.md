# 인덱싱과 Daemon

## 어떤 모드를 쓰면 되나요?

| 상황 | 권장 방식 |
|---|---|
| 가끔 검색/조회 | `search/read/definition`을 바로 실행 (auto bootstrap) |
| 코딩 세션 진행 중 | `cgrep daemon start` 후 종료 시 `cgrep daemon stop` |
| 1회성 비동기 사전 빌드 | `cgrep index --background` |
| semantic/hybrid 실험 | `cgrep index --embeddings precompute` (experimental) |

## 핵심 명령

```bash
# 인덱스 생성/갱신
cgrep index

# 강제 전체 재생성
cgrep index --force

# 1회성 백그라운드 빌드
cgrep index --background

# daemon 생명주기
cgrep daemon start
cgrep daemon status
cgrep daemon stop

# 준비 상태 확인
cgrep status
```

## 대형 저장소 팁

```bash
# 변경량이 큰 저장소에서 부하 낮추기
cgrep daemon start --debounce 30 --min-interval 180 --max-batch-delay 240
```

기본값도 백그라운드 운용에 맞게 튜닝되어 있으므로, 필요할 때만 조정하면 됩니다.

## 참고

- 인덱스 파일은 `.cgrep/`에 저장됩니다.
- 기본적으로 `.gitignore`, `.ignore`를 존중합니다.
- `--include-ignored`는 ignore 필터를 비활성화합니다.
- `--include-path <path>`로 일부 ignore 경로만 선택적으로 포함할 수 있습니다.
- daemon은 이벤트 기반으로 동작하며, 변경이 없으면 idle 상태를 유지합니다.
