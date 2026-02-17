# MCP

## MCP 서버 실행

```bash
cgrep mcp serve
cgrep mcp run
```

## Host 설정 설치

```bash
cgrep mcp install claude-code
cgrep mcp add claude-code
cgrep mcp install cursor
cgrep mcp install windsurf
cgrep mcp install vscode
cgrep mcp install claude-desktop
```

`cgrep mcp install <host>`는 가능하면 cgrep 실행 파일의 절대경로를 `command`로 기록해
에디터 GUI 환경의 PATH 차이로 인한 실행 실패를 줄입니다.

## Host 설정 제거

```bash
cgrep mcp uninstall claude-code
cgrep mcp rm claude-code
```

## Harness 가이드

MCP 모드는 안정적인 tool-calling을 위해 harness 원칙을 따릅니다.
- ad-hoc grep 반복 대신 구조화된 체인(`search -> read -> symbol navigation`) 사용
- 재시도/불안정을 줄이기 위해 결정적 출력 유지(`json/json2` + `--compact`)
- 경로/범위를 먼저 좁혀 저토큰, 안정적 조회 유지
- 변경(write) 도구 없이 read/search 도구만 노출

참고 문서: <https://blog.can.ac/2026/02/12/the-harness-problem/>

## 노출되는 MCP 도구

- `cgrep_search`
- `cgrep_read`
- `cgrep_map`
- `cgrep_symbols`
- `cgrep_definition`
- `cgrep_references`
- `cgrep_callers`
- `cgrep_dependents`
- `cgrep_index`

## 설정 파일 대상

| Host | 경로 | 키 |
|---|---|---|
| `claude-code` | `~/.claude.json` | `mcpServers` |
| `cursor` | `~/.cursor/mcp.json` | `mcpServers` |
| `windsurf` | `~/.codeium/windsurf/mcp_config.json` | `mcpServers` |
| `vscode` | `.vscode/mcp.json` | `servers` |
| `claude-desktop` | OS별 desktop 설정 경로 | `mcpServers` |

참고:
- `claude-desktop` 자동 경로는 현재 macOS/Windows만 구현되어 있습니다.
