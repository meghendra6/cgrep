# Indexing And Watch

## Indexing

```bash
# Rebuild index
cgrep index --force

# Exclude paths while indexing
cgrep index -e vendor/ -e dist/

# Embeddings mode
cgrep index --embeddings auto
cgrep index --embeddings precompute
```

## Watch and daemon

```bash
# Foreground watch
cgrep watch
cgrep w

# Daemon mode (managed background watch)
cgrep daemon start
cgrep daemon status
cgrep daemon stop

# Short alias form
cgrep bg up
cgrep bg st
cgrep bg down
```

Large-repo low-pressure example:

```bash
cgrep watch --debounce 30 --min-interval 180 --max-batch-delay 240

# Short flag form
cgrep w -d 30 -i 180 -b 240
```

Disable adaptive mode (fixed timing behavior):

```bash
cgrep watch --no-adaptive
```

## Behavior notes

- Index lives under `.cgrep/`
- Search from subdirectories reuses nearest parent index
- Indexing ignores `.gitignore`; scan mode respects `.gitignore`
- Watch mode uses adaptive backoff by default (`--no-adaptive` to disable)
- Watch defaults are tuned for background operation (`--min-interval 180`, about 3 minutes)
- Watch reacts only to indexable source extensions and skips temp/swap files
- Watch respects `[index].exclude_paths` for initial and incremental indexing
- Watch reindex is changed-path incremental (update/remove touched files only)

## Watch defaults

| Option | Default | Purpose |
|---|---:|---|
| `--debounce` | `15` | wait for event bursts to settle |
| `--min-interval` | `180` | minimum interval between reindex runs |
| `--max-batch-delay` | `180` | force a run if events keep streaming |
| adaptive mode | `on` | auto-backoff based on change volume/reindex cost |

## Background watch

```bash
# Run in background and write logs
nohup cgrep watch > .cgrep/watch.log 2>&1 &

# Check process/log
pgrep -fl "cgrep watch"
tail -f .cgrep/watch.log

# Stop
pkill -f "cgrep watch"
```

For very large repositories, prefer:
- `--min-interval 180` or higher
- `--debounce 30` or higher
- keeping adaptive mode enabled (default)
