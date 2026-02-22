# MCP

## 빠른 설정

```bash
# Codex는 agent installer 사용 (MCP 설정 포함)
cgrep agent install codex

# 직접 MCP host 설치 예시
cgrep mcp install claude-code
cgrep mcp install cursor
cgrep mcp install vscode
```

지원되는 `mcp install` host:
- `claude-code`
- `cursor`
- `windsurf`
- `vscode`
- `claude-desktop`

## 수동 실행 (디버깅)

```bash
cgrep mcp serve
```

## 자주 쓰는 MCP 도구

- `cgrep_search`
- `cgrep_read`
- `cgrep_map`
- `cgrep_definition`
- `cgrep_references`
- `cgrep_callers`
- `cgrep_symbols`
- `cgrep_dependents`
- `cgrep_agent_locate`
- `cgrep_agent_expand`

## 알아두면 좋은 동작

- 대부분 MCP 도구는 기본값으로 `auto_index=true`입니다.
- 인덱스가 없으면 첫 호출에서 자동 bootstrap 합니다.
- refresh는 MCP 호출 시점 + 파일 변경 감지 기반으로 동작합니다.
- 일반 사용에서는 주기적 상시 재인덱싱 루프가 필요하지 않습니다.
- semantic/hybrid는 experimental이며 embeddings 인덱스가 필요합니다.

## 문제 해결

```bash
# MCP 핸드셰이크 빠른 확인
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
| cgrep mcp serve
```

경로 해석이 어긋나면 MCP 도구 인자에 `cwd`를 명시하세요.
