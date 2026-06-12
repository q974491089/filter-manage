# Skills 池

统一的 skill 库，所有 AI CLI 共享。

## 目录结构

```
.skills/
├── shared/          # 通用 skills（所有角色）
├── frontend/        # 前端专属 skills
└── backend/         # 后端专属 skills
```

## 使用方式

各 CLI 通过 **symlink** 链接需要的 skills：

- **Claude Code** (`.claude/skills/`) - 链接 `shared/` + 当前角色的 skills
- **Kiro CLI** (`.kiro/skills/`) - 链接 `shared/` + 当前角色的 skills
- **OpenCode** - 链接 `shared/` + `frontend/`
- **QoderCLI** - 链接 `shared/` + `backend/`

## 角色切换

用脚本自动重建 symlink：

```bash
# 让 Claude Code 切换到后端角色
./scripts/sync-skills.sh claude backend

# 恢复前端角色
./scripts/sync-skills.sh claude frontend
```

## Skill 分类规则

- **shared/** - 编程通用技能（debugging, TDD, code review, release 等）
- **frontend/** - 前端特定（React, Tailwind, Stitch, UI 组件等）
- **backend/** - 后端特定（Rust, Tauri, Windows API, 系统集成等）

新增 skill 时，根据适用范围放入对应目录。
