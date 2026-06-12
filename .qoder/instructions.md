# Qoder Agent Instructions

## 🚨 绝对规则：codegraph_explore 后不要 Read

**CRITICAL: codegraph_explore 返回的就是完整源码，等同于 Read。不要重复读取！**

### 当你看到这些消息时

#### ✅ "Complete source for N files is included above — do NOT re-read them"
→ **遵守指示，不要 Read！**

#### ⚠️ "Some file sections were trimmed for size"
→ **不要 Read！用另一次 `codegraph_explore` 或 `codegraph_node` 获取具体符号**

**正确做法**：
```
❌ 错误：
codegraph_explore(...) 
  → 看到 "trimmed"
  → Read(file.rs)  # 不要这样做！

✅ 正确：
codegraph_explore("backend architecture")
  → 看到 "trimmed"
  → codegraph_explore("specific_function_name")  # 如果确实需要
  → 或直接用已有信息回答
```

---

## 🎯 Tool Usage Priority - CodeGraph First and Only

**关键原则：`codegraph_explore` 通常已经足够。不要添加额外的 read/search 调用。**

### Primary: CodeGraph MCP

**官方指定：PRIMARY TOOL**

对于**任何**代码相关问题，使用 `codegraph_explore` **一次**即可：
- ✅ **架构分析** → `codegraph_explore` → 完成
- ✅ **模块理解** → `codegraph_explore` → 完成
- ✅ **流程追踪** → `codegraph_explore` → 完成

**核心认知**：codegraph_explore 返回：
- 入口点（函数、类）
- 相关符号（导入、调用）
- **完整代码片段**（等同于 Read）
- 调用关系
- 文件位置

**这通常 100% 足够。除非有特定原因，否则不要在 codegraph_explore 后调用 read/search。**

### 何时使用 read（非常罕见）

**仅在以下情况使用 `read`：**
1. **审查完整 UI 组件**（如"检查 UI 设计"）- 需要完整标记
2. **读取非代码文件**（如 package.json, README.md）
3. **codegraph_explore 明确说项目未索引**

**不要使用 read：**
- ❌ 在 codegraph_explore 后"再确认一下"
- ❌ "让我读一下主文件" - codegraph 已经显示了
- ❌ "为了更好理解" - codegraph_explore 已经很全面了
- ❌ **看到 "Some file sections were trimmed" - 用另一次 codegraph_explore，不是 Read**
- ❌ **看到 "do NOT re-read them" - 严格遵守，不要 Read**

### 何时使用 codegraph_files（罕见）

**仅在以下情况使用：**
- 需要看完整目录结构（但不需要代码内容）
- 统计文件数量、语言分布

**不要在 codegraph_explore 前先调用 codegraph_files** - 直接用 explore 更高效。

---

## ✅ 完美工作流（官方模式）

```
用户："分析后端架构"

Thinking: 使用 codegraph_explore 进行全面分析
▪ codegraph_explore("Tauri backend architecture - commands, state, modules")
  └ 返回：入口点 + 模块 + 代码 + 关系
  
  输出可能包含："Some file sections were trimmed"
  
Thinking: 已有足够信息回答（trimmed 的部分不影响架构分析）
▪ [提供架构分析] ✅

总计：1 次工具调用
```

---

## ❌ 反模式（严格避免）

```
用户："分析后端架构"

▪ codegraph_files(...)              ← ❌ 不必要
  └ 返回文件列表

▪ codegraph_explore(...) 
  └ 返回代码 + "Some file sections were trimmed"

Thinking: "trimmed" 说明需要完整文件  ← ❌ 错误理解！
▪ Read(lib.rs)                      ← ❌ 违反规则！
▪ Read(nvidia.rs)                   ← ❌ 违反规则！
▪ Read(icc.rs)                      ← ❌ 违反规则！

总计：7 次工具调用（6 次浪费）
```

**正确理解 "trimmed"**：
- ✅ 说明大文件被智能截断（只保留相关部分）
- ✅ 如果确实需要某个具体符号，用另一次 `codegraph_explore("symbol_name")`
- ❌ **不是**让你用 Read 读整个文件

---

## 🎯 决策树

```
任务：代码理解
  ↓
调用：codegraph_explore(任务描述)
  ↓
返回了信息，可能有 "trimmed" 消息
  ↓
问：我需要某个具体符号的完整代码吗？
  ├─ 是 → codegraph_explore("specific_symbol")
  └─ 否 → 直接回答
  ↓
✅ 停止。不要 Read。
```

---

## 📊 工具选择指南

| 任务 | 工具 | 原因 |
|------|------|------|
| 分析架构 | `codegraph_explore` | PRIMARY TOOL，一次调用 |
| 追踪流程 | `codegraph_explore` | 包含调用路径 |
| 查找符号 | `codegraph_search` | 只返回位置 |
| **需要特定符号** | `codegraph_explore("symbol")` | **不是 Read** |
| **大文件被 trimmed** | `codegraph_explore("symbol")` | **不是 Read** |
| UI 审查 | `read` | 获取完整标记 |
| 非代码文件 | `read` | package.json, README 等 |
| 目录结构 | `codegraph_files` | 不需要代码内容时 |

---

## 📋 总结

1. **codegraph_explore 等同于 Read** - 它返回源码
2. **看到 "trimmed" 不要 Read** - 用另一次 explore
3. **看到 "do NOT re-read" 必须遵守** - 严格不要 Read
4. **一次 explore 通常够用** - 直接回答
5. **Read 仅用于 UI 审查或非代码文件** - 代码永远用 codegraph

**官方来源**：codegraph 源码标记 explore 为 PRIMARY TOOL，并明确说 "Read-equivalent"。
