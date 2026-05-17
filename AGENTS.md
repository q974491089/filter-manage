# Project Guidelines

## 后端 Agent 硬性规则

> 本 Agent 为项目专属后端开发 Agent，以下规则优先级高于一切，无例外。

1. **严格职责隔离** — 仅编写后端业务代码（Rust / Tauri 命令），禁止修改任何前端页面、组件、样式相关文件（`src/` 目录下所有 `.tsx` / `.css` 文件）。

2. **文档强制同步** — 每完成任意功能新增、接口修改、参数调整、逻辑改动，无论改动大小，必须第一时间同步更新以下两份文档：
   - `docs/api/<module>.md` — 对应模块的接口文档（标注新增/修改/删除，末尾加日期）
   - `docs/README.md` — 项目迭代目录（在当前版本块追加一行，不修改历史块）

3. **变更描述完整** — 更新文档时清晰写明本次变更内容，标注新增、修改、删减的接口与字段，不遗漏任何细微调整。

4. **完成信号** — 两份文档全部更新完毕后，在项目根目录的 `SYNC_STATUS.md` 写入固定标识：
   ```
   【文档已完成同步更新】YYYY-MM-DD — <本次变更简述>
   ```

5. **文档规范统一** — 所有接口按 `docs/api/icc.md` 的格式书写：命令名、参数类型、返回值、代码示例、日期标注。

## Tool Usage Preferences

### Search & Research
- **优先使用 ctx7 技能** (`npx ctx7@latest`) 查询官方文档（API 语法、配置、版本迁移等）
- **ctx7 没找到的内容，降级使用 Tavily MCP** (`mcp__Tavily__tavily_search` 或 `mcp__Tavily__tavily_research`) 搜索
- 不要使用 `WebSearch`，统一使用 Tavily

### General Rules
- 查询文档优先级：ctx7 > Tavily > WebSearch
- 遇到不熟悉的技术或 API，先查文档再写代码

## Project Overview
- **Tech Stack**: Tauri 2 (Rust backend) + React/Vite (TypeScript frontend) + Tailwind CSS
- **Purpose**: ICC color profile manager and NVIDIA color settings manager
- **Platform**: Windows 10/11

## Coding Behavior Guidelines

### ⚠️ CI 构建硬性规则

> **严禁修改 `.github/workflows/` 中的包管理器配置。**

- 本地开发使用 **pnpm**，CI 构建使用 **npm**（pnpm 在 GitHub Actions 上有兼容性问题无法运行）
- 不要把 CI 里的 `npm ci` 改成 `pnpm install`
- 不要添加 `pnpm/action-setup` action
- 不要修改 workflow 文件中任何与包管理器相关的配置
- 两个 lock 文件共存：`pnpm-lock.yaml`（本地）+ `package-lock.json`（CI），都需要提交到仓库

Behavioral guidelines to reduce common LLM coding mistakes.

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

### 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

### 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

### 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

### 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

## Documentation Maintenance

**Rule: 每次新增或修改后端接口后，必须同步更新 `docs/` 文档。**

### 文档结构

```
docs/
├── README.md              # 项目总入口（功能模块简介 + 文档导航）
├── architecture.md        # 技术栈、项目结构、数据流
├── frontend-guide.md      # 前端组件结构 + 新功能接入清单
└── api/
    ├── icc.md             # ICC 管理接口（icc.rs）
    ├── nvidia.md          # NVIDIA 颜色接口（nvidia.rs）
    └── config.md          # 配置预设接口（config.rs）
```

### 更新规则

| 操作 | 需要更新的文档 |
|------|--------------|
| 新增 Tauri 命令 | 对应 `docs/api/*.md`，标注 `新增于：YYYY-MM-DD`；在 `docs/README.md` 当前迭代版本下追加一行 |
| 修改命令签名/行为 | 对应 `docs/api/*.md`，更新参数/返回值说明；在 `docs/README.md` 当前迭代版本下追加一行 |
| 删除命令 | 对应 `docs/api/*.md` 删除该条目；在 `docs/README.md` 当前迭代版本下追加一行 |
| 新增业务模块（新 .rs 文件） | 新建 `docs/api/<module>.md`；在 `docs/README.md` 固定参考文档表格中添加导航 |
| 影响前端接入方式的变更 | 同步更新 `docs/frontend-guide.md` |

### docs/README.md 迭代目录格式

```markdown
### v<版本> — YYYY-MM-DD · <本次迭代主题>

<一句话描述>

| 功能 | 说明 | 文档 |
|------|------|------|
| 功能名 | 简洁描述 | [具体文档链接] |
```

- 每次迭代新开一个 `### v<版本>` 块，**不修改历史块**
- 每行只写一个功能点，说明控制在 20 字以内
- 文档链接直接指向具体文件（可带锚点）

### 更新格式要求

- 新增接口末尾加一行：`**新增于**：YYYY-MM-DD`
- 修改接口末尾加一行：`**更新于**：YYYY-MM-DD — <简短说明>`
- 保持代码示例与实际签名一致（参数名用 camelCase 对应 Rust snake_case）

## Superpowers Skills

Skills are located in `.kiro/skills/`. Before starting any task, check if a relevant skill applies and read it.

| Skill | When to use |
|-------|-------------|
| `using-superpowers.md` | Start of any session |
| `systematic-debugging.md` | Debugging any issue |
| `test-driven-development.md` | Writing or fixing code |
| `verification-before-completion.md` | Before declaring a fix done |
| `writing-plans.md` | Multi-step implementation tasks |
| `requesting-code-review.md` | Before submitting changes |
| `release-workflow.md` | 发布新版本（用户说"发布"、"发版"、"release"时） |

**Rule**: If there's even a 1% chance a skill applies, read it first.
