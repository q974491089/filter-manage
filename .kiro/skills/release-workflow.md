# Skill: 发布新版本

## 触发条件

当用户表达以下语义时激活此 skill：
- "发布"、"发版"、"release"、"发布新版本"、"打包发布"
- "准备发布 vX.X.X"

## 发布流程

### 1. 确定版本号

- 读取 `src-tauri/tauri.conf.json` 中的当前 `version`
- 根据变更类型确定新版本号（遵循 semver）：
  - 修复 bug → patch（0.2.0 → 0.2.1）
  - 新功能 → minor（0.2.0 → 0.3.0）
  - 破坏性变更 → major（0.2.0 → 1.0.0）
- 如果用户指定了版本号，使用用户指定的

### 2. 收集变更信息

从以下来源收集本次发布的变更内容：
- `docs/README.md` 迭代记录中当前版本块的内容
- `git log` 自上次 tag 以来的 commit 信息
- 用户口述的变更说明

整理为以下分类（按需使用，没有的分类不写）：
```markdown
### 新功能
- 功能描述

### 修复
- 修复描述

### 改进
- 改进描述

### 破坏性变更
- 变更描述
```

### 3. 更新版本号（同步所有位置）

```bash
# 需要同步更新的文件：
src-tauri/tauri.conf.json  → "version": "X.X.X"
package.json               → "version": "X.X.X"
src-tauri/Cargo.toml       → version = "X.X.X"
```

> **⚠️ 版本号一致性检查（必须！）**
> 在提交前，验证以上三个文件的版本号与即将发布的 tag 版本一致。
> 版本号不一致会导致 CI 构建失败（tauri-action 要求 tag 版本与 tauri.conf.json 中的 version 匹配）。
>
> 快速检查命令：
> ```bash
> grep '"version"' src-tauri/tauri.conf.json package.json
> grep '^version' src-tauri/Cargo.toml
> ```
> 三处输出的版本号必须相同，且等于即将打的 tag（去掉 `v` 前缀）。

### 4. 更新 CHANGELOG.md（必须！）

> **⚠️ 此步骤不可跳过。** CHANGELOG.md 是唯一的变更记录来源，同时服务于：
> - GitHub Release 发布说明（CI 自动提取对应版本段落）
> - 文档站 `/changelog` 页面（`docs/changelog.md` 通过 `<!--@include: ../CHANGELOG.md-->` 自动引用）
>
> 只需维护根目录 `CHANGELOG.md` 一份文件，文档站会自动同步。

在 `CHANGELOG.md` 文件**顶部**（`# Changelog` 标题之后）插入新版本块：

```markdown
## vX.X.X — YYYY-MM-DD

### 新功能
- ...

### 修复
- ...

### 改进
- ...
```

**格式要求**：
- 日期格式：`YYYY-MM-DD`
- 每个条目一行，以 `- ` 开头
- 分类按需使用，没有的不写
- 不修改历史版本块

### 5. 提交并推送代码

```bash
git add -A
git commit -m "release: vX.X.X - <一句话概括>"
git push
```

### 6. 打 tag 触发构建

```bash
git tag vX.X.X
git push origin vX.X.X
```

这会自动触发：
- **Release workflow**（`.github/workflows/release.yml`）：在 Windows 环境构建 Tauri 应用，生成 `.exe` / `.msi` 安装包，创建 GitHub Release 并附加安装包，Release body 自动从 CHANGELOG.md 提取
- **Docs workflow**（`.github/workflows/docs.yml`）：如果 `docs/` 目录有变更，自动构建并部署 VitePress 文档站

### 7. 验证发布结果

```bash
# 检查 CI 状态
gh run list --limit 3

# 确认 Release 创建成功
gh release view vX.X.X
```

确认以下内容：
- [ ] Release 页面有安装包附件（.exe, .msi）
- [ ] Release body 显示了 CHANGELOG 内容
- [ ] 文档站已更新（如有 docs 变更）

## 文件关系图

```
CHANGELOG.md（唯一维护点）
├── → GitHub Release body（CI 自动提取当前版本段落）
└── → 文档站 /changelog 页面（VitePress @include 引用）

版本号（三处同步）
├── src-tauri/tauri.conf.json
├── package.json
└── src-tauri/Cargo.toml
```

## 注意事项

- **⚠️ 严禁修改 CI workflow 的包管理器配置** — 本地开发用 pnpm，但 CI（`.github/workflows/`）必须用 npm。pnpm 在 GitHub Actions 上有兼容性问题无法运行。绝对不要把 CI 里的 `npm ci` 改成 `pnpm install`，不要添加 `pnpm/action-setup`，不要动 workflow 文件中任何与包管理器相关的配置。
- 两个 lock 文件共存：`pnpm-lock.yaml`（本地开发）+ `package-lock.json`（CI 构建）
- 如果修改了 `package.json` 依赖，需要同时更新 `package-lock.json`：`npm install --package-lock-only`
- tag 必须以 `v` 开头（如 `v0.3.0`），否则不会触发 Release workflow
- 如果构建失败需要重新触发：删除旧 tag 和 release，重新打 tag
  ```bash
  git tag -d vX.X.X
  git push origin :refs/tags/vX.X.X
  gh release delete vX.X.X --yes
  git tag vX.X.X
  git push origin vX.X.X
  ```
