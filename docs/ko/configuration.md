# 설정

## 우선순위

1. `<repo>/.cgreprc.toml`
2. `~/.config/cgrep/config.toml`

## 예시

```toml
max_results = 20
exclude_patterns = ["target/**", "node_modules/**"]

[search]
default_mode = "keyword"

[ranking]
enabled = true
path_weight = 1.2
symbol_weight = 1.8
language_weight = 1.0
changed_weight = 1.2
kind_weight = 2.0
weak_signal_penalty = 1.4
explain_top_k = 5

[cache]
ttl_ms = 600000

[index]
exclude_paths = ["vendor/", "dist/"]
respect_git_ignore = true

[profile.agent]
format = "json2"
max_results = 50
context = 6
context_pack = 8
mode = "keyword"
agent_cache = true

[embeddings]
provider = "builtin" # builtin|command|dummy
batch_size = 4      # 작을수록 메모리 사용량이 낮고 CPU 환경에서 빠른 경우가 많음
# max_chars = 2000   # 임베딩 전 심볼 텍스트 길이 제한
# command = "embedder"
# model = "local-model-id"
```

## 랭킹 튜닝

- 호환성을 위해 `[ranking] enabled` 기본값은 `false`입니다.
- 비활성화 시 keyword 정렬은 기존 동작을 유지합니다.
- 가중치 안전 범위:
  - `path_weight`, `symbol_weight`, `language_weight`, `changed_weight`, `kind_weight`, `weak_signal_penalty`: `0.0..=3.0`
  - `explain_top_k`: `1..=50` (기본값 `5`)
- 범위를 벗어나거나 finite가 아닌 값은 안전한 기본값으로 폴백됩니다.

## 결정적 출력 기본값

- 자동화/에이전트 용도에서는 프로필 기본값을 다음처럼 유지하세요:
  - `[profile.agent].format = "json2"`
  - CLI에서 `--compact` 사용
- 선택 필드는 값이 없으면 생략됩니다(`null` 자리채움 없음).
- `elapsed_ms` 같은 타이밍 필드는 정렬 기준이 아닌 정보성 필드입니다.

## 인덱스 Ignore 정책

- `cgrep index`는 기본적으로 `.gitignore`/`.ignore`를 존중합니다.
- 무시 경로를 전부 포함하려면 `cgrep index --include-ignored`를 사용하세요.
- 무시 경로 중 일부만 포함하려면 `cgrep index --include-path <path>`를 반복 지정하세요.
- 설정 파일 기준으로는 `[index] respect_git_ignore = true|false` (기본값 `true`)와 동일합니다.

## Daemon 인덱스 프로필 재사용

- `cgrep daemon`은 `.cgrep/metadata.json`에 저장된 최근 인덱스 프로필을 재사용합니다.
- 재사용 프로필은 최근 `cgrep index` 실행에 사용된 옵션을 그대로 보존합니다.
- 저장된 프로필이 없으면 `[index]` 설정 기본값으로 동작합니다.

## 아티팩트 호환성 노트

- `.cgrep/status.json`: 백그라운드/인덱스 준비 상태.
- `.cgrep/reuse-state.json`: 재사용 판단/폴백 사유(선택).
- `.cgrep/manifest/`, `.cgrep/metadata.json`: 증분/재사용 메타데이터.

위 파일들은 additive 런타임 아티팩트입니다. 기존 소비자는 알 수 없는 선택 필드를 무시해도 안전합니다.
