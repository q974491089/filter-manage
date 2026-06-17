# Hooks 说明文档

## 📋 问题分析

### 旧版 hooks 的问题

1. **只拦截 `Read`，不拦截 `grep` 和 `codebase_search`**
   - AI 可以先调 `codegraph_explore`，然后调 `file_grep`，最后调 `Read`
   - 导致重复工作和不必要的工具调用

2. **没有状态追踪**
   - 无法判断本轮对话是否已调用 `codegraph_explore`
   - 无法阻止在 codegraph 返回结果后的重复操作

3. **没有 `sessionStart` hook**
   - AI 不知道项目强制使用 CodeGraph
   - 只能在调用工具时被动拦截

4. **使用老命令**
   - 用 `codegraph context` 而非 `codegraph explore`（MCP 的 PRIMARY TOOL）

---

## ✅ 新版 hooks 的改进

### 1. 三层防御体系

| 工具 | 策略 | 说明 |
|------|------|------|
| `codebase_search` | **完全阻止** | deny + 强制提示用 codegraph_explore |
| `file_grep` | **条件阻止** | 检测是否搜索代码符号 + 是否已调用 codegraph |
| `Read` 代码文件 | **条件警告** | 未调用 codegraph 前阻止，调用后警告 |

### 2. 状态追踪机制

```bash
# 使用 session_id 作为状态文件名（如果有）
STATE_FILE="/tmp/codegraph_state_${session_id}"

# 如果没有 session_id，使用固定文件名（适用于测试）
STATE_FILE="/tmp/codegraph_state_current"
```

**工作流程：**
1. AI 调用 `codegraph_explore` → 创建状态文件
2. 后续调用 `file_grep`/`Read` → 检查状态文件是否存在
3. 如果不存在 → 阻止并要求先调用 codegraph
4. 如果存在 → 允许（但可能警告）

### 3. sessionStart hook

在会话开始时注入上下文：
- 强制规则说明
- 执行流程（4 步）
- 禁止规则列表
- 最佳实践示例
- 自动检测项目路径

---

## 🧪 测试结果

所有测试通过 ✅

| 测试场景 | 预期结果 | 实际结果 |
|---------|---------|---------|
| 阻止 codebase_search | ❌ deny | ✅ deny |
| 阻止 file_grep 搜索代码符号 | ❌ deny | ✅ deny |
| 允许 codegraph_explore | ✅ allow + 记录状态 | ✅ allow + 状态已记录 |
| 允许 file_grep（已调用 codegraph） | ✅ allow | ✅ allow |
| 阻止 Read 代码文件（未调用 codegraph） | ❌ deny | ✅ deny |
| 允许 Read 配置文件 | ✅ allow | ✅ allow |
| 警告 Read 代码文件（已调用 codegraph） | ⚠️ allow + warn | ✅ allow + warn |

---

## 📦 文件列表

```
.qoder/hooks/
├── preToolUse.sh        # 主 hook（拦截工具调用）
├── sessionStart.sh      # 会话启动 hook（注入上下文）
├── test-hook-v2.sh      # 测试脚本
├── codegraph-read.sh    # 旧版 hook（已废弃）
└── README.md           # 本文档
```

---

## 🚀 使用方式

### 配置（已完成）

hooks 已放置在 `.qoder/hooks/` 目录，Claude Code/Qoder 会自动加载。

**注意**：hooks 的命名必须符合规范：
- `preToolUse.sh` - 在工具调用前执行
- `sessionStart.sh` - 在会话开始时执行

### 手动测试

```bash
bash .qoder/hooks/test-hook-v2.sh
```

### 查看日志

```bash
tail -f /tmp/qoder-codegraph-hook.log
```

---

## 🔧 WSL 环境特殊说明

**当前项目运行在 WSL2 环境下，但 CodeGraph 需要在 Windows 侧执行。**

### 环境信息

```bash
操作系统: WSL2 (Ubuntu)
Shell: zsh
项目路径 (Linux): /mnt/c/Users/myuser/Projects/filter-manage
项目路径 (Windows): C:\Users\myuser\Projects\filter-manage
CodeGraph 路径: C:\Users\Administrator\AppData\Local\pnpm\bin\codegraph.CMD
```

