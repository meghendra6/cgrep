# 에이전트 워크플로

## 핵심 정책

- 로컬 코드 탐색은 cgrep을 우선 사용합니다.
- 기본 흐름은 `map -> search -> read -> definition/references/callers`입니다.
- `-p`, `--glob`, `--changed`로 범위를 먼저 줄입니다.
- 에이전트 출력은 `--format json2 --compact`를 기본으로 사용합니다.

## 2단계 조회 (`agent`)

`cgrep agent`는 저토큰 반복 조회에 최적화되어 있습니다.

```bash
# 1단계: locate로 후보 수집
cgrep agent locate "where token validation happens" --changed --budget balanced --compact

# 첫 번째 결과 ID 선택 예시
ID=$(cgrep agent locate "token validation" --compact | jq -r '.results[0].id')

# 2단계: 선택 ID 확장
cgrep agent expand --id "$ID" -C 8 --compact
```

## 결정적 플래닝 (`agent plan`)

`agent plan`은 범위가 제한된 `map -> agent locate -> agent expand`를 자동 구성하고, 결정적인 `json2`를 출력합니다.

```bash
cgrep --format json2 --compact agent plan "trace authentication middleware flow"
cgrep --format json2 --compact agent plan "validate_token" --max-steps 6 --max-candidates 5
```

주요 옵션:
- `--max-steps <n>`: 출력 step 수 상한 (기본 `6`)
- `--max-candidates <n>`: 최종 후보 수 상한 (기본 `5`)
- `--budget <tight|balanced|full|off>`: locate 단계 예산 프리셋 재사용
- `--profile <agent|ai|...>`: planner 메타데이터 프로필 라벨 (별칭은 built-in으로 정규화)
- `--path`, `--changed`, `--mode`: locate 전략에 전달
- map 실행 정책:
  - `--path` 사용 시: planner가 `map`을 실제 실행
  - `--path` 미사용 시: 대형 저장소 지연을 줄이기 위해 `map`을 `planned`로 유지
  - `locate/expand` 이후 상위 후보에 대해 bounded `read` 후속 step을 추가해 확인 루프를 단축

`json2` 출력 구조:
- `meta`: query/profile/budget/strategy + 저장소 fingerprint/version 정보
- `steps[]`: 안정적인 step ID, command, args, reason, expected output type, status
- `candidates[]`: 후속 탐색용 안정적인 ID + 요약
- `error`(선택): 옵션 검증 실패 시 machine-parseable 에러

결정적 정렬/동점 처리 규칙:
- step 순서는 전략 단계(`map -> locate -> expand -> navigation -> read-verification`) 순서로 고정됩니다.
- step ID는 안정적인 형식(`sNN_<slug>`)을 사용합니다.
- candidate 동점 처리 순서:
  1. score (내림차순)
  2. path (오름차순)
  3. line (오름차순)
  4. id (오름차순)
- 선택 필드(`diagnostics`, `error`)는 비어 있으면 생략됩니다.

단축 별칭 형태:

```bash
cgrep a l "where token validation happens" -u -B balanced --compact
```

## 연동 설치

지원 에이전트에 지침 설치:

```bash
cgrep agent install claude-code
cgrep agent install codex
cgrep agent install copilot
cgrep agent install cursor
cgrep agent install opencode
```

제거:

```bash
cgrep agent uninstall claude-code
cgrep agent uninstall codex
cgrep agent uninstall copilot
cgrep agent uninstall cursor
cgrep agent uninstall opencode
```

기존 `install-*`, `uninstall-*` 명령도 호환성 때문에 유지됩니다.

설치 시 MCP 자동 연동:
- `mcp install` host 값은 `claude-code`, `cursor`, `windsurf`, `vscode`, `claude-desktop`이며, Codex는 `agent install codex`를 사용합니다.
- `agent install claude-code`는 `claude-code` host MCP 설정도 함께 적용합니다.
- `agent install codex`는 `~/.codex/config.toml`의 `[mcp_servers.cgrep]`를 자동 보정하며, `command = "cgrep"`, `args = ["mcp", "serve"]`, startup timeout을 명시합니다.
- `agent install copilot`는 `vscode` host MCP 설정(`.vscode/mcp.json`)도 함께 적용합니다.
- `agent install cursor`는 `.cursor/rules/cgrep.mdc` 생성 + `cursor` host MCP 설정을 함께 적용합니다.
- `agent install opencode`는 OpenCode tool 파일만 생성합니다.

Codex 런타임 참고:
- `agent install codex` 후에는 현재 Codex 세션을 재시작해 MCP 설정을 다시 로드하세요.
- `agent uninstall codex`는 `~/.codex/AGENTS.md`의 skill 블록과 `~/.codex/config.toml`의 `[mcp_servers.cgrep]` 블록을 함께 제거합니다.

## instruction/skill 파일 생성 위치

`cgrep agent install <provider>` 실행 시 각 에이전트 형식에 맞는 파일이 생성/수정됩니다.

| Provider | 생성/수정 파일 |
|---|---|
| `claude-code` | `~/.claude/CLAUDE.md` |
| `codex` | `~/.codex/AGENTS.md` |
| `copilot` | `.github/instructions/cgrep.instructions.md` (필요 시 `.github/copilot-instructions.md`에 섹션 추가) |
| `cursor` | `.cursor/rules/cgrep.mdc` |
| `opencode` | `~/.config/opencode/tool/cgrep.ts` |

## 1분 검증

```bash
# Codex host MCP 등록 확인
codex mcp list

# MCP 서버 응답 확인
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
| cgrep mcp serve
```
