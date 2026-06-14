# CodeGraph 配置更新总结

**更新日期**: 2026-06-12  
**原因**: 通过源码调研，确认 `codegraph_explore` 是官方 PRIMARY TOOL

---

## 🔍 调研发现

### 源码证据

从 codegraph 官方仓库（https://github.com/colbymchenry/codegraph）确认：

1. **MCP 工具定义** (`src/mcp/tools.ts`):
   ```typescript
   {
     name: 'codegraph_explore',
     description: 'PRIMARY TOOL — call FIRST for almost any question...'
   }
   ```

2. **工具列表**:
   - ✅ `codegraph_search`
   - ✅ `codegraph_callers`
   - ✅ `codegraph_callees`
   - ✅ `codegraph_impact`
   - ✅ `codegraph_node`
   - ✅ **`codegraph_explore`** ← PRIMARY TOOL
   - ✅ `codegraph_status`
   - ✅ `codegraph_files`
   - ❌ `codegraph_context` ← **不存在于 MCP**

3. **官方 Benchmark** (README):
   ```
   Re-validated on Opus 4.8 (2026-06-02), on the current build
   (codegraph_explore as the primary tool).
   
   Average: 16% cheaper · 47% fewer tokens · 22% faster · 58% fewer tool calls
   ```

### 关键结论

- **`codegraph_explore`** 是 MCP 的 PRIMARY TOOL
- **`codegraph_context`** 只是 CLI 命令（`codegraph context <task>`），不是 MCP 工具
- 之前的配置混淆了 CLI 命令和 MCP 工具

---

## ✅ 已更新的文件

### 1. Claude Code

| 文件 | 变更 | 状态 |
|------|------|------|
| `.claude/rules/codegraph-context-priority.md` | `context` → `explore` | ✅ 完成 |
| `~/.claude/projects/.../memory/feedback_tool_preferences.md` | 更新工具优先级 | ✅ 完成 |

### 2. OpenCode

| 文件 | 变更 | 状态 |
|------|------|------|
| `.opencode/instructions.md` | `context` → `explore` | ✅ 完成 |
| `.opencode/modes/frontend.md` | `context` → `explore` | ✅ 完成 |

### 3. Qoder

| 文件 | 变更 | 状态 |
|------|------|------|
| `.qoder/instructions.md` | `context` → `explore` | ✅ 完成 |

---

## 📋 配置对比

### 更新前

```
❌ codegraph_context (不存在的工具)
   ├── .claude/rules/ - 说 explore 首选
   ├── .opencode/ - 说 context 首选
   └── .qoder/ - 说 context 首选
```

### 更新后

```
✅ codegraph_explore (PRIMARY TOOL)
   ├── .claude/rules/ - explore
   ├── .opencode/ - explore
   └── .qoder/ - explore
```

---

## 🎯 正确用法

### 代码理解任务

```
任务："分析后端架构"

✅ 正确:
▪ codegraph_explore("Tauri backend - commands, state, modules")
  └ 返回完整信息
▪ 直接回答
总计：1 次调用

❌ 错误:
▪ codegraph_context(...)  ← 工具不存在
或
▪ codegraph_explore(...)
▪ Read(lib.rs)            ← 不必要
▪ Read(other.rs)          ← 不必要
总计：3+ 次调用
```

### 工具选择指南

| 任务 | 首选工具 | 备注 |
|------|---------|------|
| **代码架构分析** | `codegraph_explore` | PRIMARY TOOL |
| **流程追踪** | `codegraph_explore` | 包含动态分发路径 |
| **符号搜索** | `codegraph_search` | 只返回位置 |
| **调用关系** | `codegraph_callers` / `codegraph_callees` | 特定用途 |
| **影响分析** | `codegraph_impact` | 变更影响 |
| **UI 审查** | `Read` | 获取完整组件标记 |
| **非代码文件** | `Read` | package.json, README 等 |

---

## 📊 预期效果

### 基于官方 Benchmark

使用 `codegraph_explore` 作为 PRIMARY TOOL:
- ✅ **16% 更便宜**
- ✅ **47% 更少 tokens**
- ✅ **22% 更快**
- ✅ **58% 更少工具调用**

### 目标

所有 AI CLI (OpenCode, Qoder, Claude Code) 在代码分析任务中：
- 🎯 **1 次 `codegraph_explore` 调用**
- 🎯 **0 次额外 Read/Search 调用**
- 🎯 **直接从 explore 结果回答**

---

## 🔧 后续建议

### 1. 升级 codegraph

当前版本: `0.9.7`  
最新版本: `0.9.9`

```bash
cmd.exe /c "C:\Users\Administrator\AppData\Local\pnpm\bin\codegraph.CMD" upgrade
```

### 2. 验证更新

重新测试各个 AI CLI，确认都使用 `codegraph_explore`：

```bash
# OpenCode
opencode run "分析 src-tauri 的后端架构"
# 预期：1 次 codegraph_explore 调用

# Qoder (如果可用)
qoder
"分析后端架构"
# 预期：1 次 codegraph_explore 调用
```

### 3. 清理旧文档

删除或更新任何提到 `codegraph_context` 作为 MCP 工具的文档：
- ✅ 已更新所有配置文件
- ✅ 已更新 memory
- ⚠️ 可能还有其他文档需要检查

---

## 📚 参考资料

- **CodeGraph 官方仓库**: https://github.com/colbymchenry/codegraph
- **源码位置**: `src/mcp/tools.ts` - MCP 工具定义
- **Benchmark**: README.md - 官方性能测试结果
- **当前版本**: 0.9.7 (本地安装)
- **最新版本**: 0.9.9 (GitHub)

---

## ✅ 完成标记

- [x] 调研 codegraph 源码
- [x] 确认 `codegraph_explore` 是 PRIMARY TOOL
- [x] 更新 Claude Code 配置
- [x] 更新 OpenCode 配置
- [x] 更新 Qoder 配置
- [x] 更新 memory 文件
- [x] 创建总结文档
- [ ] 升级 codegraph 到最新版 (可选)
- [ ] 测试验证 (可选)

**所有配置已统一使用 `codegraph_explore` 作为 PRIMARY TOOL。** ✅
