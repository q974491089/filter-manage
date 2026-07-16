# Multi-Agent Collaboration Framework

**统一的 Agent 注册表** - 所有 AI CLI 的入口。

---

## 当前 Agent 分配

| CLI | 模型 | 当前角色 | 职责文档 | Skills |
|-----|------|---------|---------|--------|
| **Claude Code** (`claude`) | Claude Opus 4.8 | **Universal** (全栈) | `.agent/universal.md` | `shared/` + `frontend/` + `backend/` |
| **Codex** (`codex`) | GPT-5 | **Universal** (全栈) | `.agent/universal.md` | `shared/` + `frontend/` + `backend/` |
| **Grok** (`grok`) | Grok (xAI) | **Universal** (全栈) | `.agent/universal.md` | `shared/` + `frontend/` + `backend/` |
| **OpenCode** | Xiaomi | **Frontend** | `.agent/frontend.md` | `shared/` + `frontend/` |
| **QoderCLI** (`qoder`) | Qwen | **Universal**（临时切换） | `.agent/universal.md` | `shared/` + `frontend/` + `backend/` |
| **Kiro CLI** (`kiro`) | - | （暂未使用） | - | - |

**注**：Claude Code、Codex、Grok 共享 **Universal** 角色，拥有前端、后端和协作规则允许范围内的完整项目权限。

---

## 工作流程

