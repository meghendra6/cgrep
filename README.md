# lgrep

> Local semantic code search tool with AST and BM25 support

**lgrep** is a high-performance, fully local code search tool that combines:
- **tree-sitter** for AST-aware symbol extraction
- **tantivy** for BM25-ranked full-text search
- **ripgrep's ignore crate** for respecting .gitignore

## Features

- **Zero cloud dependencies** - All processing is local, no data leaves your machine
- **Single binary** - No runtime or dependencies required (~5MB)
- **Fast** - ripgrep-level file scanning performance
- **AST-aware** - Understands code structure (functions, classes, etc.)
- **BM25 ranking** - Relevant results, not just pattern matches
- **Multi-language** - TypeScript, JavaScript, Python, Rust, Go, C, C++, Java, Ruby
- **Shell completions** - Tab completion for Bash, Zsh, Fish, PowerShell
- **Agent integrations** - Works with Claude Code, Codex, Copilot, OpenCode

## Installation

### From Source

```bash
cd lgrep
cargo build --release
cp target/release/lgrep ~/.local/bin/
```

### Using Cargo

```bash
cargo install --path .
```

## Quick Start

```bash
# Build the search index (run once)
lgrep index

# Search for code
lgrep search "authentication flow"

# Find a symbol definition
lgrep definition handleAuth

# Find all callers of a function
lgrep callers validateToken

# Find all references to a symbol
lgrep references MyClass

# Search for symbols by type
lgrep symbols UserService --type class
```

## Commands

| Command | Description |
|---------|-------------|
| `lgrep search <query>` | Full-text search with BM25 ranking |
| `lgrep symbols <name>` | Search for symbols by name |
| `lgrep definition <name>` | Find symbol definition location |
| `lgrep callers <function>` | Find all callers of a function |
| `lgrep references <name>` | Find all references to a symbol |
| `lgrep dependents <file>` | Find files that depend on a file |
| `lgrep index` | Build or rebuild the search index |
| `lgrep watch` | Watch for file changes and update index |
| `lgrep completions <shell>` | Generate shell completions |

## Search Command Flags

```bash
lgrep search <query> [options]
```

| Flag | Description |
|------|-------------|
| `-p, --path <path>` | Path to search in (defaults to current directory) |
| `-m, --max-results <n>` | Maximum number of results (default: 20) |
| `-C, --context <n>` | Show N lines before and after each match |
| `-t, --type <type>` | Filter by file type/language (e.g., rust, ts, python) |
| `-g, --glob <pattern>` | Filter files matching glob pattern (e.g., "*.rs") |
| `--exclude <pattern>` | Exclude files matching pattern |
| `-q, --quiet` | Suppress statistics output |
| `-f, --fuzzy` | Enable fuzzy matching (allows 1-2 character differences) |
| `--format <text\|json>` | Output format |

## Symbols Command Flags

```bash
lgrep symbols <name> [options]
```

| Flag | Description |
|------|-------------|
| `-T, --type <type>` | Filter by symbol type (function, class, variable, etc.) |
| `-l, --lang <lang>` | Filter by language (typescript, python, rust, etc.) |
| `-t, --file-type <type>` | Filter by file type |
| `-g, --glob <pattern>` | Filter files matching glob pattern |
| `--exclude <pattern>` | Exclude files matching pattern |
| `-q, --quiet` | Suppress statistics output |

## References Command Flags

```bash
lgrep references <name> [options]
```

| Flag | Description |
|------|-------------|
| `-p, --path <path>` | Path to search in (defaults to current directory) |
| `-m, --max-results <n>` | Maximum number of results (default: 50) |
| `--format <text\|json>` | Output format |

## Configuration

### Config File

lgrep supports configuration via `.lgreprc.toml` in your project directory or `~/.config/lgrep/config.toml` for global settings:

```toml
# .lgreprc.toml
max_results = 20
default_format = "text"  # or "json"
```

### Index Location

lgrep stores its index in `.lgrep/` directory in your project root. Add this to your `.gitignore`:

```
.lgrep/
```

## Shell Completions

Generate shell completions for your preferred shell:

