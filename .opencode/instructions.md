# OpenCode Agent Instructions

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

## 🎯 Tool Usage - CodeGraph First and Only

**CRITICAL: For code understanding tasks, `codegraph_explore` is usually SUFFICIENT. Do NOT add extra read/search calls.**

### Primary Tool: codegraph_explore

**Official designation: PRIMARY TOOL**

For **any code-related question**, use `codegraph_explore` **ONCE** and answer from that:
- ✅ **Architecture analysis** → `codegraph_explore` with task description → DONE
- ✅ **Module understanding** → `codegraph_explore` → DONE
- ✅ **Code flow tracing** → `codegraph_explore` → DONE

**Key insight**: codegraph_explore returns:
- Entry points (functions, classes)
- Related symbols (imports, calls)
- **Complete code snippets (Read-equivalent)**
- Call relationships
- File locations

**This is usually 100% sufficient. Do NOT call read/search after codegraph_explore unless you have a specific reason.**

### When to use read (very rare cases)

**ONLY use `read` when:**
1. **Reviewing full UI component** (e.g., "check UI design") - need complete markup
2. **Reading non-code files** (e.g., package.json, README.md)
3. **codegraph_explore explicitly says the project is not indexed**

**Do NOT use read when:**
- ❌ After codegraph_explore "just to double-check"
- ❌ "Let me read the main file" - codegraph already showed it
- ❌ "To understand better" - codegraph_explore is comprehensive
- ❌ **Seeing "Some file sections were trimmed" - use another codegraph_explore, NOT Read**
- ❌ **Seeing "do NOT re-read them" - strictly obey, do NOT Read**

### When to use codegraph_files (rare)

**ONLY use when:**
- Need to see full directory structure (without code content)
- Count files or check language distribution

**Do NOT call codegraph_files before codegraph_explore** - explore is more efficient.

---

## ✅ Perfect Workflow (Official Pattern)

```
User: "Analyze the backend architecture"

Thinking: I'll use codegraph_explore for comprehensive analysis
▪ codegraph_explore("Tauri backend architecture - commands, state, modules")
  └ Returns: Entry points + modules + code + relationships
  
  May include: "Some file sections were trimmed"
  
Thinking: I have sufficient info (trimmed parts don't affect architecture analysis)
▪ [Provide full analysis] ✅

Total: 1 tool call
```

---

## ❌ Anti-Pattern (STRICTLY AVOID)

```
User: "Analyze the backend architecture"

▪ codegraph_files(...)              ← ❌ UNNECESSARY
  └ Returns file list

▪ codegraph_explore(...) 
  └ Returns code + "Some file sections were trimmed"

Thinking: "trimmed" means need full files  ← ❌ WRONG! 
▪ Read(lib.rs)                      ← ❌ VIOLATES RULES!
▪ Read(nvidia.rs)                   ← ❌ VIOLATES RULES!
▪ Read(icc.rs)                      ← ❌ VIOLATES RULES!

Total: 7 tool calls (6 wasted)
```

**Correct understanding of "trimmed"**:
- ✅ Large files intelligently truncated (keeps relevant parts)
- ✅ If you truly need a specific symbol, use another `codegraph_explore("symbol_name")`
- ❌ **NOT** telling you to Read the whole file

---

## 🎯 Decision Tree

```
Task: Code understanding
  ↓
Call: codegraph_explore(task description)
  ↓
Returns info, may have "trimmed" message
  ↓
Question: Do I need a specific symbol's full code?
  ├─ Yes → codegraph_explore("specific_symbol")
  └─ No → Answer directly
  ↓
✅ STOP. Do NOT Read.
```

---

## 📊 Tool Selection Guide

| Task | Tool | Reason |
|------|------|--------|
| Architecture analysis | `codegraph_explore` | PRIMARY TOOL, one call |
| Flow tracing | `codegraph_explore` | Includes call paths |
| Symbol search | `codegraph_search` | Returns locations only |
| **Need specific symbol** | `codegraph_explore("symbol")` | **NOT Read** |
| **File was trimmed** | `codegraph_explore("symbol")` | **NOT Read** |
| UI review | `read` | Get complete markup |
| Non-code files | `read` | package.json, README, etc |
| Directory structure | `codegraph_files` | When no code needed |

---

## 📋 Summary

1. **codegraph_explore is Read-equivalent** - it returns source code
2. **Seeing "trimmed" → don't Read** - use another explore
3. **Seeing "do NOT re-read" → must obey** - strictly no Read
4. **One explore usually sufficient** - answer directly
5. **Read only for UI review or non-code files** - code always uses codegraph

**Remember**: Official codegraph benchmark shows explore as PRIMARY TOOL with 1 call achieving best results.