```
1. 你的 CLI 启动
   ↓
2. 读入口文件（CLAUDE.md / .kiro/README.md / .grok/README.md 等）
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

## 项目目录约定

| 目录 | 用途 | 受众 | 是否提交 Git |
|------|------|------|-------------|
| `docs/` | **公开文档站**（VitePress）— changelog、安装指南、产品介绍等面向用户的内容 | 用户/访客 | 是 |
| `.docs/` | **Agent 内部文档** — API 文档、迭代记录、交接文档、架构说明等面向 Agent 的技术内容 | AI Agent | 是 |
| `.docs/prd/` | **PRD 产品需求文档** — 活跃功能的需求（`/prd` skill 产出，writing-plans 的输入） | AI Agent | 是 |
| `.docs/archive/` | **归档区** — 已完成的 plans/handoff/完工报告/同步日志，可追溯历史，**平时不读**，仅追溯背景时查 | AI Agent | 是 |
| `.skills/` | Skill 源文件池（shared/frontend/backend 分类，统一 `<name>/SKILL.md` 目录格式） | AI Agent | 是 |
| `.env.local` | 本地敏感配置（服务器、账号、密码、token 等） | 本地开发/Agent | **否**（gitignore） |
| `.env.example` | 本地敏感配置模板（只写占位符） | 全体 | 是 |
| `.writing/` | CLI 写作缓冲临时文件 | 临时 | **否**（gitignore） |
| `.codegraph/` | CodeGraph 运行时数据 | 临时 | **否**（gitignore） |

**关键区分**：
- **`docs/`** = 给人看的（公开文档站，自动部署，`CHANGELOG.md` 通过 `@include` 自动引用）
- **`.docs/`** = 给 Agent 看的（技术迭代记录、API 文档、交接文档，比 CHANGELOG 更详尽）
- 修改 API / 迭代记录时 → 改 `.docs/`
- 修改用户文档 / 安装指南时 → 改 `docs/`

---

## 共享规则

**所有 Agent 必须遵循**：

| 规则 | 文件 | 说明 |
|------|------|------|
| **工具使用** | `.rules/tools.md` | CodeGraph, ctx7, Tavily 使用规范 |
| **文档同步** | `.rules/docs.md` | API 文档、迭代记录、同步信号 |
| **交接格式** | `.rules/handoff.md` | Agent 间交接文档模板 |
| **Git 规范** | `.rules/git.md` | Commit message 格式、分支策略 |
| **子Agent分工** | `.rules/subagent-dispatch.md` | 跨端任务的能力自适应分工（多 CLI 通用） |
| **归档规则** | `.rules/archive.md` | 历史产物归档到 `.docs/archive/` |
| **AI 编码规范** | `.rules/coding.md` | 零警告交付、Rust/前端质量底线、完成前验证 |

---

## 敏感信息与配置

**所有 Agent 必须遵守**：

1. **禁止提交真实敏感信息**
   - 不要在 `AGENTS.md`、`.docs/`、`.rules/`、`docs/`、脚本或 workflow 中写真实密码、token、cookie、私钥路径、服务器登录命令。
   - 交接文档需要说明部署方式时，只写占位符，例如 `<SERVER_HOST>`、`<SSH_KEY_PATH>`、`<ALIST_USERNAME>`、`<ALIST_PASSWORD>`。

2. **本地敏感配置统一放 `.env.local`**
   - `.env.local` 已被 `.gitignore` 忽略，不提交 GitHub。
   - 仓库只提交 `.env.example`，用于列出需要哪些变量。
   - Agent 需要读取本地发布/上传配置时，优先读取环境变量；本地调试可 `source .env.local`。

3. **GitHub Actions 只使用 GitHub Secrets**
   - CI/CD 中需要的密钥统一放到 GitHub 仓库或环境的 Secrets。
   - workflow 中只允许使用 `${{ secrets.SECRET_NAME }}`，不要写明文值。
   - 当前发布/上传相关变量包括：`ALIST_URL`、`ALIST_USERNAME`、`ALIST_PASSWORD`、`UPLOAD_WEBHOOK_URL`、`UPLOAD_SECRET`、`UPDATE_API_URL`、`UPDATE_RELEASE_SECRET`、`TAURI_SIGNING_PRIVATE_KEY`、`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`。

4. **发现历史明文立即收敛**
   - 如果文档或脚本里出现真实凭据，先改为环境变量或占位符。
   - 如凭据已经提交过 GitHub，需要提醒用户轮换对应密码/token/key。

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

# 示例：让 Grok 切换到全栈
./scripts/sync-skills.sh grok universal

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

**A**: 读你的 CLI 入口文件（`CLAUDE.md` / `.kiro/README.md` / `.grok/README.md`），会指向 `AGENTS.md`，然后在"当前 Agent 分配"表中找到你的 CLI。

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
- **你的 CLI 目录**：`.claude/skills/`, `.kiro/skills/`, `.grok/skills/` 等（symlink）

### Q: 我该读哪些规则？

**A**: **必读** `.rules/` 目录下所有文件：
- `tools.md` - 工具使用（codegraph, ctx7, tavily）
- `docs.md` - 文档同步
- `handoff.md` - Agent 交接
- `git.md` - Git 规范

---

## CI/CD Workflows

### Release Workflow (`release.yml`)

**触发**：`v*` tag push 自动触发

**流程**：
1. 构建 Tauri 应用（Windows）
2. 生成 `.exe` / `.msi` 安装包
3. 创建 GitHub Release，自动从 `CHANGELOG.md` 提取对应版本内容
4. 上传安装包到 9 个云盘（通过 AList）

**云盘列表**：夸克、阿里云盘、115、百度网盘、蓝奏云、UC、Yandex、豆包、悟空（zip 格式）

### 云盘手动补传 (`upload-drives.yml`)

**触发**：手动触发（`workflow_dispatch`）

**场景**：Release 时云盘上传失败，或需要重新上传某版本

```bash
gh workflow run upload-drives.yml -f version=0.3.2
```

**参数**：`version` — 版本号（不带 `v` 前缀）

**流程**：
1. 从 GitHub Release 下载对应版本的 `.exe`
2. 登录 AList 获取 token
3. 上传到 9 个云盘

**注意**：GitHub Actions 上传经常失败（HTTP 524 超时），原因：
- Actions 在美国，通过 Cloudflare Tunnel 连接 AList 服务器，延迟高
- 服务器带宽有限（3Mbps），大文件上传容易超时

### SSH 服务器上传（推荐）

当 GitHub Actions 云盘上传失败时，通过 SSH 到服务器本地上传（AList 走 localhost，速度快且稳定）。

> 真实服务器、SSH key、AList 账号密码必须放在本地 `.env.local` 或服务器本机的环境变量中，不要写进本文档或交接文档。

本地 `.env.local` 参考 `.env.example`，至少包含：

```bash
FILTER_MANAGE_SSH_KEY=<SSH_KEY_PATH>
FILTER_MANAGE_SSH_TARGET=<SSH_USER>@<SERVER_HOST>
ALIST_LOCAL_URL=http://127.0.0.1:5244
ALIST_USERNAME=<ALIST_USERNAME>
ALIST_PASSWORD=<ALIST_PASSWORD>
```

登录服务器：

```bash
set -a
source .env.local
set +a

