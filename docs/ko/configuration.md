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

## 인덱스 Ignore 정책

- `cgrep index`는 기본적으로 `.gitignore`/`.ignore`를 존중합니다.
- 무시 경로를 전부 포함하려면 `cgrep index --include-ignored`를 사용하세요.
- 무시 경로 중 일부만 포함하려면 `cgrep index --include-path <path>`를 반복 지정하세요.
- 설정 파일 기준으로는 `[index] respect_git_ignore = true|false` (기본값 `true`)와 동일합니다.

## Watch/daemon 인덱스 프로필 재사용

- `cgrep watch`와 `cgrep daemon`은 `.cgrep/metadata.json`에 저장된 최근 인덱스 프로필을 재사용합니다.
- 재사용 프로필은 최근 `cgrep index` 실행에 사용된 옵션을 그대로 보존합니다.
- 저장된 프로필이 없으면 `[index]` 설정 기본값으로 동작합니다.
