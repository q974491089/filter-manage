# Universal Agent (全栈角色)

**适用 CLI**: Claude Code（当前默认）

**职责**: 全栈开发 - 前端 + 后端都负责

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
- ❌ **绝对禁止** 直接 `Read` 代码目录（`src/`, `src-tauri/`, `components/` 等）
- ❌ **绝对禁止** 用 `Grep` 搜索代码符号
- ✅ **必须使用** `codegraph_explore` / `codegraph_search`

**当 skill 说 "scan the project" / "read components" / "explore codebase"**：
- ✅ 正确：`codegraph_explore "project structure React Rust components"`
- ❌ 违规：`Read src/` / `Read src-tauri/` / `Read App.tsx`

**当 skill 说 "read with your native file tool"**：
- ✅ 仅对非代码文件（`.md`, `.json`, `.toml`, `.css`）用 Read
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
- ✅ 只有非代码文件（`package.json`, `Cargo.toml`, `.md`）或 CodeGraph 失败才能用 Read

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
