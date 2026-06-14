# Multi-Agent Collaboration Framework

**统一的 Agent 注册表** - 所有 AI CLI 的入口。

---

## 当前 Agent 分配

| CLI | 模型 | 当前角色 | 职责文档 | Skills |
|-----|------|---------|---------|--------|
| **Claude Code** (`claude`) | Claude Opus 4.8 | **Universal** (全栈) | `.agent/universal.md` | `shared/` + `frontend/` + `backend/` |
| **OpenCode** | Xiaomi | **Frontend** | `.agent/frontend.md` | `shared/` + `frontend/` |
| **QoderCLI** (`qoder`) | Qwen | **Backend** | `.agent/backend.md` | `shared/` + `backend/` |
| **Kiro CLI** (`kiro`) | - | （暂未使用） | - | - |

**注**：Codex（未来可能负责服务端）

---

## 工作流程

```
1. 你的 CLI 启动
   ↓
2. 读入口文件（CLAUDE.md / .kiro/README.md 等）
   ↓
3. 入口指示："读 AGENTS.md 找到你的角色"
   ↓
4. 在此文件找到你的 CLI → 看"职责文档"列
   ↓
5. 读职责文档（如 .agent/frontend.md）
   ↓
6. 遵循统一规则（.rules/*.md）
   ↓
7. 加载 skills（通过 symlink 自动链接）
```

---

## 角色定义

### Universal（全栈）
- **代码范围**：前端 + 后端都可以修改
- **适用场景**：单人开发、快速原型、跨层功能
- **文档**：`.agent/universal.md`

### Frontend（前端）
- **代码范围**：`src/`, `tailwind.config.js`, `package.json`
- **禁止修改**：`src-tauri/`, `Cargo.toml`
- **文档**：`.agent/frontend.md`

### Backend（后端）
- **代码范围**：`src-tauri/`, `Cargo.toml`, `.docs/api/`
- **禁止修改**：`src/`, `tailwind.config.js`
- **文档**：`.agent/backend.md`

### DevOps（运维）
- **代码范围**：`.github/workflows/`, `Dockerfile`
- **文档**：`.agent/devops.md`（待创建）

---

## 共享规则

**所有 Agent 必须遵循**：

| 规则 | 文件 | 说明 |
|------|------|------|
| **工具使用** | `.rules/tools.md` | CodeGraph, ctx7, Tavily 使用规范 |
| **文档同步** | `.rules/docs.md` | API 文档、迭代记录、同步信号 |
| **交接格式** | `.rules/handoff.md` | Agent 间交接文档模板 |
| **Git 规范** | `.rules/git.md` | Commit message 格式、分支策略 |

---

## 协作机制

### Frontend ↔ Backend

**Frontend 需要后端支持**：
1. 创建 `.docs/handoff/<feature>-backend.md`
2. Backend Agent 实现功能
3. Backend 更新 `.docs/api/<module>.md`
4. Backend 创建 `.docs/handoff/<feature>-frontend.md`
5. Backend 写入 `SYNC_STATUS.md` 完成信号
6. Frontend 看到信号，重新读取文档

**Backend API 变更**：
1. 修改代码
2. 更新 `.docs/api/<module>.md`
3. 更新 `.docs/README.md` 迭代记录
4. 写入 `SYNC_STATUS.md` 完成信号
5. 必要时创建交接文档

---

## 角色切换

**让某个 CLI 切换到不同角色**：

```bash
# 语法
./scripts/sync-skills.sh <cli-name> <role>

# 示例：让 Claude Code 切换到后端角色
./scripts/sync-skills.sh claude backend

# 示例：恢复全栈角色
./scripts/sync-skills.sh claude universal

# 示例：让 Kiro CLI 接管前端
./scripts/sync-skills.sh kiro frontend
```

**原理**：
- 脚本读取此文件的映射表
- 清理旧的 skill symlinks
- 重建新角色需要的 symlinks
- CLI 重启后自动加载新角色

---

## Skill 自动链接

各角色通过 **symlink** 自动加载需要的 skills：

| 角色 | 链接的 Skills |
|------|--------------|
| **Universal** | `shared/` + `frontend/` + `backend/`（全部） |
| **Frontend** | `shared/` + `frontend/` |
| **Backend** | `shared/` + `backend/` |
| **DevOps** | `shared/` + `devops/`（待创建） |

**Skills 源文件**在 `.skills/` 目录，各 CLI 的 `skills/` 目录只是 symlink。

---

## 添加新 CLI

1. 确定角色（frontend / backend / universal / devops）
2. 在此文件的"当前 Agent 分配"表中添加一行
3. 在 `.claude/`, `.kiro/` 等目录类比，创建 `.<cli-name>/` 目录
4. 创建入口文件（如 `.newcli/README.md`），指示读 `AGENTS.md`
5. 运行 `./scripts/sync-skills.sh <cli-name> <role>` 链接 skills

---

## 添加新角色

1. 在 `.agent/` 创建 `<role>.md` 文档
2. （可选）在 `.skills/<role>/` 创建角色专属 skills
3. 在此文件的"角色定义"章节添加说明
4. 更新 `scripts/sync-skills.sh` 支持新角色

---

## 常见问题

### Q: 我怎么知道我是什么角色？

**A**: 读你的 CLI 入口文件（`CLAUDE.md` / `.kiro/README.md`），会指向 `AGENTS.md`，然后在"当前 Agent 分配"表中找到你的 CLI。

### Q: 我想切换角色怎么办？

**A**: 让用户运行 `./scripts/sync-skills.sh <your-cli> <new-role>`，或者让用户直接修改此文件的"当前 Agent 分配"表 + 手动重建 symlinks。

### Q: 我需要修改其他角色的代码怎么办？

**A**: 
- 如果是协作需求：创建 `.docs/handoff/` 交接文档
- 如果是临时例外：告知用户这跨越了职责边界，询问是否确认
- 如果需要永久切换：让用户运行角色切换脚本

### Q: Skills 在哪？

**A**: 
- **源文件**：`.skills/shared/`, `.skills/frontend/`, `.skills/backend/`
- **你的 CLI 目录**：`.claude/skills/`, `.kiro/skills/` 等（symlink）

### Q: 我该读哪些规则？

**A**: **必读** `.rules/` 目录下所有文件：
- `tools.md` - 工具使用（codegraph, ctx7, tavily）
- `docs.md` - 文档同步
- `handoff.md` - Agent 交接
- `git.md` - Git 规范

---

## 框架说明

详见 `.agent/README.md` 和 `.agent/IMPLEMENTATION_REPORT.md`。
