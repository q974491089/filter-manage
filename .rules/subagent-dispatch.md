# 子 Agent 分工规则（能力自适应）

**所有 CLI 的 AI 执行计划时遵循。** 解决一个现实问题：本项目可能在不同 CLI（Claude Code / Codex / OpenCode / Qoder / Kiro）中执行，而"派子 Agent"是**部分 CLI 才有的能力**。因此跨端任务的分工必须**先自检能力，再优雅降级**，绝不假设自己有某种能力。

---

## 何时触发本规则

当你**开始执行一个实现计划 / plan** 且任务涉及代码改动时，先走下面的决策树。

## 端的定义

| 端 | 代码范围 |
|----|---------|
| 前端 | `src/`, `tailwind.config.js`, `index.css`, `package.json`, `vite.config.ts` |
| 后端（客户端 Rust） | `src-tauri/`, `Cargo.toml`, `build.rs` |
| 服务端 | 更新服务器 / Spring Boot 等（见对应 plan） |

**跨端任务** = 同一个任务同时需要改 ≥2 个端。

---

## 决策树（每次执行计划前自评）

### 第 1 步：我是什么角色？

读你的 CLI 入口 → `AGENTS.md` 找到自己的角色。

- **角色被锁定为 frontend / backend**（如 OpenCode 锁前端、Kiro 锁后端）：
  → **绝不越界**。只做本端。跨端需求 → 在 `.docs/handoff/<feature>-<对端>.md` 建交接文档 + 写 `SYNC_STATUS.md` 信号，交给对端 Agent。**本规则对你到此结束。**
- **角色是 universal（全栈）**：继续第 2 步。

### 第 2 步：任务是否跨端？

- **单端** → 直接自己做，不必分工。
- **跨端** → 继续第 3 步。

### 第 3 步：我有没有"派子 Agent"的能力？

**自检**：我的工具箱里是否存在可派生子 Agent 的工具？（名字通常类似 `Task` / `Agent` / `subagent` / `dispatch` / `spawn`）

- **有**（如 Claude Code 的 Task/Agent 工具）→ **Tier 1**
- **无**（如 Codex / Qoder 的单 Agent 循环）→ **Tier 2**（必要时降 **Tier 3**）

---

## 三级执行路径

### Tier 1 — 会话内派角色约束子 Agent（能力最强）

1. 按端拆分任务：前端部分、后端部分、服务端部分。
2. 为每个端派一个子 Agent，**其约束 prompt = 对应 `.agent/<role>.md` 的角色边界**（可改什么、禁改什么、文档同步义务）。例如：
   - 后端子 Agent：prompt 注入 `.agent/backend.md` 的代码范围 + "改完同步 `.docs/api/` 并写 handoff"。
   - 前端子 Agent：prompt 注入 `.agent/frontend.md` 的代码范围 + DESIGN.md/DevTools 属性要求。
3. 有依赖就串（后端先定接口 → 前端接），无依赖可并行。
4. 子 Agent 之间通过 `.docs/handoff/` 传接口契约。
5. 主 Agent 汇总结果、跑验证、统一提交。

### Tier 2 — 顺序"换帽子"自己做（无子 Agent 能力）

1. 按端拆分，**逐端顺序完成**，每端严格遵守对应 `.agent/<role>.md` 边界（当前做哪端就只碰哪端的文件）。
2. 端与端之间**写 handoff 文档留痕**（即使是自己交给自己，也保证接口契约可追溯、后续 AI 能看懂）。
3. 后端改完 → 更新 `.docs/api/` + `SYNC_STATUS.md` → 再切前端。

### Tier 3 — 交接给其它 CLI（需要真并行 / 多模型时）

1. 当你无子 Agent 能力，但任务量大、希望多模型真并行（如 Codex 做后端 + 另一个 CLI 做前端）：
2. 完成自己擅长的一端，为另一端建 `.docs/handoff/<feature>-<对端>.md`。
3. **提示用户**手动开启对应 CLI（或用 `./scripts/sync-skills.sh` 切角色）承接另一端。

---

## 强制：显式声明所走路径

执行前必须**明确告诉用户**你的自检结论，例如：

> 检测到本任务跨前端+后端，我是 universal 角色且具备 Task 子 Agent 能力 → 采用 **Tier 1**，派 backend / frontend 两个子 Agent 分工。

或：

> 我是 universal 但无子 Agent 派生能力 → 采用 **Tier 2**，先完成后端并写 handoff，再切前端接入。

这样保证行为透明、可预期，不会"假设有能力却调不动"。

---

## 各 CLI 能力参考矩阵

| CLI | 角色 | 子Agent能力 | 跨端默认路径 |
|-----|------|------------|-------------|
| Claude Code | universal | ✅ | Tier 1 |
| Codex | universal | ❌ | Tier 2（或 Tier 3） |
| Qoder | universal | ✅（`Agent` 工具，支持 Explore/Plan/general-purpose 子Agent + 并行 + worktree 隔离） | Tier 1 |
| OpenCode | 锁 frontend | — | 只做前端 → handoff |
| Kiro | 锁 backend | — | 只做后端 → handoff |

> 矩阵是参考，**以运行时自检为准**：CLI 能力会变，永远先看"我现在到底有没有这个工具"，再决定 Tier。

## 与现有机制的关系

- 复用 `.agent/*.md`（角色边界）、`.rules/handoff.md`（交接格式）、`.rules/docs.md`（文档同步）、`sync-skills.sh`（多 CLI 切角色）。
- 本规则不新增通信机制，只规定"何时分工、用哪种能力分工"。
