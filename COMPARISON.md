# Code Search Tool Comparison

> A comprehensive comparison of **cgrep** (formerly lgrep) with popular code search tools.

## Executive Summary

**cgrep** is a local-first, AI-agent-optimized code search tool combining BM25 full-text search with AST-based semantic understanding. Unlike traditional pattern-matching tools, cgrep is purpose-built for integration with AI coding agents (GitHub Copilot, Claude Code, Codex, OpenCode).

| Tool | Primary Paradigm | Best For |
|------|------------------|----------|
| **cgrep** | BM25 + AST semantic | AI agent workflows, natural language queries |
| **Semgrep** | SAST pattern rules | Security scanning, linting |
| **ast-grep** | AST structural patterns | Refactoring, code transforms |
| **ugrep** | Regex pattern matching | Raw speed text search |
| **codegrep** | Pattern search | Quick local lookup |
| **grep.app** | Web indexed search | Public repo exploration |

---

## Tool Overview

### cgrep (formerly lgrep)

**cgrep** is a high-performance, fully local code search tool combining:

- **BM25 Search** - Tantivy-powered full-text search with intelligent ranking
- **AST Analysis** - tree-sitter for semantic symbol extraction
- **Incremental Indexing** - mtime-based for fast updates
- **Parallel Processing** - rayon for multi-core utilization
- **JSON Output** - Machine-readable output for AI agent consumption
- **Agent Integration** - Built-in install commands for popular AI agents

**Tech Stack**: Rust, Tantivy, tree-sitter, rayon

**Supported Languages**: TypeScript, TSX, JavaScript, Python, Rust, Go, C, C++, Java, Ruby

