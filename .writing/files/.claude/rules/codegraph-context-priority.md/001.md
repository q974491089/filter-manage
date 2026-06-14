# 🎯 CodeGraph Explore 优先规则

**补充 `.rules/tools.md` 的规则。**

## 核心原则更新

**对于代码理解任务，`codegraph_explore` 是最强大的工具，通常只需 1 次调用。**

### 工具选择策略（更新）

| 意图 | 工具 | 备注 |
|------|------|------|
| **代码分析/架构理解/模块关系** | `codegraph_explore` | **PRIMARY TOOL，首选** |
| **查找符号位置** | `codegraph_search` | 只要位置，不要代码 |
| **追踪调用路径** | `codegraph_trace` | from → to 路径 |
| **查看单个符号详情** | `codegraph_node` | ⚠️ 大文件会截断 |

### codegraph_explore 的强大之处

**官方标注为 PRIMARY TOOL：**

```
PRIMARY TOOL — call FIRST for almost any question:
- How does X work
- Architecture
- Bug investigation
- Where/what is X
- Surveying an area

Returns verbatim source of relevant symbols grouped by file in ONE capped call.
Usually the ONLY call you need.
```

返回内容：
- ✅ Entry points（入口函数、类）
- ✅ Related symbols（相关符号）
- ✅ Complete code snippets（完整代码）
- ✅ Call relationships（调用关系）
- ✅ File locations（文件位置）

**这通常已经 100% 足够！**

### 决策流程

```
任务：理解代码
  ↓
第一选择：codegraph_explore(查询描述)
  ↓
返回完整信息？ → 是（99%）
  ↓
直接回答。停止。
  ↓
仅当不够（1%）：
  ↓
再用 codegraph_search / codegraph_trace / codegraph_node
```

### ⚠️ 重要：不要在 codegraph_explore 后添加 Read

**错误模式**：
```
❌ codegraph_explore(...)
   → "让我再读一下文件确认..."
   → Read(file.ts)  # 不必要！
```

**正确模式**：
```
✅ codegraph_explore(...)
   → 获得完整信息
   → 直接回答
```

**例外**：只有在 UI 审查（需要完整标记）或非代码文件时才用 Read。

---

## 实际示例

### 完美执行

```
用户："分析后端架构"

▪ codegraph_explore("Tauri backend architecture - commands, state, modules")
  └ 返回完整架构
  
▪ 直接回答 ✅

工具调用：1 次
```

### 避免的模式

```
用户："分析后端架构"

▪ codegraph_explore(...)
  └ 返回完整架构
  
▪ Read(lib.rs)      ← ❌ 不必要
▪ Read(other.rs)    ← ❌ 不必要

工具调用：3+ 次（浪费）
```

---

## 📋 与旧规则的变化

**之前说的 `codegraph_context`**：
- ❌ MCP 工具中不存在（或已废弃）
- ✅ 应该用 `codegraph_explore` 代替

**原因**：
- codegraph 源码显示 `codegraph_explore` 是 PRIMARY TOOL
- 官方 benchmark 用的就是 explore
- 它就是"一次调用就够"的那个工具

---

**总结**：`codegraph_explore` 是最强工具，信任它的输出，不要画蛇添足。