```bash
# Bash
lgrep completions bash > ~/.local/share/bash-completion/completions/lgrep

# Zsh
lgrep completions zsh > ~/.zfunc/_lgrep

# Fish
lgrep completions fish > ~/.config/fish/completions/lgrep.fish

# PowerShell
lgrep completions powershell > $PROFILE.CurrentUserAllHosts
```

## Agent Integrations

lgrep integrates with AI coding agents for enhanced code understanding:

### Claude Code

```bash
lgrep install-claude-code    # Install integration
lgrep uninstall-claude-code  # Uninstall
```

### OpenAI Codex

```bash
lgrep install-codex    # Install integration
lgrep uninstall-codex  # Uninstall
```

### GitHub Copilot

```bash
lgrep install-copilot    # Install integration
lgrep uninstall-copilot  # Uninstall
```

### OpenCode

```bash
lgrep install-opencode    # Install integration
lgrep uninstall-opencode  # Uninstall
```

## Supported Languages

| Language | File Extensions | AST Support | Full-text |
|----------|----------------|-------------|-----------|
| TypeScript | .ts, .tsx | ✅ | ✅ |
| JavaScript | .js, .jsx | ✅ | ✅ |
| Python | .py | ✅ | ✅ |
| Rust | .rs | ✅ | ✅ |
| Go | .go | ✅ | ✅ |
| C | .c, .h | ✅ | ✅ |
| C++ | .cpp, .cc, .hpp | ✅ | ✅ |
| Java | .java | ✅ | ✅ |
| Ruby | .rb | ✅ | ✅ |
| Other | * | ❌ | ✅ |

## Examples

### Full-text Search

```bash
$ lgrep search "error handling"

✓ Found 15 results for: error handling

➜ src/lib/auth.ts (score: 8.59)
    throw new Error("Authentication failed");

➜ src/commands/search.ts (score: 7.23)
    } catch (error) {
```

### Full-text Search with Context

```bash
$ lgrep search "auth middleware" -C 2 -t typescript

✓ Found 5 results for: auth middleware

➜ src/middleware/auth.ts (score: 9.12)
    // Previous line
    export const authMiddleware = async (req, res, next) => {
    // Next line
```

### Fuzzy Search

```bash
$ lgrep search "authentcation" --fuzzy  # Note typo

✓ Found 12 results (fuzzy matching)
```

### Symbol Search

```bash
$ lgrep symbols handleAuth --type function

🔍 Searching for symbol: handleAuth

  [function] handleAuth src/lib/auth.ts:45
```

### Find Definition

```bash
$ lgrep definition FileScanner

🔍 Finding definition of: FileScanner

  [struct] FileScanner lgrep/src/indexer/scanner.rs:20:1

  ➜   20 pub struct FileScanner {
      21     root: PathBuf,
      22     extensions: Vec<String>,
```

### Find Callers

```bash
$ lgrep callers validateToken

🔍 Finding callers of: validateToken

  src/api/routes.ts:45 const result = validateToken(token);
  src/middleware/auth.ts:23 if (!validateToken(req.token)) {
```

### Find References

```bash
$ lgrep references UserService

🔍 Finding references of: UserService

  src/services/user.ts:5:14 export class UserService {
  src/api/routes.ts:12:22 const service = new UserService();
  src/tests/user.test.ts:8:10 describe('UserService', () => {

✓ Found 3 references
```

### JSON Output

```bash
$ lgrep search "config" --format json
[
  {
    "path": "src/config.ts",
    "line": 10,
    "score": 8.5,
    "content": "export const config = { ... }"
  }
]
```

## Performance

Compared to traditional tools:

| Metric | grep | ripgrep | lgrep |
|--------|------|---------|-------|
| File scan | 1x | 10-50x | 10-50x |
| Code understanding | ❌ | ❌ | ✅ |
| Ranking | ❌ | ❌ | ✅ (BM25) |
| Symbol search | ❌ | ❌ | ✅ |
| Dependency tracking | ❌ | ❌ | ✅ |

## Environment Variables

| Variable | Description |
|----------|-------------|
| `LGREP_LOG` | Set log level (e.g., `debug`, `info`, `warn`) |
| `NO_COLOR` | Disable colored output |

## License

MIT
