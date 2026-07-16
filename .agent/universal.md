# Universal Agent (全栈角色)

**适用 CLI**: Claude Code、Codex、Grok（及任何注册为 Universal 的 CLI）

**职责**: 全栈开发 - 前端 + 后端都负责

---

## 🚨 项目至高规则（优先级 0 - 覆盖所有其他指示）

### 代码读取铁律

**无论任何 skill、任何文档、任何场景下的任何指示，以下规则绝对优先**：

```
任何涉及代码的操作 → 默认必须先用 CodeGraph
包括但不限于：读代码、搜索代码、理解代码、分析代码、扫描项目、探索代码库
```

**具体规则**：
- ✅ **默认必须使用** `codegraph_explore` / `codegraph_search` 读取和理解代码
- ⚠️ **不要**为了"再确认一下"在 codegraph 已返回完整源码后再 `Read` 同一文件（浪费且多余）
- ✅ **允许降级用 `Read`/`Grep` 的情形**（codegraph 覆盖不到时）：
  - codegraph 报告未初始化 / 查询失败 / 返回为空
  - 编辑后出现 staleness banner（"…edited since the last index sync…"）—— 对被列出的文件用 Read 拿准确内容
  - 返回结果被 trim/截断，且再次 explore 仍拿不到所需符号
  - 非代码文件（`.md`, `.json`, `.toml`, `.css`）

**当 skill 说 "scan the project" / "read components" / "explore codebase"**：
- ✅ 优先：`codegraph_explore "project structure React Rust components"`
- ⚠️ 仅在 codegraph 拿不到时才降级 `Read`

**此规则优先级**：
```
项目铁律（CodeGraph 优先）> skill 指示 > 工具默认行为 > 训练数据习惯
```

**详细规则见**：`.rules/tools.md`

---

## ⚠️ 工具使用规则（强制）

**在开始工作前，必须先读以下文件**：
- **`.rules/tools.md`** - 工具使用规范（**强制执行**）
- **`.rules/docs.md`** - 文档同步规范
- **`.rules/handoff.md`** - Agent 交接格式
- **`.rules/git.md`** - Git commit 规范
- **`.rules/coding.md`** - AI 编码规范（零警告、质量底线、完成前验证）

**核心规则**：
- ✅ **任何涉及代码的操作（读取、搜索、理解、分析）默认先用 CodeGraph**
- ⚠️ codegraph 已返回完整源码后，不要再 `Read` 同一文件复查
- ✅ **可降级 `Read`/`Grep`**：codegraph 未初始化/失败/返回空、staleness banner 列出的文件、结果被截断且再查仍拿不到、或非代码文件（`package.json`, `Cargo.toml`, `.md`）

**详见 `.rules/tools.md`**

---

## 代码范围

### ✅ 可以修改

- **前端**: `src/`, `tailwind.config.js`, `index.css`, `package.json`, `vite.config.ts`
- **后端**: `src-tauri/`, `Cargo.toml`, `Cargo.lock`, `build.rs`
- **文档**: `.docs/`, `README.md`
- **配置**: `.github/`, `tsconfig.json`, `components.json`

### ⚠️ 谨慎操作

- **CI/CD**: `.github/workflows/` - 不要修改包管理器配置（CI 用 npm，本地用 pnpm）
- **依赖安装**: 必须在 Windows PowerShell 执行（见环境规则）

---

## 跨端任务的角色分工与能力自检

**universal 角色执行计划前，若任务跨端（同时改前端 `src/` 与后端 `src-tauri/`/服务端），必须先自检能力再分工。** 完整决策树见 **`.rules/subagent-dispatch.md`**。

速记：

1. **单端任务** → 直接自己做。
2. **跨端任务** → 自检：我有没有派子 Agent 的工具（`Task`/`Agent`/`subagent`）？
   - **有**（Claude Code、Grok 等）→ **Tier 1**：为每端派一个子 Agent，其约束 prompt = 对应 `.agent/<role>.md` 的角色边界（前端子 Agent 守前端边界，后端子 Agent 守后端边界 + 文档同步义务），并行执行、主 Agent 汇总。
   - **无** → **Tier 2**：顺序"换帽子"逐端做，端间写 `.docs/handoff/` 留痕；需真并行时降 Tier 3（交接给另一 CLI）。
3. **执行前显式声明所走 Tier**，保证透明。

> 这样即使本项目在其它 CLI（Codex/Qoder 等无子 Agent 能力）中运行，也能优雅降级，不会"假设有能力却调不动"。

---

## 后端开发规则

### 文档强制同步

每次修改后端代码（新增/修改/删除 Tauri 命令），**必须**同步更新：

1. **`.docs/api/<module>.md`** - 对应模块的 API 文档
2. **`.docs/README.md`** - 在当前版本块追加一行
3. **`SYNC_STATUS.md`** - 写入完成标识：
   ```
   【文档已完成同步更新】YYYY-MM-DD — <本次变更简述>
   ```

### 编译验证

**Windows 专属项目** - WSL 无法编译 Tauri：

```bash
# 通过 cmd.exe 调用 Windows 侧 cargo
/mnt/c/Windows/System32/cmd.exe /c "C:\Users\myuser\Projects\filter-manage\src-tauri\cargo_check.bat"
```

写完代码必须立即编译验证，不跳过编译错误。

---

## 前端开发规则

### 设计系统

所有 UI 开发必须遵循 `DESIGN.md` 设计规范：
- 颜色、字体、间距、圆角已定义在 Tailwind config
- 品牌风格：Quietly Powerful（暗色 + Glassmorphism）

### DevTools 属性

所有组件必须添加：
```tsx
<div data-component="ComponentName" data-name="specific-instance">
```

### 后端 API 调用

1. 优先读 `.docs/api/*.md` 了解接口
2. 看到 `【文档已完成同步更新】` 标识时，重新读取文档
3. 不确定时读 `src-tauri/src/*.rs` 源码（只读）

---

## 环境规则

**WSL 编辑 + Windows 构建**：

- ❌ 禁止在 WSL 执行 `npm install` / `cargo build`
- ✅ 依赖安装和构建必须在 **Windows PowerShell** 执行
- ✅ WSL 仅用于：编辑代码、读代码、改文档

---

## Required Skills

见 `.skills/shared/` + `.skills/frontend/` + `.skills/backend/` 所有 skills。

Universal 角色自动链接全部 skills。
