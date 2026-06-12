# 🎯 关键领悟：codegraph_context 已经足够

## 问题根源

之前的 instructions 太"温和"了：
```markdown
❌ 旧版：
"Use codegraph_context FIRST"
"Only use read if codegraph doesn't have what you need"
```

这给了 AI 借口去添加额外的 `read` 调用。

## 正确理解

**codegraph_context 返回的内容已经非常全面：**
- ✅ Entry points（入口函数、类）
- ✅ Related symbols（相关符号、导入）
- ✅ Complete code snippets（完整代码片段）
- ✅ File locations（文件位置）
- ✅ Call relationships（调用关系）

**99% 的代码分析任务，这已经足够了！**

---

## 实际对比

### Qoder（完美示例）
```
用户："分析后端架构"

▪ codegraph_context("Tauri backend - commands, state, modules")
  └ 返回完整架构（入口点 + 模块 + 代码 + 关系）

▪ 分析完毕 ✅

工具调用：1 次
结果：完整、准确
```

### OpenCode（之前的问题）
```
用户："分析后端架构"

▪ codegraph_context(...)
  └ 返回完整架构

Thinking: "让我再读一下主文件确认..."  ← ❌ 不必要
▪ Read(lib.rs)

Thinking: "让我再看看其他模块..."      ← ❌ 不必要
▪ Read(nvidia.rs)

工具调用：3+ 次
结果：相同，但浪费了 2+ 次调用
```

---

## 新的 Instructions 策略

### 更强硬的表述

```markdown
✅ 新版：
"codegraph_context is usually SUFFICIENT"
"Do NOT add extra read/search calls"
"Do NOT call read after codegraph_context unless you have a specific reason"
```

### 明确的决策树

```
codegraph_context 返回信息
  ↓
足够吗？ → 是（99%）
  ↓
立即回答。停止。不需要更多工具。
```

### 倾听 codegraph 的建议

当 codegraph_context 返回说：
```
"This project is small. The entry points and code above cover 
the relevant surface — do NOT call codegraph_explore"
```

**这不仅是说不要用 codegraph_explore，更是在说：**
- ✅ 你已有所需的一切
- ❌ 不要 read
- ❌ 不要 search
- ✅ 直接回答

---

## 唯一的例外：UI 审查

**只有在明确的 UI/设计审查任务时才用 read：**

```
用户："检查进程监听器的 UI 设计"

▪ codegraph_search("ProcessMonitor") 或 search
▪ read(src/components/SettingsModal.tsx)  ← 仅此例外
▪ [审查完整的 UI 标记和设计]
```

**为什么？**
- UI 审查需要看完整的 JSX/HTML 标记
- 需要看组件的布局结构
- codegraph_node 可能截断大型组件

---

## 核心原则

### 1. codegraph_context 是终点，不是起点

```
❌ 错误思维：
codegraph_context → "好的开始，再看看具体文件"

✅ 正确思维：
codegraph_context → "已经足够，直接回答"
```

### 2. 信任 codegraph 的输出

codegraph_context 不是"概览"，而是：
- 完整的架构图
- 关键代码片段
- 调用关系
- 文件位置

**它已经是完整答案，不是线索。**

### 3. 额外调用 = 浪费

```
codegraph_context 后的每个 read/search：
- 浪费时间
- 浪费 tokens
- 浪费 API 调用
- 结果相同或更差（碎片化）
```

---

## 学习 Qoder 的智慧

Qoder 为什么用 1 次调用就完成？

**不是因为 Qoder 更聪明，而是因为它信任 codegraph_context 的输出。**

OpenCode 应该学习这一点：
1. 调用 codegraph_context
2. 读取返回的完整信息
3. 直接回答
4. 停止

不要"再确认"，不要"再看看"，不要"补充一下"。

---

## 总结

**核心领悟**：
> codegraph_context 不是"第一步"，而是"唯一步"（对于代码理解任务）

**新的心态**：
- ❌ "codegraph 给了概览，让我深入看看" → 错误
- ✅ "codegraph 给了完整答案，直接用" → 正确

**预期效果**：
- OpenCode 和 Qoder 一样，用 1 次工具调用完成代码分析
- 不再有多余的 read/search
- 不再有无限循环

---

## 文件更新

- ✅ `.opencode/instructions.md` - 重写，更强硬的表述
- ✅ `.qoder/instructions.md` - 同步更新
- ✅ 本文档 - 记录关键领悟

**重启 CLI 后生效**。
