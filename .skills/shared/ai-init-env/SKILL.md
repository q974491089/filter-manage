---
name: ai-init-env
description: Initialize multi-agent development environment for a new project. Copies agent definitions, rules, skills, hooks, and config templates from a source project.
version: 1.0.0
user-invocable: true
argument-hint: "[check|init] [source-project-path]"
---

# AI Init Environment

初始化多 Agent 开发环境，将 agent 框架（角色定义、规则、skills、hooks）快速部署到新项目。

---

## 何时使用

用户说以下内容时：
- "初始化 AI 环境"
- "设置 agent 框架"
- "init ai env"
- "把 agent 框架复制到新项目"
- "检查 AI 环境状态"
- "check ai env"

---

## 子命令

| 命令 | 说明 |
|------|------|
| `init` | 初始化新项目（默认） |
| `check` | 检查当前项目环境状态 |

---

## 执行流程

### 1. 解析参数

从用户输入中提取：
- **子命令**：`init` 或 `check`（默认 `init`）
- **源项目路径**：要复制框架的源项目（默认：当前 skill 所在项目）

### 2. 环境检测

运行环境检测脚本确定当前环境：

```bash
source .skills/shared/ai-init-env/scripts/env-detect.sh
detect_environment
```

输出示例：
- `wsl` - WSL2 环境
- `windows-msys` - Windows Git Bash
- `windows-ps` - Windows PowerShell
- `unix` - Linux/Mac

### 3. 执行子命令

#### `check` - 检查环境状态

运行检查脚本：

```bash
bash .skills/shared/ai-init-env/scripts/check.sh
```

输出检查结果：
```
🔍 AI Environment Status

项目: /path/to/project
环境: wsl

组件状态：
  ✅ .agent/ 目录存在 (3 个文件)
  ✅ .rules/ 目录存在 (4 个文件)
  ✅ .skills/ 目录存在
  ✅ Skills symlink 有效 (8 个链接)
  ❌ Hooks 目录缺失
  ✅ AGENTS.md 存在
  ✅ CLAUDE.md 存在

已安装 CLI：
  ✅ claude
  ✅ opencode
  ❌ qoder
  ❌ codex

建议：
  - 运行 `ai-init-env init` 补全缺失组件
```

#### `init` - 初始化新项目

**Step 1: 确认源项目**

如果用户未指定源项目路径，询问：
```
请输入源项目路径（agent 框架来源）：
> /mnt/c/Users/myuser/Projects/filter-manage
```

验证源项目包含必要的目录（`.agent/`, `.rules/`, `.skills/`）。

**Step 2: 交互式组件选择**

显示组件选择菜单：
```
🔧 AI Init Environment

源项目: /path/to/source
目标项目: /path/to/target
环境: wsl

请选择要初始化的组件（输入编号切换，回车确认）：

  1. [x] Agent 定义     - .agent/*.md（角色职责文档）
  2. [x] Rules          - .rules/*.md（统一规则）
  3. [x] Skills         - .skills/ + sync-skills.sh
  4. [x] Hooks          - 各 CLI 的 hooks 目录
  5. [x] 配置模板       - AGENTS.md, CLAUDE.md 等
  6. [ ] CodeGraph      - .codegraph/ 配置（可选）

目标 CLI（输入编号切换，回车确认）：

  A. [x] Claude Code    - .claude/
  B. [x] OpenCode       - .opencode/
  C. [ ] QoderCLI       - .qoder/
  D. [ ] Codex          - .codex/

确认初始化？[Y/n]
```

**Step 3: 执行初始化**

根据用户选择，执行以下操作：

1. **Agent 定义**
   ```bash
   # 复制 .agent/ 目录
   cp -r "$SOURCE/.agent" "$TARGET/.agent"
   ```

2. **Rules**
   ```bash
   # 复制 .rules/ 目录
   cp -r "$SOURCE/.rules" "$TARGET/.rules"
   ```

3. **Skills**
   ```bash
   # 复制 .skills/ 目录（源文件）
   cp -r "$SOURCE/.skills" "$TARGET/.skills"

   # 复制 sync-skills.sh 脚本
   mkdir -p "$TARGET/scripts"
   cp "$SOURCE/scripts/sync-skills.sh" "$TARGET/scripts/"
   chmod +x "$TARGET/scripts/sync-skills.sh"

   # 运行 sync-skills.sh 为每个 CLI 创建 symlink
   for cli in $SELECTED_CLIS; do
       "$TARGET/scripts/sync-skills.sh" "$cli" "universal"
   done
   ```

