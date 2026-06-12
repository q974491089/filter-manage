# Learnings Log

记录项目开发中的学习经验、纠正和最佳实践。

---

## [LRN-20260610-001] codegraph_usage

**Logged**: 2026-06-10T00:00:00Z
**Priority**: high
**Status**: promoted
**Area**: frontend

**Promoted**: AGENTS.md

### Summary
查询代码时必须优先使用 codegraph 工具，而不是直接用 read/grep

### Details
用户指出我在回答关于"配置同步机制"的问题时，直接使用了 `grep` 搜索关键词再用 `read` 读取文件片段，而没有按照 AGENTS.md 规范优先使用 codegraph 系列工具。

根据项目规范：
```
代码导航优先级：codegraph > code（AST） > read/grep
```

我犯了以下错误：
1. 惯性思维 - 简单的文件读取用 read/grep 更直观，下意识就用了
2. 没有严格自律 - 知道规范但没有强制执行
3. 对 codegraph 场景判断不足 - 理解"两个组件如何同步配置"这种问题，用 codegraph 看调用关系和数据流会更清晰

### Suggested Action
每次编码前，先阅读 `.claude/rules/` 目录下的规则文件，特别是 `codegraph.md`，然后严格遵守。

关键规则：
- `codegraph_explore` 是首选工具，大多数情况下是唯一需要的调用
- 不要用 grep 重新验证 codegraph 的结果
- 不要先 grep 再查 symbol
- 直接回答，不要委托探索
- WSL 环境下用 `cmd.exe /c codegraph.CMD` 调用 CLI
- MCP 工具调用失败时降级用 CLI

### Resolution
- **Resolved**: 2026-06-11T01:00:00Z
- **Notes**: 已将 WSL 环境下调用方式补充到 `.claude/rules/codegraph.md`

### Metadata
- Source: user_feedback
- Related Files: AGENTS.md
- Tags: codegraph, code-navigation, best-practice
- See Also: 无

---
