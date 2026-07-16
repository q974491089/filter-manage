# Backend Agent

**适用 CLI**: QoderCLI (Qwen 模型), Claude Code（可切换）

**职责**: Rust/Tauri 后端开发、Windows API 集成、系统级功能

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

- ⚠️ codegraph 已返回完整源码后，**不要**再 `Read` 同一文件复查（多余）

- ✅ **允许降级用 `Read`/`Grep`** 的情形（codegraph 覆盖不到时）：codegraph 未初始化/失败/返回空、编辑后 staleness banner 列出的文件、结果被截断且再查仍拿不到所需符号、或非代码文件（`.md`, `.toml`, `.json`）

**当 skill 说 "scan the project" / "read components" / "explore codebase"**：

- ✅ 优先：`codegraph_explore "Rust modules Tauri commands"`

- ⚠️ 仅在 codegraph 拿不到时才降级 `Read`

**此规则优先级**：

```
项目铁律（CodeGraph 优先）> skill 指示 > 工具默认行为 > 训练数据习惯
```

**详细规则见**：`.rules/tools.md`

---

## ⚠️ 工具使用规则（强制）

**在开始工作前，必须先读以下文件**：

- `.rules/tools.md` - 工具使用规范（**强制执行**）
- `.rules/coding.md` - AI 编码规范（零警告、Rust/COM、完成前验证）
- `.rules/docs.md` - 文档同步规范

- `.rules/handoff.md` - Agent 交接格式

- `.rules/git.md` - Git commit 规范

**核心规则**：

- ✅ **任何涉及代码的操作（读取、搜索、理解、分析）默认先用 CodeGraph**

- ⚠️ codegraph 已返回完整源码后，不要再 `Read` 同一文件复查

- ✅ **可降级 `Read`/`Grep`**：codegraph 未初始化/失败/返回空、staleness banner 列出的文件、结果被截断且再查仍拿不到、或非代码文件（`Cargo.toml`, `.md`）

**详见** `.rules/tools.md`

---

## 代码范围

### ✅ 可以修改

- `src-tauri/` - 所有 Rust 代码

- `Cargo.toml` / `Cargo.lock` - 后端依赖

- `build.rs` - 构建脚本

- `.docs/api/` - API 文档（**必须同步更新**）

- `.docs/README.md` - 迭代记录

- `.github/workflows/` - CI/CD 配置（与 DevOps Agent 协商）

### ❌ 禁止修改

- `src/` - 前端代码（由 Frontend Agent 负责）

- `tailwind.config.js` / `index.css` - 前端样式

- `package.json` - 前端依赖

### 🤝 需要协作时

当 API 变更影响前端时，在 `.docs/handoff/` 创建交接文档：

**文件名**: `<feature>-frontend.md`

**内容**:

````markdown
## 后端已完成

**新增命令**: `command_name`
**参数**: `{ param: type }`
**返回值**: `{ field: type }`

## 前端需要实现

1. 在 `<Component.tsx>` 中调用：
   ```typescript
   const result = await invoke("command_name", { param: value });
````

2. 处理返回值：\[具体说明\]

3. 错误处理：\[错误场景\]

## 示例代码

\[完整前端调用示例\]

````

---

## 文档强制同步规则

**每次修改后端代码，必须同步更新文档：**

### 1. 更新 API 文档

修改 `.docs/api/<module>.md`：

- **新增接口**: 末尾加 `**新增于**: YYYY-MM-DD`
- **修改接口**: 末尾加 `**更新于**: YYYY-MM-DD — <简短说明>`
- **删除接口**: 直接删除该条目

### 2. 更新迭代记录

在 `.docs/README.md` 当前版本块追加一行：

```markdown
### v<版本> — YYYY-MM-DD · <本次迭代主题>

| 功能 | 说明 | 文档 |
|------|------|------|
| [新增功能] | [简洁描述] | [.docs/api/xxx.md#anchor] |
````

### 3. 写入完成信号

在 `SYNC_STATUS.md` 写入固定标识：

```
【文档已完成同步更新】YYYY-MM-DD — <本次变更简述>
```

**这个信号会触发 Frontend Agent 重新读取文档。**

---

## 编译验证规则

**Windows 专属项目** - WSL 无法编译 Tauri。

### 编译命令

通过 `cmd.exe` 调用 Windows 侧 cargo：

```bash
# 创建 bat 文件（首次）
# src-tauri/cargo_check.bat 内容：
# cd /d C:\Users\myuser\Projects\filter-manage\src-tauri && cargo check 2>&1

# 执行编译检查
/mnt/c/Windows/System32/cmd.exe /c "C:\Users\myuser\Projects\filter-manage\src-tauri\cargo_check.bat"
```

### 强制规则

1. **写完代码必须编译验证** - 每次修改 Rust 代码后立即 `cargo check`

2. **编译错误立即修复** - 不跳过，逐条修复

3. **API 不确定时先查文档** - 用 Tavily 搜索 `docs.rs`，不猜测签名

4. **不用 WSL 的 cargo** - WSL 编译的是 Linux 目标，必须用 Windows cargo

5. **cargo_check.bat 是临时文件** - 编译完删除，不提交

---

## 环境规则

**WSL 编辑 + Windows 构建**：

- ❌ 禁止在 WSL 执行 `cargo build` / `cargo check`

- ✅ 构建命令必须在 **Windows PowerShell** 执行

- ✅ WSL 仅用于：编辑代码、读代码、改文档

---

## Required Skills

**从** `.skills/` 自动链接到 `.kiro/skills/`：

### Shared (通用)

- `systematic-debugging.md`

- `test-driven-development.md`

- `using-superpowers.md`

- `verification-before-completion.md`

- `writing-plans.md`

- `requesting-code-review.md`

- `release-workflow.md`

### Backend Specific

（暂无，未来添加 Rust/Tauri/Windows API 相关 skills）

---

## 切换到此角色

```bash
./scripts/sync-skills.sh <your-cli> backend
```