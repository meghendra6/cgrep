# cgrep Usability Improvement Notes

## 문제점 (Agent/User)

### Agent
- provider별 설치 지침이 장문이며 중복 내용이 많아 토큰 사용량이 크다.
- provider 간 옵션/규칙 표기가 일관되지 않아 agent 행동이 분산될 수 있다.
- 설치 지침에 과도한 예시/옵션 목록이 포함되어 핵심 사용 규칙이 희석된다.

### User
- `search` 중심 옵션 학습 비용이 높아 실제 사용이 `search`/`definition`에 집중된다.
- grep/ripgrep 전환 사용자에게 익숙한 진입점(`grep` 형태)이 부족하다.
- `-p/--path`, profile/budget/mode 조합의 적용 방식이 직관적이지 않다.

## 근거 파일/라인
- Agent 설치 지침 원문:
  - `src/install/codex.rs:13`
  - `src/install/copilot.rs:15`
  - `src/install/claude_code.rs:14`
  - `src/install/cursor.rs:17`
  - `src/install/opencode.rs:15`
- 사용자 CLI 표면/옵션 경로:
  - `src/cli.rs:216`
  - `src/main.rs:185`
- 사용자 문서 학습 경로:
  - `docs/usage.md:64`
  - `docs/ko/usage.md:64`

## 영향
- Agent: 설치 후 retrieval 규칙 이해에 불필요한 토큰이 소모되고, provider별 행동 편차가 생길 수 있다.
- User: 초기 학습 시간이 길어지고, `grep`/`rg` 사용자의 전환 마찰이 커진다.
- 유지보수: provider별 지침 중복으로 변경 시 동기화 비용이 증가한다.

## 개선안
- 공통 코어 지침 + provider 얇은 레이어로 구조를 통합한다.
- 지침은 핵심 규칙/최소 예시만 남기고 deprecated 표기/중복 옵션표를 제거한다.
- `cgrep search`에 grep 친화 진입(`grep` alias, positional path)을 추가한다.
- 문서는 "grep/rg -> cgrep 전환" 섹션을 앞쪽으로 배치해 핵심 5개 명령 중심으로 재구성한다.

## 기준선 수치 (설치 지침 문자 수, char)

### Baseline (current)
- Codex skill text (`src/install/codex.rs`): 2354
- Claude Code skill text (`src/install/claude_code.rs`): 1601
- Copilot instructions (`src/install/copilot.rs`): 3044
- Cursor rule text (`src/install/cursor.rs`): 1113
- OpenCode skill text (`src/install/opencode.rs`): 1741

### Target
- 각 provider 지침 텍스트를 baseline 대비 30~40% 축소
- 허용 상한: baseline의 70% 이하

## 검증 기준
- 기능 회귀 없음:
  - `tests/definition_command.rs`
  - `tests/usage_modes.rs`
  - `tests/p1_agent_efficiency.rs`
- 토큰/출력 예산 회귀 없음:
  - `tests/search_output_budget.rs`
  - `tests/agent_profiles.rs`
  - `tests/cli_simplification.rs`
- 설치/연동 회귀 없음:
  - `tests/copilot_install.rs`
  - `tests/cursor_install.rs`
  - `tests/mcp_install.rs`
  - `tests/mcp_server.rs`
- 성능 게이트:
  - `python3 scripts/perf_gate.py` 통과

## 비목표 (성능/정확도 비회귀)
- 검색 엔진, 랭킹, 인덱싱 알고리즘 자체 변경은 수행하지 않는다.
  - 예: `src/query/*`, `src/indexer/*`의 핵심 알고리즘 변경 제외
- 이번 변경은 사용성 개선에 한정한다.
- 성능 저하/정확도 저하를 허용하지 않는다.