ssh -i "${FILTER_MANAGE_SSH_KEY}" "${FILTER_MANAGE_SSH_TARGET}"
```

服务器本地上传示例（在服务器上执行，变量从服务器本机环境或 `.env.local` 读取）：

```bash
VERSION=0.3.2
set -a
[ -f .env.local ] && source .env.local
set +a

: "${ALIST_LOCAL_URL:?missing ALIST_LOCAL_URL}"
: "${ALIST_USERNAME:?missing ALIST_USERNAME}"
: "${ALIST_PASSWORD:?missing ALIST_PASSWORD}"

cd /tmp && \
curl -sL "https://github.com/q974491089/filter-manage/releases/download/v${VERSION}/Filter-Manage_${VERSION}_x64-setup.exe" -o setup.exe && \
TOKEN=$(curl -s -X POST "${ALIST_LOCAL_URL}/api/auth/login" -H "Content-Type: application/json" -d "{\"username\":\"${ALIST_USERNAME}\",\"password\":\"${ALIST_PASSWORD}\"}" | grep -o '"token":"[^"]*"' | cut -d'"' -f4) && \
for S in quark aliyundrive 115 baidu lanzou uc yandex doubao; do \
  EP=$(python3 -c "import urllib.parse; print(urllib.parse.quote('/${S}/filter-manage/Filter-Manage_${VERSION}_x64-setup.exe'))") && \
  echo -n "上传 $S ..." && \
  R=$(curl -s -w "\n%{http_code}" --max-time 300 -X PUT "${ALIST_LOCAL_URL}/api/fs/put" -H "Authorization: $TOKEN" -H "File-Path: $EP" -H "Content-Type: application/octet-stream" --data-binary @/tmp/setup.exe) && \
  HC=$(echo "$R" | tail -1) && echo " HTTP $HC" ; \
done && \
zip -j /tmp/setup.zip /tmp/setup.exe && \
EP=$(python3 -c "import urllib.parse; print(urllib.parse.quote('/wukong/filter-manage/Filter-Manage_${VERSION}_x64-setup.zip'))") && \
echo -n "上传 wukong (zip)..." && \
R=$(curl -s -w "\n%{http_code}" --max-time 300 -X PUT "${ALIST_LOCAL_URL}/api/fs/put" -H "Authorization: $TOKEN" -H "File-Path: $EP" -H "Content-Type: application/octet-stream" --data-binary @/tmp/setup.zip) && \
HC=$(echo "$R" | tail -1) && echo " HTTP $HC" && \
rm -f /tmp/setup.exe /tmp/setup.zip && echo "完成！"
```

**优势**：
- AList 走 localhost，不受服务器带宽限制
- 无 Cloudflare Tunnel 超时问题
- 通常 2-3 分钟完成全部 9 个云盘上传

### 检查上传状态

```bash
# 查看最近 workflow 运行
gh run list --limit 5

# 查看某次运行详情
gh run view <run-id>

# 查看运行日志（过滤关键信息）
gh run view <run-id> --log | grep -E "OK|failed|warning|Upload"
```

### AList Token 健康检查

发布前建议运行：
```bash
./scripts/update-alist-tokens.sh
```

检测所有 Cookie 类网盘（夸克、UC）的 token 是否过期，豆包 token ~7 天过期。

### 文档站部署

`docs/` 目录变更 push 后自动触发 VitePress 文档站部署。`CHANGELOG.md` 通过 `docs/changelog.md` 的 `@include` 自动引用。

---

## 框架说明

详见 `.agent/README.md` 和 `.docs/archive/reports/IMPLEMENTATION_REPORT.md`（已归档）。
