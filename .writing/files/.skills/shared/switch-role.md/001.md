# Agent 角色切换

当用户要求切换角色时使用此 skill。

---

## 何时使用

用户说以下内容时：
- "切换到后端角色"
- "变成前端 agent"
- "我想让你只负责前端"
- "切换到全栈模式"
- "switch to backend role"
- "become a frontend agent"

---

## 当前可用角色

| 角色 | 说明 | 代码范围 |
|------|------|---------|
| **universal** | 全栈（前端+后端都可以改） | `src/` + `src-tauri/` |
| **frontend** | 前端专属 | `src/`, `tailwind.config.js` |
| **backend** | 后端专属 | `src-tauri/`, `.docs/api/` |
| **devops** | 运维（未来） | `.github/workflows/` |

---

## 执行步骤

### 1. 确认目标角色

如果用户没有明确说要切换到哪个角色，询问：

```
你想切换到哪个角色？
- universal (全栈，前端+后端都可以改)
- frontend (只负责前端)
- backend (只负责后端)
```

### 2. 执行切换脚本

```bash
./scripts/sync-skills.sh claude <role>
```

**示例**：
```bash
# 切换到后端
./scripts/sync-skills.sh claude backend

# 切换到前端
./scripts/sync-skills.sh claude frontend

# 恢复全栈
./scripts/sync-skills.sh claude universal
```

### 3. 验证切换结果

```bash
# 查看当前 skills 数量
ls .claude/skills/ | wc -l

# 查看具体 skills
ls .claude/skills/
```

**预期结果**：
- **universal**: 8 个 skills (shared + frontend + backend)
- **frontend**: 8 个 skills (shared + frontend)
- **backend**: 7 个 skills (shared only)

### 4. 告知用户

切换成功后，告诉用户：

```
✅ 已切换到 <role> 角色

**新的职责范围**：
[从 .agent/<role>.md 读取并总结职责]

**Skills 已更新**：
[列出当前加载的 skills]

💡 提示：如需切换回其他角色，告诉我"切换到 <角色>"即可。
```

---

## 切换后的行为

### Universal 角色
- ✅ 可以修改前端代码（`src/`）
- ✅ 可以修改后端代码（`src-tauri/`）
- ✅ 负责文档同步

### Frontend 角色
- ✅ 可以修改前端代码（`src/`）
- ❌ **禁止修改后端代码**（`src-tauri/`）
- 🤝 需要后端支持时，创建 `.docs/handoff/<feature>-backend.md`

### Backend 角色
- ✅ 可以修改后端代码（`src-tauri/`）
- ❌ **禁止修改前端代码**（`src/`）
- 🤝 实现完成后，创建 `.docs/handoff/<feature>-frontend.md`
- 📝 必须同步更新 `.docs/api/` 文档

---

## 快速切换命令

用户可以直接说：
- "切换到后端" → 执行 `./scripts/sync-skills.sh claude backend`
- "切换到前端" → 执行 `./scripts/sync-skills.sh claude frontend`
- "恢复全栈" → 执行 `./scripts/sync-skills.sh claude universal`

---

## 注意事项

1. **不需要重启 CLI** - skills 通过 symlink 链接，立即生效
2. **角色信息在 `AGENTS.md`** - 切换后可查看 `AGENTS.md` 确认当前角色
3. **职责文档在 `.agent/`** - 读 `.agent/<role>.md` 了解详细职责
4. **统一规则仍然适用** - `.rules/` 目录下的规则所有角色都遵循

---

## 示例对话

**用户**: 切换到后端角色

**AI 执行**:
```bash
./scripts/sync-skills.sh claude backend
ls .claude/skills/ | wc -l
```

**AI 回复**:
```
✅ 已切换到 backend 角色

**新的职责范围**：
- ✅ 可以修改：src-tauri/, Cargo.toml, .docs/api/
- ❌ 禁止修改：src/, tailwind.config.js
- 📝 每次修改后端必须同步更新 API 文档

**当前加载的 Skills (7 个)**：
- systematic-debugging.md
- test-driven-development.md
- release-workflow.md
- requesting-code-review.md
- using-superpowers.md
- verification-before-completion.md
- writing-plans.md

💡 提示：如需切换回全栈模式，告诉我"恢复全栈"即可。
```

---

## 故障排除

### 脚本执行失败

```bash
# 检查脚本权限
ls -la scripts/sync-skills.sh

# 如果没有执行权限，添加权限
chmod +x scripts/sync-skills.sh

# 重新执行
./scripts/sync-skills.sh claude <role>
```

### Symlink 无法读取

```bash
# 检查 symlink 是否正确
ls -la .claude/skills/

# 如果有问题，重新运行同步脚本
./scripts/sync-skills.sh claude universal
```
