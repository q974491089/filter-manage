# Agent 职责框架

多 Agent 协作框架 - 通过文件约定实现职责隔离和动态切换。

## 核心理念

1. **声明式角色定义** - 每个角色一个文档（`frontend.md`, `backend.md` 等）
2. **CLI 无关** - 任何 AI CLI 都能通过读取 `AGENTS.md` 找到职责
3. **动态切换** - 通过脚本重建 symlink 实现角色流转
4. **规则共享** - 工具使用规范统一在 `.rules/` 目录

## 角色定义

| 角色 | 职责描述 | 文档 |
|------|---------|------|
| **Frontend** | React/TypeScript 前端开发 | `frontend.md` |
| **Backend** | Rust/Tauri 后端开发 | `backend.md` |
| **DevOps** | CI/CD、部署、发布 | `devops.md` |
| **Universal** | 全栈（前端+后端） | `universal.md` |

## 工作流

```
1. CLI 启动 → 读 CLAUDE.md / .kiro/README.md 等入口文件
2. 入口文件指示 → 读 AGENTS.md（Agent 注册表）
3. 找到当前角色 → 读 .agent/<role>.md（详细职责）
4. 遵循统一规则 → 读 .rules/*.md（工具、文档、Git 规范）
5. 加载 skills → 通过 symlink 自动加载角色所需 skills
```

## 添加新角色

1. 在 `.agent/` 创建 `<role>.md` 文档
2. 在 `AGENTS.md` 注册表中添加映射
3. 在 `.skills/<role>/` 创建角色专属 skills（可选）
4. 更新 `scripts/sync-skills.sh` 脚本支持新角色

## 切换角色

见 `AGENTS.md` 中的"角色切换"章节。
