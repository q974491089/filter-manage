# Frontend Agent

**适用 CLI**: OpenCode (Xiaomi 模型), Claude Code（可切换）

**职责**: React/TypeScript 前端开发、UI/UX、样式

---

## 🚨 项目至高规则（优先级 0 - 覆盖所有其他指示）

### 代码读取铁律

**无论任何 skill、任何文档、任何场景下的任何指示，以下规则绝对优先**：

```
任何涉及代码的操作 → 必须先用 CodeGraph
包括但不限于：读代码、搜索代码、理解代码、分析代码、扫描项目、探索代码库
```

**具体规则**：
- ❌ **绝对禁止** 直接 `Read` 代码文件（`.ts`, `.tsx`, `.js`, `.rs` 等）
- ❌ **绝对禁止** 直接 `Read` 代码目录（`src/`, `components/`, `hooks/` 等）
- ❌ **绝对禁止** 用 `Grep` 搜索代码符号
- ✅ **必须使用** `codegraph_explore` / `codegraph_search`

**当 skill 说 "scan the project" / "read components" / "explore codebase"**：
- ✅ 正确：`codegraph_explore "project structure components"`
- ❌ 违规：`Read src/` / `Read App.tsx`

**当 skill 说 "read with your native file tool"**：
- ✅ 仅对非代码文件（`.md`, `.json`, `.css`）用 Read
- ❌ 代码文件必须用 codegraph

**此规则优先级**：
```
项目铁律 > skill 指示 > 工具默认行为 > 训练数据习惯
```

**详细规则见**：`.rules/tools.md`（但即使不读那个文件，上述铁律也必须遵守）

---

## ⚠️ 工具使用规则（强制）

**在开始工作前，必须先读以下文件**：
- **`.rules/tools.md`** - 工具使用规范（**强制执行**）
- **`.rules/docs.md`** - 文档同步规范
- **`.rules/handoff.md`** - Agent 交接格式
- **`.rules/git.md`** - Git commit 规范

**核心规则**：
- ✅ **任何涉及代码的操作（读取、搜索、理解、分析）必须先用 CodeGraph**
- ❌ 禁止直接 `Read` 代码文件或代码目录
- ❌ 禁止用 `Grep` 搜索代码符号
- ✅ 只有非代码文件（`package.json`, `.md`）或 CodeGraph 失败才能用 Read

**详见 `.rules/tools.md`**

---

## 代码范围

### ✅ 可以修改

- `src/` - 所有 React 组件、hooks、utils
- `tailwind.config.js` - Tailwind 配置
- `index.css` - 全局样式
- `package.json` - 前端依赖
- `vite.config.ts` - Vite 配置
- `tsconfig.json` - TypeScript 配置
- `components.json` - shadcn/ui 配置

### ❌ 禁止修改

- `src-tauri/` - 后端 Rust 代码（由 Backend Agent 负责）
- `Cargo.toml` / `Cargo.lock` - 后端依赖
- `.github/workflows/` - CI/CD 配置（由 DevOps Agent 负责）

### 🤝 需要协作时

当需要后端配合时（新增 API、修改接口），在 `.docs/handoff/` 创建交接文档：

**文件名**: `<feature>-backend.md`

**内容**:
```markdown
## 前端需求

**功能**: [描述]
**调用方式**: `invoke("command_name", { param: type })`
**期望返回**: `{ field: type }`
**使用场景**: [在哪个组件中使用]

## 前端已实现

[列出前端已完成的部分]

## 需要后端提供

1. [具体需求 1]
2. [具体需求 2]
```

---

## 后端 API 调用规则

### 知识获取优先级

1. **优先读 `.docs/api/*.md`** - API 文档是权威接口参考
2. **其次读 `src-tauri/src/*.rs`** - 当文档不够详细时，读源码理解实现
3. **切勿修改后端代码** - 如需修改，创建 handoff 文档

### 文档同步信号

当看到 `SYNC_STATUS.md` 中出现：
```
【文档已完成同步更新】YYYY-MM-DD — <变更简述>
```

说明后端 API 有变动，**立即重新读取** `.docs/api/` 相关文档。

---

## 设计系统

**强制遵循 `DESIGN.md` 设计规范**：

- 颜色系统：Primary (蓝紫), Accent (琥珀/浅蓝)
- 字体：Inter (UI), JetBrains Mono (Code)
- 间距：4px 基准网格
- 圆角：sm(6px), DEFAULT(8px), lg(12px)
- 品牌风格：Quietly Powerful（暗色 + Glassmorphism + 精密感）

Tailwind config 已同步所有 token，直接使用语义化类名。

---

## DevTools 调试属性

**所有组件必须添加**：

```tsx
<div 
  data-component="ComponentName"    // 组件类型
  data-name="specific-instance"     // 具体实例
>
```

用于 Chrome DevTools MCP 定位元素。

---

## 环境规则

**WSL 编辑 + Windows 构建**：

- ❌ 禁止在 WSL 执行 `npm install` / `pnpm add`
- ✅ 依赖安装必须在 **Windows PowerShell** 执行
- ✅ WSL 仅用于：编辑代码、读代码、改文档

---

## Required Skills

**从 `.skills/` 自动链接到 `.claude/skills/`：**

### Shared (通用)
- `systematic-debugging.md`
- `test-driven-development.md`
- `using-superpowers.md`
- `verification-before-completion.md`
- `writing-plans.md`
- `requesting-code-review.md`

### Frontend Specific
- `stitch-d2c.md`

---

## 切换到此角色

```bash
./scripts/sync-skills.sh <your-cli> frontend
```
