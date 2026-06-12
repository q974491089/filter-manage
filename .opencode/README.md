# OpenCode 配置

**角色**: Frontend Agent  
**Agent 配置**: [.opencode/agent.md](.opencode/agent.md)  
**职责文档**: [.agent/frontend.md](.agent/frontend.md)

---

## 🚨 项目至高规则（优先级 0 - 覆盖一切）

### 代码读取铁律（绝对优先）

**无论任何 skill、任何指示、任何场景，以下规则至高无上**：

```
任何涉及代码的操作 → 必须先用 CodeGraph
```

**绝对禁止**：
- ❌ `Read` 代码文件（`.ts`, `.tsx`, `.js`, `.rs` 等）
- ❌ `Read` 代码目录（`src/`, `components/` 等）
- ❌ `Grep` 代码符号

**必须使用**：
- ✅ `codegraph_explore "query"`
- ✅ `codegraph_search "symbol"`

**规则优先级**：
```
项目铁律 > skill 指示 > 任何其他指示
```

**即使 skill 说以下内容，也必须用 codegraph**：
- "read components"
- "scan project"
- "use native file tool"
- "Read src/"
- "explore codebase"

---

## ⚠️ 强制约束

### 🚫 核心规则（零容忍）

**任何涉及代码的操作，必须先用 CodeGraph。没有 "或者"，没有 "可以直接"。**

**包括**：
- 读代码文件（`.ts`, `.tsx`, `.js`, `.rs` 等）
- 读代码目录（`src/`, `components/`, `hooks/` 等）
- 搜索函数/类/变量
- 理解代码结构
- 分析调用关系

**禁止的思考方式**：
- ❌ "使用 codegraph 或者直接 Read" - **没有"或者"！**
- ❌ "先尝试直接读取" - **不是"先尝试"，是"只能"！**
- ❌ "codegraph 可能慢，直接 Read 更快" - **不准确 > 快！**

**唯一合法的思考**：
- ✅ "需要理解代码，必须用 codegraph_explore"
- ✅ "先用 codegraph，如果没结果再考虑 Read"
- ✅ "这是代码目录，不能直接 Read，用 codegraph_explore"

**流程（无例外）**：
```
需要操作代码
  ↓
1. 用 codegraph_explore（第一步，强制）
  ↓
2. 结果不够？用 codegraph_search
  ↓
3. 还不够？说明为什么 codegraph 不行
  ↓
4. 才能用 Read（需要理由）
```

**唯一例外**（非代码文件）：
- `package.json`, `.md`, `.css` - 可以直接 Read
- codegraph 返回 "not found" - 可以降级 Read

---

## 🚀 Multi-Agent Framework

本项目采用多 Agent 协作框架。

**请先读以下文件（按顺序）**：
1. **`AGENTS.md`** - Agent 注册表，找到你的角色
2. **`.agent/frontend.md`** - 你的详细职责文档
3. **`.rules/tools.md`** - 统一工具规则（**强制执行**）

---

## 你的角色

根据 `AGENTS.md`，你是 **Frontend Agent**。

- ✅ 可以修改：`src/`, `tailwind.config.js`, `package.json`
- ❌ 禁止修改：`src-tauri/`, `Cargo.toml`
- 🤝 需要后端时：创建 `.docs/handoff/<feature>-backend.md`

---

## 工具使用规范

### CodeGraph MCP（强制第一选择）

**必须使用 MCP 工具**：
- `mcp__codegraph__codegraph_explore` - 代码理解（**首选**）
- `mcp__codegraph__codegraph_search` - 搜索 symbol
- `mcp__codegraph__codegraph_node` - 查看单个 symbol
- `mcp__codegraph__codegraph_callers` - 查找调用者

**MCP 不可用才降级 CLI**：
```bash
cmd.exe /c "C:\Users\Administrator\AppData\Local\pnpm\bin\codegraph.CMD" context --path "C:\Users\myuser\Projects\filter-manage" "query"
```

**详见**：`.rules/tools.md`（包含违规后果说明）

---

## Skills

你的 skills 在 `.opencode/skills/` 目录（通过 symlink 链接到 `.skills/`）。

如果目录不存在，运行：
```bash
./scripts/sync-skills.sh opencode frontend
```
