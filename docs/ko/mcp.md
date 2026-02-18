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

- `cgrep mcp install <host>`는 cgrep 실행 경로를 `command`에 기록합니다.
  (가능하면 절대경로) GUI/PATH 불일치 이슈를 줄이기 위함입니다.
- `claude-desktop` 자동 경로는 현재 macOS/Windows에서 구현되어 있습니다.

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

## 설정 파일 대상

| Host | 경로 | 키 |
|---|---|---|
| `claude-code` | `~/.claude.json` | `mcpServers` |
| `cursor` | `~/.cursor/mcp.json` | `mcpServers` |
| `windsurf` | `~/.codeium/windsurf/mcp_config.json` | `mcpServers` |
| `vscode` | `.vscode/mcp.json` | `servers` |
| `claude-desktop` | OS별 desktop 설정 경로 | `mcpServers` |
