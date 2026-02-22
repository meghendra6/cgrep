# cgrep（中文）

[English](./README.en.md) | [한국어](./README.ko.md) | [中文](./README.zh.md)

面向开发者和 AI 编码代理的本地优先代码搜索工具。

`grep` 适合定位文本，`cgrep` 更适合快速定位“功能是在哪里实现的”。

## 为什么选择 cgrep

- 基于 Tantivy 的本地高速索引（不依赖云端）
- 代码导航命令：`definition`、`references`、`callers`、`read`、`map`
- 面向代理的稳定输出：`--format json2 --compact`
- 支持 Codex、Claude Code、Cursor、Copilot 等 MCP 集成

## 30 秒安装

```bash
curl -fsSL https://raw.githubusercontent.com/meghendra6/cgrep/main/scripts/install_release.sh | bash
cgrep --help
```

## 2 分钟上手

```bash
# 可选：预热索引
cgrep index

# 日常检索流程
cgrep s "token validation" src/
cgrep d handle_auth
cgrep r UserService
cgrep read src/auth.rs
cgrep map --depth 2
```

## AI 编码代理

### 1) 一次性安装（按你的宿主工具选择）

```bash
cgrep agent install codex
cgrep agent install claude-code
cgrep agent install cursor
cgrep agent install copilot
cgrep agent install opencode
```

### 2) 必需项与可选项

- 必需：安装后重启当前代理会话一次。
- 普通使用不需要：手动执行 `cgrep index` 或 `cgrep daemon start`。
- 可选：在长时间、高频改动会话中可开启 daemon 让索引保持热状态。

### 可选：CLI 两阶段检索示例

```bash
ID=$(cgrep agent locate "where token validation happens" --compact | jq -r '.results[0].id')
cgrep agent expand --id "$ID" -C 8 --compact
cgrep --format json2 --compact agent plan "trace authentication middleware flow"
```

## 索引模式（简明规则）

- 偶发使用：直接运行 `search/definition/read`（自动 bootstrap）
- 持续开发：`cgrep daemon start`，结束后 `cgrep daemon stop`
- semantic/hybrid：实验特性，需要 embeddings 索引

## 基准快照（PyTorch, Codex, runs=2）

- 日期：**2026-02-22 (UTC)**
- baseline billable tokens：**151,466**
- cgrep billable tokens：**69,874**
- 降幅：**53.9%**

完整报告：[`docs/benchmarks/pytorch-codex-agent-efficiency.md`](./docs/benchmarks/pytorch-codex-agent-efficiency.md)

## 文档

- 文档站点：<https://meghendra6.github.io/cgrep/>
- 安装：[`docs/installation.md`](./docs/installation.md)
- 使用：[`docs/usage.md`](./docs/usage.md)
- Agent 工作流：[`docs/agent.md`](./docs/agent.md)
- MCP：[`docs/mcp.md`](./docs/mcp.md)
- 索引/daemon：[`docs/indexing-watch.md`](./docs/indexing-watch.md)
- 故障排查：[`docs/troubleshooting.md`](./docs/troubleshooting.md)

## 发布

- 当前版本：**v1.5.2**
- 更新日志：[`CHANGELOG.md`](./CHANGELOG.md)
