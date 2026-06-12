# OpenCode vs Qoder CodeGraph 使用对比测试

**测试日期**: 2026-06-12  
**测试任务**: "分析 src-tauri 的后端架构"

---

## 🧪 测试结果

### Qoder CLI（基准）

**工具调用**: 1 次 ✅
```
▪ codegraph_context("Tauri backend architecture - commands, state, modules")
  └ 返回完整架构
▪ 分析完毕
```

**特点**:
- 完美执行
- 信任 codegraph_context 的输出
- 不添加额外调用

---

### OpenCode CLI

#### 第一次测试（优化前）

**工具调用**: 20+ 次 ❌

```
✅ codegraph_context (1)
❌ codegraph_search (16 次)
❌ codegraph_explore (1)
❌ codegraph_node (2+)
```

**问题**:
- 不信任第一次 codegraph_context 的结果
- 疯狂调用 codegraph_search 查找各种符号
- 完全没有遵循"一次就够"的原则

**配置**: `.opencode/instructions.md`（全局）未被 frontend agent 读取

---

#### 第二次测试（优化后）

**工具调用**: 5 次 ⚠️

```
⚙ codegraph_context (第 1 次)
⚙ codegraph_context (第 2 次) - "获取更多细节"
✱ Glob "src-tauri/**/*.rs" - "列出文件"
⚙ codegraph_context (第 3 次) - "确认模块"
→ Read(src-tauri/src/lib.rs) - "验证"
```

**改善**:
- ✅ 从 20+ 次降到 5 次（75% 改善）
- ✅ 不再疯狂 codegraph_search
- ⚠️ 但还是调用了 3 次 codegraph_context

**问题**:
- 仍然不够信任第一次结果
- 倾向于"再确认一下"、"补充一下"

**配置**: `.opencode/modes/frontend.md` 已更新为强硬规则

---

## 📊 对比总结

| 指标 | Qoder | OpenCode (优化前) | OpenCode (优化后) |
|------|-------|------------------|------------------|
| 工具调用次数 | 1 次 | 20+ 次 | 5 次 |
| 效率 | ⭐⭐⭐⭐⭐ | ⭐ | ⭐⭐⭐ |
| 遵循 instructions | ✅ 完美 | ❌ 完全不遵循 | ⚠️ 部分遵循 |
| 结果质量 | ✅ 完整准确 | ✅ 完整（但低效） | ✅ 完整（但低效） |

---

## 🤔 原因分析

### 为什么 Qoder 更好？

1. **更信任工具** - 一次调用就相信结果
2. **更严格执行 instructions** - 说"一次就够"就真的只调一次
3. **更优的模型行为** - 可能使用了不同的模型或参数

### 为什么 OpenCode 还在多次调用？

1. **谨慎性** - 模型倾向于"确认"和"补充"
2. **Temperature** - `0.7` 可能导致更多探索性行为
3. **Instructions 解析** - 可能没有像 Qoder 那样严格解析"STOP"指令

---

## 💡 建议

### 短期方案

**使用 Qoder 做代码分析**（已经完美）:
```bash
qoder
"分析后端架构"
# 1 次调用，完美结果 ✅
```

**OpenCode 用于其他任务**:
- 前端开发
- UI 修改
- 已经知道要改什么文件的任务

### 长期优化

1. **降低 temperature**:
   ```yaml
   # .opencode/modes/frontend.md
   temperature: 0.0  # 更确定性
   ```

2. **等待 OpenCode 改进** - 这可能是 OpenCode 的特性
3. **反馈给 OpenCode 团队** - Qoder 的行为是更优的

---

## 🎯 结论

**Qoder 是代码分析的王者** ⭐

- 完美执行（1 次调用）
- 完全信任 codegraph_context
- 不添加额外调用

**OpenCode 有改善，但还不够完美** ⚠️

- 从 20+ 次降到 5 次（75% 改善）
- 但还是不如 Qoder 的 1 次调用
- 建议用于非代码分析任务

**配置已优化，但模型行为是瓶颈。**
