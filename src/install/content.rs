// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shared instruction content for agent installation targets.
//!
//! Keep this module compact and provider-agnostic. Provider files should only
//! wrap this core guidance for their own file format.

const CORE_FLOW: &str =
    "- Prefer structured flow: `map -> search -> read -> definition/references/callers`.\n\
- Scope early with `-p`, `--glob`, `--changed`.\n\
- Keep outputs deterministic for agents: `--format json2 --compact`.\n\
- Use `agent locate` then `agent expand` for low-token loops.\n";

const CORE_EXAMPLES: &str = "```bash\n\
cgrep i\n\
cgrep map --depth 2\n\
cgrep s \"authentication flow\" -p src/\n\
cgrep d handleAuth\n\
cgrep r UserService -M auto\n\
cgrep read src/auth.rs\n\
ID=$(cgrep agent locate \"token validation\" --compact | jq -r '.results[0].id')\n\
cgrep agent expand --id \"$ID\" -C 8 --compact\n\
```";

pub(crate) fn codex_skill() -> String {
    format!(
        r#"---
name: cgrep
description: Local code-aware search/navigation for AI coding agents.
license: Apache 2.0
---

## Core Policy

- Use cgrep first for repository navigation and symbol lookup.
{core_flow}- Fall back to host-native grep/read only after two scoped cgrep attempts fail.

## Quick Commands

{examples}

## MCP Policy

- Prefer cgrep MCP tools first: `cgrep_map`, `cgrep_search`, `cgrep_read`, `cgrep_definition`, `cgrep_references`, `cgrep_callers`.
- Keep calls scoped and deterministic.
"#,
        core_flow = CORE_FLOW,
        examples = CORE_EXAMPLES
    )
}

pub(crate) fn claude_skill() -> String {
    format!(
        r#"## cgrep Local Code Search

Use `cgrep` as the default local retrieval tool.

### Core workflow

{core_flow}
### Minimal commands

{examples}

### MCP

- In MCP mode (`cgrep mcp serve`), prefer cgrep tools over host-native search/read tools.
"#,
        core_flow = CORE_FLOW,
        examples = CORE_EXAMPLES
    )
}

pub(crate) fn copilot_instructions() -> String {
    format!(
        r#"---
name: cgrep Local Code Search
description: Use cgrep for local code search instead of built-in grep/search tools.
applyTo: "**/*"
---

# cgrep Local Code Search Instructions

Use cgrep first for local repository navigation.

## Core workflow

{core_flow}
## Minimal commands

{examples}

## Guardrails

- Do not fall back to grep/ripgrep when cgrep can answer.
- Keep queries scoped and deterministic.
- MCP mode: `cgrep mcp serve` / `cgrep mcp install <host>`.
"#,
        core_flow = CORE_FLOW,
        examples = CORE_EXAMPLES
    )
}

pub(crate) fn copilot_appendix() -> String {
    r#"

## cgrep Local Code Search

When searching code in this project:
1. Start with `cgrep map`/`cgrep search` and keep scope narrow.
2. Use `cgrep read` or symbol commands for focused context.
3. Prefer `--format json2 --compact` for machine-readable responses.
"#
    .to_string()
}

pub(crate) fn cursor_rule() -> String {
    let cursor_examples = "```bash\n\
cgrep map --depth 2\n\
cgrep s \"authentication flow\" -p src/\n\
cgrep d handleAuth\n\
ID=$(cgrep agent locate \"token validation\" --compact | jq -r '.results[0].id')\n\
cgrep agent expand --id \"$ID\" -C 8 --compact\n\
```";
    format!(
        r#"---
description: Use cgrep for local code search/navigation instead of ad-hoc grep loops.
globs:
  - "**/*"
alwaysApply: false
---

# cgrep Local Code Search

Use `cgrep` as the default local retrieval tool.

## Core workflow

{core_flow}
## Minimal commands (Cursor)

{cursor_examples}

## MCP

- `cgrep mcp install cursor`
"#,
        core_flow = CORE_FLOW,
        cursor_examples = cursor_examples
    )
}

pub(crate) fn opencode_skill() -> String {
    format!(
        r#"---
name: cgrep
description: Local code-aware search/navigation for AI coding agents.
license: Apache 2.0
---

## Core Policy

- Use cgrep first for local search and symbol lookup.
{core_flow}
## Minimal commands

{examples}
"#,
        core_flow = CORE_FLOW,
        examples = CORE_EXAMPLES
    )
}
