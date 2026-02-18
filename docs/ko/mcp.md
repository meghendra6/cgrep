# MCP

## 1분 설정

```bash
cgrep mcp install codex
cgrep mcp install claude-code
cgrep mcp install cursor
```

진단용 수동 서버 실행:

```bash
cgrep mcp serve
```

별칭:

```bash
cgrep mcp run
```

## 지원 Host

```bash
cgrep mcp install claude-code
cgrep mcp install cursor
cgrep mcp install windsurf
cgrep mcp install vscode
cgrep mcp install claude-desktop
```

별칭:

```bash
cgrep mcp add <host>
```

설정 제거:

```bash
cgrep mcp uninstall <host>
cgrep mcp rm <host>
```

## 검증

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
| cgrep mcp serve
```

## 동작 참고

- `cgrep mcp install <host>`는 기본적으로 `command = "cgrep"`를 기록합니다.
  따라서 MCP 설정을 다시 설치하지 않아도 바이너리 업데이트가 바로 반영됩니다.
- 고정 경로가 필요하면 설치 전에 `CGREP_MCP_COMMAND` 환경변수를 지정하세요.
- `claude-desktop` 자동 경로는 현재 macOS/Windows에서 구현되어 있습니다.
- MCP tool 호출에는 내부 타임아웃이 적용됩니다. 제한을 넘기면 host 전체 타임아웃까지 대기하지 않고 명시적 에러를 반환합니다.

## 노출 MCP 도구

- `cgrep_search`
- `cgrep_read`
- `cgrep_map`
- `cgrep_symbols`
- `cgrep_definition`
- `cgrep_references`
- `cgrep_callers`
- `cgrep_dependents`
- `cgrep_index`

## 도구 인자 참고

- MCP 도구는 optional `cwd`를 받아 상대경로 해석 기준을 고정할 수 있습니다.
- `cgrep_search`는 `-n`, `--help`처럼 `-`로 시작하는 쿼리를 리터럴 검색어로 처리합니다.
- `cgrep_search`는 빈/공백 쿼리를 일관되게 거부합니다 (`regex=true` 포함).
- `cgrep_search` 결과 `path`는 재사용 가능하도록 유지됩니다:
  워크스페이스 내부 스코프는 상대경로, 외부 스코프는 절대경로를 반환합니다.
- `cgrep_read`는 빈 `path` 인자를 거부합니다 (`Error: Path cannot be empty`).
- MCP 서버 cwd가 `/`일 때 상대경로 스코프를 쓰면 `cwd`(또는 절대 `path`)가 필요합니다. 실수로 시스템 루트를 스캔하는 것을 막기 위함입니다.

## 설정 파일 대상

| Host | 경로 | 키 |
|---|---|---|
| `claude-code` | `~/.claude.json` | `mcpServers` |
| `cursor` | `~/.cursor/mcp.json` | `mcpServers` |
| `windsurf` | `~/.codeium/windsurf/mcp_config.json` | `mcpServers` |
| `vscode` | `.vscode/mcp.json` | `servers` |
| `claude-desktop` | OS별 desktop 설정 경로 | `mcpServers` |