4. **Hooks**
   ```bash
   # 复制各 CLI 的 hooks 目录
   if [[ "$SELECTED_CLIS" == *"claude"* ]]; then
       mkdir -p "$TARGET/.claude/hooks"
       cp "$SOURCE/.claude/hooks/"* "$TARGET/.claude/hooks/" 2>/dev/null || true
   fi
   # ... 类似处理其他 CLI
   ```

5. **配置模板**
   ```bash
   # 复制 AGENTS.md（如果不存在）
   if [ ! -f "$TARGET/AGENTS.md" ]; then
       cp "$SOURCE/AGENTS.md" "$TARGET/"
   fi

   # 生成 CLAUDE.md（如果不存在）
   if [ ! -f "$TARGET/CLAUDE.md" ]; then
       # 基于模板生成，替换项目相关信息
       generate_claude_md "$TARGET"
   fi
   ```

**Step 4: 覆盖处理**

遇到已存在的文件时，询问用户：
```
⚠️ 文件已存在: .agent/universal.md

选项：
  [o] 覆盖 - 用源文件替换
  [s] 跳过 - 保留现有文件
  [b] 备份 - 备份后覆盖
  [a] 全部覆盖 - 后续文件不再询问
```

**Step 5: 验证结果**

```bash
# 运行 check 脚本验证
bash .skills/shared/ai-init-env/scripts/check.sh
```

**Step 6: 输出使用说明**

```
✅ AI 环境初始化完成！

已初始化组件：
  - .agent/ (3 个角色定义)
  - .rules/ (4 个规则文件)
  - .skills/ (8 个 skills + symlink)
  - AGENTS.md

下一步：
  1. 读取 AGENTS.md 了解多 Agent 框架
  2. 读取 .agent/universal.md 了解你的角色
  3. 读取 .rules/tools.md 了解工具使用规则
  4. 运行 `ai-init-env check` 验证环境状态

切换角色：
  ./scripts/sync-skills.sh claude frontend
  ./scripts/sync-skills.sh claude backend
  ./scripts/sync-skills.sh claude universal
```

---

## 环境差异处理

### Symlink 创建

根据检测到的环境，使用不同的 symlink 方式：

| 环境 | 命令 |
|------|------|
| WSL / Linux / Mac | `ln -s <target> <link>` |
| Windows (Git Bash) | `cmd.exe /c "mklink /J <target> <link>"` |
| Windows (PowerShell) | `New-Item -ItemType SymbolicLink -Path <link> -Target <target>` |

### 路径转换

WSL 环境需要路径转换：
```bash
# WSL → Windows 路径
wslpath -w "/mnt/c/Users/..."  # → "C:\Users\..."

# Windows → WSL 路径
wslpath -u "C:\Users\..."  # → "/mnt/c/Users/..."
```

---

## 注意事项

1. **源项目验证**：确保源项目包含完整的 agent 框架结构
2. **权限检查**：确保对目标项目有写权限
3. **Git 状态**：建议在初始化前提交当前更改
4. **备份提醒**：覆盖文件前提醒用户备份
5. **CLI 依赖**：某些 CLI（如 qoder）可能需要额外配置

---

## 示例对话

**用户**: 初始化 AI 环境

**AI 执行**:
```bash
# 检测环境
source .skills/shared/ai-init-env/scripts/env-detect.sh
detect_environment

# 显示交互式菜单
# ... 用户选择组件 ...

# 执行初始化
bash .skills/shared/ai-init-env/scripts/init.sh /path/to/source
```

**AI 回复**:
```
✅ AI 环境初始化完成！

已初始化组件：
  - .agent/ (3 个角色定义)
  - .rules/ (4 个规则文件)
  - .skills/ (8 个 skills)
  - AGENTS.md

下一步：读取 AGENTS.md 了解多 Agent 框架
```

---

## 故障排除

### 源项目不存在

```
❌ 错误：源项目路径不存在: /path/to/source
请检查路径是否正确
```

### 缺少必要目录

```
❌ 错误：源项目缺少必要目录: .agent/, .rules/, .skills/
请确保源项目已初始化多 Agent 框架
```

### Symlink 创建失败

```
❌ 错误：无法创建 symlink
可能原因：
  - 权限不足
  - 目标路径不存在
  - Windows 环境需要管理员权限

解决方案：
  - 以管理员身份运行
  - 检查路径是否存在
  - 使用 `mklink` 替代 `ln -s`
```

### CLI 未安装

```
⚠️ 警告：CLI 未安装: qoder
跳过该 CLI 的配置，可稍后手动配置
```
