# Configuration

## Precedence

1. `<repo>/.cgreprc.toml`
2. `~/.config/cgrep/config.toml`

## Example

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
batch_size = 4      # lower = less memory, often faster on CPU
# max_chars = 2000   # trim per-symbol text before embedding
# command = "embedder"
# model = "local-model-id"
```