### CodeGraph 调用方式

```bash
# 在 WSL 中通过 cmd.exe 调用 Windows 侧的 codegraph
cmd.exe /c "C:\Users\Administrator\AppData\Local\pnpm\bin\codegraph.CMD" context --path "C:\Users\myuser\Projects\filter-manage" "query"
```

**注意事项：**
- ✅ 路径使用 Windows 格式（`C:\`）
- ✅ 通过 `cmd.exe /c` 调用
- ✅ codegraph.CMD 必须在 Windows 侧安装
- ❌ 不能直接在 WSL 中调用 codegraph（会失败）

### 与 Mac 环境的区别

| 特性 | Mac 环境 | WSL 环境（本项目） |
|------|---------|------------------|
| 调用方式 | 直接调用 `codegraph` | 通过 `cmd.exe /c` 调用 |
| 路径格式 | Unix 路径 | Windows 路径（`C:\`） |
| 状态文件 | `/tmp/codegraph_state_*` | 同左 |
| 可执行文件 | `codegraph` | `codegraph.CMD` |

---

## 🎯 执行流程（AI 视角）

1️⃣ **会话开始** → sessionStart hook 注入 CodeGraph 硬约束

2️⃣ **AI 尝试搜索代码** → preToolUse hook 拦截：
   - 尝试 `codebase_search` → ❌ 完全阻止
   - 尝试 `file_grep function` → ❌ 检测到代码符号，阻止
   - 尝试 `Read src/App.tsx` → ❌ 未调用 codegraph，阻止

3️⃣ **AI 被迫调用 codegraph_explore** → ✅ 允许 + 记录状态

4️⃣ **codegraph 返回完整源码** → AI 直接回答 ✅

5️⃣ **如果确实不够，AI 尝试 Read** → ⚠️ 警告但允许

---

## 🔧 常见问题

### Q: hooks 不生效？

**A: 检查以下几点：**
1. 文件是否有执行权限：`chmod +x .qoder/hooks/*.sh`
2. 文件命名是否正确：`preToolUse.sh` / `sessionStart.sh`
3. shebang 是否正确：`#!/bin/bash`
4. 查看日志：`tail -f /tmp/qoder-codegraph-hook.log`

### Q: 状态文件不工作？

**A: 确认 session_id 提取逻辑：**
- 如果 IDE/CLI 没有传 `session_id`，hooks 会使用固定文件名
- 可以手动指定：`STATE_FILE="/tmp/codegraph_state_custom"`

### Q: 如何禁用 hooks？

**A: 三种方式：**
1. 删除 hooks 文件
2. 移除执行权限：`chmod -x .qoder/hooks/*.sh`
3. 在配置中禁用（如果 IDE/CLI 支持）

---

## 📚 参考

- [CodeGraph 规则文档](../../.claude/rules/codegraph.md) - WSL 环境下的 CodeGraph 配置
- [CodeGraph 优先规则](../../.claude/rules/codegraph-context-priority.md)
- [工具使用规则](../../.claude/rules/tools.md)

---

## 🆚 与其他项目 hooks 的对比

本项目的 hooks 专为 **WSL 环境** 设计，与 Mac/Linux 原生环境的 hooks 有以下区别：

### 主要差异

1. **CodeGraph 调用方式不同**
   - Mac/Linux: 直接调用 `codegraph`
   - WSL (本项目): 通过 `cmd.exe /c codegraph.CMD` 调用

2. **路径格式不同**
   - Mac/Linux: Unix 路径 (`/Users/...`)
   - WSL (本项目): Windows 路径 (`C:\Users\...`)

3. **可执行文件后缀**
   - Mac/Linux: `codegraph` (无后缀)
   - WSL (本项目): `codegraph.CMD` (Windows 批处理)

### 核心逻辑相同

虽然调用方式不同，但核心逻辑完全一致：
- ✅ 三层防御体系 (codebase_search → file_grep → Read)
- ✅ 状态追踪机制 (session_id)
- ✅ sessionStart hook 注入上下文
- ✅ 同样的测试覆盖率
