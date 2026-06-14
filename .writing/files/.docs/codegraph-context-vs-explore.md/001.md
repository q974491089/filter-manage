# Codegraph 工具名称修正

## ❌ 发现问题

`.claude/rules/codegraph.md` 中大量提到 `codegraph_explore` 作为首选工具。

**但实际情况**：
- ❌ CLI 没有 `explore` 命令
- ✅ CLI 有 `context` 命令
- ✅ MCP 工具 `codegraph_context` 映射到 CLI `context`

## ✅ 正确的工具映射

| MCP 工具 | CLI 命令 | 用途 |
|---------|---------|------|
| `codegraph_context` | `context` | **构建任务上下文（首选）** |
| `codegraph_search` | `query` | 搜索符号 |
| `codegraph_callers` | `callers` | 查找调用者 |
| `codegraph_callees` | `callees` | 查找被调用者 |
| `codegraph_node` | ? | 查看单个符号详情 |
| `codegraph_impact` | `impact` | 影响分析 |
| `codegraph_explore` | ❌ **不存在** | 可能是 MCP 的别名或已废弃 |

## 🎯 实际最佳实践

**Qoder 的做法（证明有效）**：
```
任务："分析后端架构"
→ codegraph_context("Tauri backend - commands, state, modules")
→ 获得完整信息
→ 直接回答
→ 1 次调用完成 ✅
```

**规则文档说的（可能过时）**：
```
任务："分析后端架构"  
→ codegraph_explore(...) ← 这个工具可能不存在或者就是 context
```

## 📋 建议

1. **更新 `.claude/rules/codegraph.md`**
   - 把所有 `codegraph_explore` 改为 `codegraph_context`
   
2. **更新所有 agent instructions**
   - OpenCode, Qoder, Claude Code 都应该用 `codegraph_context`
   
3. **统一认知**
   - `codegraph_context` 就是首选工具
   - 它就是规则里说的"一次调用返回完整信息"的那个工具

---

**结论**：`codegraph_context` = 规则里说的 `codegraph_explore`，只是名字不同。
