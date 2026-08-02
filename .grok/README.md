# Grok (xAI) 入口

> **🚀 Multi-Agent Framework**
>
> 本项目采用多 Agent 协作框架。你的职责、代码范围、协作规则都在框架中定义。
>
> **请先读以下文件（按顺序）**：
> 1. **`AGENTS.md`** - Agent 注册表，找到你的角色
> 2. **`.agent/<your-role>.md`** - 你的详细职责文档
> 3. **`.rules/*.md`** - 统一工具规则（必读）
>
> 当前 Grok 的角色：**Universal (全栈)**
> 详见：`.agent/universal.md`

---

## 角色

| 项 | 值 |
|----|-----|
| CLI | `grok` |
| 角色 | **Universal（全栈）** |
| 职责文档 | [`.agent/universal.md`](../.agent/universal.md) |
| Skills 目录 | [`.grok/skills/`](./skills/)（symlink → `.skills/`） |

## 代码范围

- ✅ **前端**: `src/`, `tailwind.config.js`, `package.json`, `vite.config.ts`
- ✅ **后端**: `src-tauri/`, `Cargo.toml`, `Cargo.lock`
- ✅ **文档**: `.docs/`, `docs/`, `README.md`
- ⚠️ **CI/CD**: `.github/workflows/` 谨慎修改

## 必读规则

| 规则 | 文件 |
|------|------|
| 工具使用（CodeGraph 优先） | `.rules/tools.md` |
| 文档同步 | `.rules/docs.md` |
| Agent 交接 | `.rules/handoff.md` |
| Git 规范 | `.rules/git.md` |
| 子 Agent 分工 | `.rules/subagent-dispatch.md` |

## 切换角色

```bash
./scripts/sync-skills.sh grok universal   # 全栈（当前）
./scripts/sync-skills.sh grok frontend    # 仅前端
./scripts/sync-skills.sh grok backend     # 仅后端
```

## Skills

源文件在 `.skills/`；本目录 `skills/` 由 `sync-skills.sh` 按角色 symlink 生成。

Grok 会自动发现：

- 项目级：`.grok/skills/`、根目录 `AGENTS.md` / `CLAUDE.md`
- Claude 兼容：`.claude/skills/`、`.claude/rules/`（默认开启）
- 用户级：`~/.grok/skills/`

项目角色 skills 以 **`.grok/skills/`** 为准。
