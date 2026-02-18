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
- `agent install claude-code`는 `claude-code` host MCP 설정도 함께 적용합니다.
- `agent install codex`는 `~/.codex/config.toml`의 `[mcp_servers.cgrep]`를 자동 보정합니다.
- `agent install copilot`는 `vscode` host MCP 설정(`.vscode/mcp.json`)도 함께 적용합니다.
- `agent install cursor`는 `.cursor/rules/cgrep.mdc` 생성 + `cursor` host MCP 설정을 함께 적용합니다.
- `agent install opencode`는 OpenCode tool 파일만 생성합니다.

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
