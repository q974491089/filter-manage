# Multi-Agent Framework 迁移检查清单

**版本**: v1.0  
**日期**: 2026-06-11

---

## ✅ 已完成的工作

- [x] 创建 `.agent/` 目录和角色文档
- [x] 创建 `.skills/` 统一 skill 池
- [x] 创建 `.rules/` 统一规则
- [x] 创建 `AGENTS.md` 注册表
- [x] 创建 `scripts/sync-skills.sh` 自动化脚本
- [x] 更新 `CLAUDE.md` 入口
- [x] 为 Claude Code 配置 skills symlinks
- [x] 为 Kiro CLI 配置 skills symlinks
- [x] 备份旧文件到 `.backup/20260611-multi-agent-framework/`
- [x] 添加 `.docs/agents.md` 废弃声明

---

## 📋 下一步操作

### 立即验证（必须）

- [ ] **测试角色切换**：
  ```bash
  ./scripts/sync-skills.sh claude backend
  ls -la .claude/skills/  # 应该只有 7 个 shared skills
  
  ./scripts/sync-skills.sh claude universal
  ls -la .claude/skills/  # 应该恢复到 8 个 skills
  ```

- [ ] **验证 symlink 正常**：
  ```bash
  cat .claude/skills/systematic-debugging.md  # 应该能正常读取
  cat .kiro/skills/release-workflow.md        # 应该能正常读取
  ```

- [ ] **测试协作流程**（可选）：
  - 让 Frontend Agent 创建 handoff 文档
  - 让 Backend Agent 更新文档并写入 `SYNC_STATUS.md`

### 配置其他 CLI（如果使用）

- [ ] **OpenCode**（Xiaomi 模型 - Frontend）：
  - 创建 `.opencode/README.md`（指向 `AGENTS.md`）
  - 运行 `./scripts/sync-skills.sh opencode frontend`
  - 在 `AGENTS.md` 确认配置

- [ ] **QoderCLI**（Qwen 模型 - Backend）：
  - 确认配置目录位置（`~/.qoder-cn/` 或项目内）
  - 如果需要项目配置，创建 `.qoder/README.md`
  - 运行 `./scripts/sync-skills.sh qoder backend`（如果支持）

### 清理工作（确认稳定后）

- [ ] **删除旧文件**（1-2 个版本后）：
  ```bash
  rm .docs/agents.md
  rm .kiro/backend-agent-constraints.md
  ```

- [ ] **删除备份**（确认无需回滚）：
  ```bash
  rm -rf .backup/20260611-multi-agent-framework/
  ```

---

## 🚨 常见问题排查

### Symlink 无法读取

**问题**: `cat .claude/skills/xxx.md` 报错 "No such file or directory"

**解决**:
```bash
# 检查 symlink 是否正确
ls -la .claude/skills/

# 重新运行同步脚本
./scripts/sync-skills.sh claude universal
```

### 角色切换不生效

**问题**: 切换角色后，CLI 仍读取旧的 skills

**解决**:
1. 重启 CLI（关闭并重新打开）
2. 检查 `.claude/skills/` 目录是否更新
3. 检查 `CLAUDE.md` 是否指向 `AGENTS.md`

### 找不到职责文档

**问题**: Agent 无法找到自己的职责文档

**解决**:
1. 检查 `AGENTS.md` 中的映射表
2. 确认 `.agent/<role>.md` 文件存在
3. 检查入口文件（`CLAUDE.md`）是否指向 `AGENTS.md`

---

## 📊 文件清单

### 新增文件

- `AGENTS.md` - Agent 注册表
- `.agent/README.md` - 框架说明
- `.agent/universal.md` - 全栈角色
- `.agent/frontend.md` - 前端角色
- `.agent/backend.md` - 后端角色
- `.agent/IMPLEMENTATION_REPORT.md` - 实施报告
- `.skills/README.md` - Skills 池说明
- `.skills/shared/` - 7 个通用 skills（从 `.kiro/skills/` 移动）
- `.skills/frontend/` - 1 个前端 skill
- `.skills/backend/` - 空目录
- `.rules/README.md` - 规则说明
- `.rules/tools.md` - 工具使用规范
- `.rules/docs.md` - 文档同步规范
- `.rules/handoff.md` - Agent 交接格式
- `.rules/git.md` - Git 规范
- `scripts/sync-skills.sh` - 角色切换脚本
- `.backup/20260611-multi-agent-framework/` - 备份目录
- `MIGRATION_CHECKLIST.md` - 本文件

### 修改文件

- `CLAUDE.md` - 添加框架入口说明

### 创建的 Symlinks

- `.claude/skills/*.md` → `../../.skills/shared/` 或 `../../.skills/frontend/`
- `.kiro/skills/*.md` → `../../.skills/shared/`

### 废弃文件（保留但添加了声明）

- `.docs/agents.md` - 指向 `.agent/frontend.md`

### 备份文件

- `.backup/20260611-multi-agent-framework/AGENTS.old.md` - 旧的根目录 AGENTS.md
- `.backup/20260611-multi-agent-framework/agents.md` - 旧的 .docs/agents.md
- `.backup/20260611-multi-agent-framework/backend-agent-constraints.md` - 旧的 Kiro 配置
- `.backup/20260611-multi-agent-framework/rules/` - 旧的 Claude rules

---

## ✨ 验证完成标准

框架实施成功的标志：

1. ✅ Claude Code 能读取 `AGENTS.md` 并找到自己的角色
2. ✅ `.claude/skills/` 目录包含 8 个 symlinks（universal 角色）
3. ✅ `.kiro/skills/` 目录包含 7 个 symlinks（backend 角色）
4. ✅ 运行 `./scripts/sync-skills.sh claude backend` 能成功切换
5. ✅ 切换后 `.claude/skills/` 只剩 7 个 symlinks（不包含 frontend）
6. ✅ 切换回 `./scripts/sync-skills.sh claude universal` 恢复 8 个
7. ✅ 所有 symlinks 都能正常读取（`cat .claude/skills/xxx.md` 无报错）

---

**完成以上检查后，框架即可投入使用！** 🎉
