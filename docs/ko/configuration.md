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