\`\`\`bash
# Natural language search
cgrep search "authentication flow"

# Find symbol definitions
cgrep definition handleAuth

# Find all callers
cgrep callers validateToken

# JSON output for AI agents
cgrep search "config" --format json
\`\`\`

---

### Semgrep

**Website**: https://github.com/semgrep/semgrep

Semgrep is a lightweight static analysis tool for finding bugs and enforcing code standards. It uses semantic patterns to match code.

**Key Characteristics**:
- SAST (Static Application Security Testing) focus
- Declarative rule-based patterns
- Language-agnostic metavariable matching
- Cloud-based rule registry
- CI/CD integration focus

\`\`\`yaml
# Semgrep rule example
rules:
  - id: insecure-eval
    pattern: eval(\$X)
    message: "Avoid eval()"
    severity: WARNING
\`\`\`

---

### ast-grep

**Website**: https://ast-grep.github.io/

ast-grep uses AST (Abstract Syntax Tree) structural patterns for code search and transformation.

**Key Characteristics**:
- AST-based structural matching
- Code rewriting capabilities
- Pattern syntax similar to target code
- Language-aware search
- Designed for refactoring workflows

\`\`\`bash
# Find React hooks
ast-grep -p 'useState(\$_)'

# Rewrite patterns
ast-grep -p 'console.log(\$_)' -r '' --rewrite
\`\`\`

---

### ugrep

**Website**: https://ugrep.com/

ugrep is an ultra-fast grep replacement with extensive format support and regex capabilities.

**Key Characteristics**:
- Fastest regex-based search
- Multi-format file support (PDF, docs, archives)
- Unicode-aware
- grep-compatible syntax
- Interactive TUI mode

\`\`\`bash
# Fuzzy search with context
ugrep -Z3 "authen" *.py

# Search in binary and archives
ugrep --format=pdf,docx "password"
\`\`\`

---

### codegrep

**Website**: https://github.com/codegrep/codegrep

codegrep provides real-time code search with pattern matching.

**Key Characteristics**:
- Real-time file watching
- Pattern-based matching
- Fast incremental search
- Simple CLI interface

---

### grep.app (by Vercel)

**Website**: https://grep.app/

A web-based code search engine that indexes public GitHub repositories.

**Key Characteristics**:
- Web interface, no local installation
- Indexes 500k+ public repos
- Real-time search
- Regex support
- GitHub integration

---

## Feature Comparison Matrix

| Feature | cgrep | Semgrep | ast-grep | ugrep | codegrep | grep.app |
|---------|--------|---------|----------|-------|----------|----------|
| **Search Type** | BM25 + AST | Pattern rules | AST patterns | Regex | Pattern | Regex |
| **Local-first** | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ (cloud) |
| **Offline capable** | ✅ | ⚠️ (rules online) | ✅ | ✅ | ✅ | ❌ |
| **Semantic ranking** | ✅ BM25 | ❌ | ❌ | ❌ | ❌ | Basic |
| **AST awareness** | ✅ tree-sitter | ✅ | ✅ | ❌ | ❌ | ❌ |
| **Symbol extraction** | ✅ | ❌ | ⚠️ | ❌ | ❌ | ❌ |
| **Code rewriting** | ❌ | ⚠️ (autofix) | ✅ | ❌ | ❌ | ❌ |
| **Security rules** | ❌ | ✅ (SAST) | ❌ | ❌ | ❌ | ❌ |
| **Incremental indexing** | ✅ mtime | N/A | ❌ | ❌ | ⚠️ | N/A |
| **JSON output** | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| **AI agent integration** | ✅ built-in | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Natural language queries** | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Definition lookup** | ✅ | ❌ | ⚠️ | ❌ | ❌ | ❌ |
| **Caller/callee analysis** | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Reference finding** | ✅ | ⚠️ | ⚠️ | ❌ | ❌ | ❌ |
| **Fuzzy matching** | ✅ | ❌ | ❌ | ✅ | ❌ | ❌ |
| **Multi-language support** | 10+ languages | 30+ languages | 10+ languages | Any | Limited | Any |
| **Search speed** | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐ |
| **Setup complexity** | Low | Medium | Low | Low | Low | None |

### Legend
- ✅ Full support
- ⚠️ Partial/limited support
- ❌ Not supported

---

## AI Agent Integration

### Why cgrep is Superior for AI Agents

| Capability | cgrep | Others |
|------------|--------|--------|
| **One-command install** | \`cgrep install-copilot\` | Manual configuration |
| **Structured JSON output** | Native \`--format json\` | Varies or parsing needed |
| **Natural language queries** | Built-in BM25 ranking | Requires exact patterns |
| **Symbol-aware search** | Functions, classes, methods | Text-only matching |
| **Incremental updates** | Automatic mtime-based | Manual re-indexing |
| **Agent configuration** | Auto-updates CLAUDE.md, AGENTS.md | Manual setup |

### Supported AI Agents

\`\`\`bash
# GitHub Copilot
cgrep install-copilot

# Claude Code
cgrep install-claude-code

# OpenAI Codex
cgrep install-codex

# OpenCode
cgrep install-opencode
\`\`\`

### Why This Matters for AI Workflows

1. **Natural Language Understanding**: AI agents speak natural language. cgrep's BM25 ranking handles queries like "find authentication middleware" without requiring exact regex patterns.

2. **Structured Output**: AI agents parse JSON efficiently. cgrep's \`--format json\` provides machine-readable results with file paths, line numbers, and context.

3. **Symbol Awareness**: Instead of text matching, cgrep understands code structure—finding function definitions, class declarations, and symbol references.

4. **Zero Cloud Dependency**: Works fully offline. No API keys, no network latency, no privacy concerns.

5. **Fast Incremental Updates**: The index updates automatically based on file modification times. No full re-index on every query.

---

## Performance Characteristics

### Index-Based vs Real-Time

| Tool | Model | Trade-off |
|------|-------|-----------|
| **cgrep** | Pre-built index (Tantivy) | Initial indexing cost, near-instant queries |
| **Semgrep** | On-the-fly parsing | Per-query parsing overhead |
| **ast-grep** | On-the-fly parsing | Per-query parsing overhead |
| **ugrep** | Real-time file scan | No index, fastest for simple patterns |
| **codegrep** | Real-time with caching | Moderate speed |
| **grep.app** | Pre-built cloud index | Network latency |

### Benchmark Characteristics

| Metric | cgrep Approach |
|--------|-----------------|
| **Indexing** | Parallel (rayon), incremental (mtime) |
| **Search** | BM25 via Tantivy (sub-100ms typical) |
| **Memory** | Index on disk, memory-mapped access |
| **Scaling** | Handles 100k+ file repositories |

**Note**: Actual performance depends on project size, hardware, and query complexity.

---

## Use Case Decision Matrix

### When to Use Each Tool

| Use Case | Recommended Tool | Why |
|----------|------------------|-----|
| **AI agent integration** | cgrep | Built-in agent support, JSON output, natural language |
| **Security scanning** | Semgrep | Extensive SAST rules, CVE detection |
| **Code refactoring** | ast-grep | AST rewriting, structural transforms |
| **Raw text speed** | ugrep | Fastest regex, no index overhead |
| **Quick local lookup** | codegrep | Simple, no index required |
| **Public repo exploration** | grep.app | Web-based, no install |
| **Symbol navigation** | cgrep | Definition/reference/caller tracking |
| **Offline code search** | cgrep, ugrep | Full local operation |
| **CI/CD linting** | Semgrep, ast-grep | Rule-based, automation-friendly |

### Workflow Examples

**Scenario 1: GitHub Copilot wants to understand your codebase**
\`\`\`bash
# cgrep provides semantic understanding
cgrep search "error handling patterns" --format json
cgrep definition CustomError --format json
cgrep callers logError --format json
\`\`\`

**Scenario 2: Security audit before release**
\`\`\`bash
# Semgrep excels here
semgrep --config=p/owasp-top-ten .
\`\`\`

**Scenario 3: Rename a function across codebase**
\`\`\`bash
# ast-grep for structural transforms
ast-grep -p 'oldFunction(\$_)' -r 'newFunction(\$_)' --rewrite
\`\`\`

**Scenario 4: Quick text search**
\`\`\`bash
# ugrep for speed
ugrep -r "TODO" --include="*.js"
\`\`\`

---

## Integration Comparison

### Agent Configuration Files

| Agent | cgrep Integration | Other Tools |
|-------|-------------------|-------------|
| Claude Code | Adds to \`CLAUDE.md\` | Manual config |
| GitHub Copilot | Adds to \`.github/copilot-instructions.md\` | Manual config |
| Codex | Adds to \`AGENTS.md\` | Manual config |
| OpenCode | Adds to OpenCode config | Manual config |

### Example: Claude Code Integration

After running \`cgrep install-claude-code\`, your \`CLAUDE.md\` includes:

\`\`\`markdown
## Code Search

Use cgrep for semantic code search:

- \`cgrep search "query"\` - Full-text search with BM25 ranking
- \`cgrep symbols <name>\` - Find symbols by name
- \`cgrep definition <name>\` - Find symbol definitions
- \`cgrep callers <function>\` - Find all callers
- \`cgrep references <name>\` - Find all references
\`\`\`

---

## Limitations

### cgrep Limitations
- Requires initial indexing (1-10 seconds for typical repos)
- No code rewriting (use ast-grep for transforms)
- No security rule database (use Semgrep for SAST)
- Language support limited to tree-sitter availability

### When NOT to Use cgrep
- One-off quick text search (use ripgrep/ugrep)
- Security scanning (use Semgrep)
- Code transforms/refactoring (use ast-grep)
- Searching public repos you don't have locally (use grep.app)

---

## Summary: The Right Tool for the Job

| Your Need | Best Choice |
|-----------|-------------|
| AI agent code understanding | **cgrep** |
| Security vulnerability detection | **Semgrep** |
| Structural code refactoring | **ast-grep** |
| Maximum raw search speed | **ugrep** |
| Web-based public repo search | **grep.app** |
| Simple quick search | **ripgrep** / **ugrep** |

**cgrep shines when**:
1. You're working with AI coding agents
2. You need symbol-aware search (definitions, callers, references)
3. You prefer natural language queries over regex
4. You want fully local, offline-capable search
5. You need machine-readable JSON output
6. You value incremental, fast index updates

---

## Getting Started with cgrep

\`\`\`bash
# Build and install
cargo build --release
cp target/release/cgrep ~/.local/bin/

# Index your project
cgrep index

# Start searching
cgrep search "authentication flow"
cgrep definition UserService
cgrep callers validateToken

# Install for your AI agent
cgrep install-copilot    # GitHub Copilot
cgrep install-claude-code # Claude Code
cgrep install-codex       # OpenAI Codex
cgrep install-opencode    # OpenCode
\`\`\`

---

## Links

| Tool | Repository/Website |
|------|-------------------|
| cgrep | (this project) |
| Semgrep | https://github.com/semgrep/semgrep |
| ast-grep | https://ast-grep.github.io/ |
| ugrep | https://ugrep.com/ |
| codegrep | https://github.com/codegrep/codegrep |
| grep.app | https://grep.app/ |

---

*Last updated: February 2026*
