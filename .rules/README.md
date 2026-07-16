# 统一规则

所有 Agent 必须遵循的规则文档。

## 文件列表

| 文件 | 说明 |
|------|------|
| `tools.md` | 工具使用规范（CodeGraph, ctx7, Tavily） |
| `docs.md` | 文档同步规范（API 文档、迭代记录） |
| `handoff.md` | Agent 交接文档格式 |
| `git.md` | Git commit 规范、分支策略 |
| `subagent-dispatch.md` | 跨端任务的子 Agent 分工（能力自适应，多 CLI 通用） |
| `archive.md` | 历史产物归档规则（归档区 `.docs/archive/`） |
| `coding.md` | **AI 编码规范**（零警告交付、Rust/前端质量、完成前验证） |

## 强制要求

**所有 Agent 在开始工作前必须读取这些规则。**

- 不遵守工具规范会导致效率低下（用错工具）
- 不遵守文档规范会导致协作混乱（前后端信息不同步）
- 不遵守交接规范会导致工作阻塞（不知道对方需要什么）
- 不遵守 Git 规范会导致提交历史混乱（无法追踪变更）
- 不遵守编码规范会留下 rustc 警告、死代码与前后端竞态（见 `coding.md`）
