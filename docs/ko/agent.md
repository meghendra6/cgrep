# 에이전트 워크플로

cgrep으로 AI 코딩 에이전트의 탐색 루프를 짧고 일관되게 유지할 수 있습니다.

## 1) 사용 중인 호스트에 설치

```bash
cgrep agent install codex
cgrep agent install claude-code
cgrep agent install cursor
cgrep agent install copilot
cgrep agent install opencode
```

## 2) 필수 작업과 선택 작업

- 필수: 설치 후 현재 에이전트 세션을 한 번 재시작
- 일반 사용에서 불필요: 수동 `cgrep index`, 상시 daemon 실행
- 선택: 변경이 매우 잦은 장시간 세션에서 `cgrep daemon start`

## 3) 선택: 저토큰 2단계 조회 (CLI)

```bash
# 1단계: 후보 찾기
ID=$(cgrep agent locate "where token validation happens" --compact | jq -r '.results[0].id')

# 2단계: 선택 후보 확장
cgrep agent expand --id "$ID" -C 8 --compact
```

## 4) 선택: 결정적 조회 계획 (CLI)

```bash
cgrep --format json2 --compact agent plan "trace authentication middleware flow"
```

유용한 옵션:
- `--max-steps <n>`
- `--max-candidates <n>`
- `--budget tight|balanced|full|off`
- `--path`, `--changed`

## 권장 정책

- 기본 흐름: `map -> search -> read -> definition/references/callers`
- `-p`, `--glob`, `--changed`로 범위를 먼저 줄이기
- 에이전트 파싱에는 `--format json2 --compact` 사용

## 제거

```bash
cgrep agent uninstall codex
cgrep agent uninstall claude-code
cgrep agent uninstall cursor
cgrep agent uninstall copilot
cgrep agent uninstall opencode
```

## 1분 검증

```bash
codex mcp list
cgrep --format json2 --compact s "DispatchKeySet" -p c10/core
```
