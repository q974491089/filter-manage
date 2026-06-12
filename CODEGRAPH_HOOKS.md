# CodeGraph 自动拦截 + 指令优化 - 快速参考

## 🎯 关键领悟

**codegraph_context 已经足够！不要添加额外的 read/search。**

Qoder 用 1 次工具调用完成分析，OpenCode 也应该如此：
```
codegraph_context → 完整答案 → 停止 ✅
```

不是：
```
codegraph_context → read → read → ... ❌
```

---

## ✅ 已完成配置

### Qoder CLI
- **配置**: `.qoder/settings.json` 
- **Hook**: `.qoder/hooks/codegraph-read.sh` - 拦截 read 工具（兜底）
- **指令**: `.qoder/instructions.md` - **codegraph_context 已足够，不要额外调用**
- **状态**: ✅ 已测试通过

### OpenCode CLI  
- **配置**: `.opencode/opencode.json`
- **Tool**: `.opencode/tools/read.ts` - 覆盖内置 read（兜底）
- **指令**: `.opencode/instructions.md` - **codegraph_context 已足够，不要额外调用**
- **状态**: ✅ 已配置

## 🎯 双层优化（已更新）

### 第一层：Instructions（行为引导）⭐ 核心

**关键更新**：更强硬的表述
- ✅ "codegraph_context is usually SUFFICIENT"
- ✅ "Do NOT add extra read/search calls"
- ✅ "Answer immediately from codegraph_context result"

**核心原则**：codegraph_context 是**终点**，不是起点

### 第二层：Hook/Custom Tool（透明拦截）🛡️ 兜底

当 AI 确实调用 read 时（如 UI 审查），自动使用 codegraph（兜底保障）

## 工作原理

```
AI 任务：分析代码
  ↓
Instructions 引导: codegraph_context 已足够
  ↓
codegraph_context("task description")
  └ 返回：入口点 + 模块 + 代码 + 关系（完整！）
  ↓
AI 直接回答 ✅（不再 read/search）
```

## ⚠️ 常见问题

### AI 还在 codegraph_context 后调用 read？

**原因**：旧的心态 - 把 codegraph 当"概览"
**解决**：新的 instructions 明确说 codegraph 已足够
**手动干预**：
```
codegraph_context 的结果已经完整，请直接回答，不要再 read
```

### AI 陷入无限循环？
- 症状：重复执行 `codegraph_node` 并说"输出被截断"
- 原因：大型组件内容超出限制
- 解决：手动告诉 AI "请用 read 工具读取文件"
- 详见：`.docs/codegraph-troubleshooting.md`

## 🧪 测试

```bash
# Qoder Hook 测试
.qoder/hooks/test-hook.sh

# 查看日志
tail -50 /tmp/qoder-codegraph-hook.log
```

## 📖 文档

- **快速参考**: 本文档
- **关键领悟**: `.docs/codegraph-key-insight.md` ⭐ 必读
- **详细配置**: `.docs/codegraph-auto-intercept.md`
- **效果对比**: `.docs/codegraph-optimization-comparison.md`
- **故障排查**: `.docs/codegraph-troubleshooting.md`
