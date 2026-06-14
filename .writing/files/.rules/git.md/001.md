# Git 提交规范

统一的 Git commit message 格式和分支管理策略。

---

## Commit Message 格式

```
<type>(<scope>): <subject>

[optional body]

[optional footer]
```

### Type（必需）

| Type | 说明 | 示例 |
|------|------|------|
| `feat` | 新功能 | `feat(config): 新增导出配置功能` |
| `fix` | Bug 修复 | `fix(icc): 修复配置文件解析错误` |
| `docs` | 仅文档变更 | `docs(api): 更新 ICC 接口文档` |
| `style` | 代码格式（不影响逻辑） | `style(ui): 统一按钮间距` |
| `refactor` | 重构（不是 feat 也不是 fix） | `refactor(config): 简化配置加载逻辑` |
| `perf` | 性能优化 | `perf(render): 优化列表渲染性能` |
| `test` | 新增或修复测试 | `test(config): 添加配置导入测试` |
| `chore` | 构建/工具变更 | `chore(deps): 升级 Tauri 到 2.1.0` |
| `ci` | CI 配置变更 | `ci(workflow): 添加 Linux 构建` |
| `release` | 发布新版本 | `release: v0.3.0 - 新增云盘上传` |

### Scope（可选）

模块或组件名称：
- `config` - 配置相关
- `icc` - ICC 色彩管理
- `nvidia` - NVIDIA 设置
- `ui` - 前端 UI
- `api` - 后端 API
- `docs` - 文档
- `workflow` - CI/CD

### Subject（必需）

- 简洁描述（≤50 字符）
- 动词开头（"新增"、"修复"、"优化"）
- 不要句号

### Body（可选）

详细说明：
- 为什么改
- 改了什么
- 有什么影响

### Footer（可选）

- 关闭 issue：`Closes #123`
- 破坏性变更：`BREAKING CHANGE: 配置文件格式改为 JSON`
- API 变更通知：`[API CHANGE] 修改了 load_config 返回值结构`

---

## 示例

### 简单 commit

```bash
git commit -m "feat(config): 新增导出配置功能"
git commit -m "fix(icc): 修复配置文件路径错误"
git commit -m "docs(api): 更新配置管理接口文档"
```

### 详细 commit

```bash
git commit -m "feat(config): 新增导出配置功能

支持将当前配置导出为 JSON 文件，方便备份和分享。

- 新增 export_config Tauri 命令
- 前端添加导出按钮
- 支持自定义导出路径

Closes #45"
```

### API 变更 commit

```bash
git commit -m "refactor(config): 统一配置为单文件 app.json

[API CHANGE] 配置文件从多文件合并为单个 app.json

破坏性变更：
- 原 config.json、profiles.json、presets.json 合并
- 调用 load_config 返回值结构改变
- 前端需更新配置读取逻辑

详见 .docs/handoff/config-refactor-frontend.md"
```

---

## 分支管理

### 主分支

- `main` - 生产稳定版本
- `develop` - 开发分支（可选，小项目直接用 main）

### 功能分支

命名：`feature/<feature-name>`

```bash
git checkout -b feature/export-config
# 开发...
git commit -m "feat(config): 新增导出配置功能"
git push origin feature/export-config
# 创建 PR
```

### 修复分支

命名：`fix/<bug-name>`

```bash
git checkout -b fix/icc-parsing-error
# 修复...
git commit -m "fix(icc): 修复配置文件解析错误"
git push origin fix/icc-parsing-error
# 创建 PR
```

### 发布分支

命名：`release/<version>`

```bash
git checkout -b release/v0.3.0
# 更新版本号、CHANGELOG
git commit -m "release: v0.3.0 - 新增云盘上传"
git tag v0.3.0
git push origin release/v0.3.0 --tags
```

---

## Agent 协作规则

### Frontend Agent

- 分支前缀：`feature/ui-*`, `fix/ui-*`
- Commit scope：`ui`, `config`, `components`
- 不要修改 `src-tauri/` 文件

### Backend Agent

- 分支前缀：`feature/api-*`, `fix/api-*`
- Commit scope：`config`, `icc`, `nvidia`, `api`
- 不要修改 `src/` 文件
- API 变更必须加 `[API CHANGE]` 标记

### DevOps Agent

- 分支前缀：`ci/*`, `chore/*`
- Commit scope：`ci`, `workflow`, `deps`

---

## 最佳实践

1. **原子提交** - 每个 commit 只做一件事
2. **有意义的 message** - 让别人（和未来的自己）看懂
3. **及时提交** - 功能完成就提交，不要累积
4. **先 pull 再 push** - 避免冲突
5. **API 变更必须标记** - 用 `[API CHANGE]` 前缀
6. **破坏性变更必须说明** - 在 footer 写 `BREAKING CHANGE`

---

## 工具配置（可选）

### Commitlint（未来可添加）

强制 commit message 格式：

```bash
npm install --save-dev @commitlint/{cli,config-conventional}
```

### Husky（未来可添加）

Pre-commit hook 检查：

```bash
npm install --save-dev husky lint-staged
```
