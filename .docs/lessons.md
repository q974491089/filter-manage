# 踩坑经验集（Agent Lessons）

> 来源：内置预览与分享预览系列改动（2026-11，`codex/color-fixes-no-login` 分支）及
> 服务端发布过程中的真实事故沉淀。规范条目同步在 `.rules/coding.md`（本地不入库）；
> 本文为入库主副本，含事故过程与修复 commit，便于回溯。

## 1. Tailwind 同级互斥类按声明序取胜（踩过两次）

**事故**：`SegmentedTabs` 组件根写死 `relative`，调用方传 `absolute`（浮在预览图右上角）。
两个类拼接在同一 className 上，**谁生效取决于 Tailwind 的 CSS 声明顺序**（`relative`
生成在 `absolute` 之后）—— 与 className 字符串拼接顺序无关。第一次表现为全宽横条
盖住角标；重写组件时又写回 `relative`，第二次表现为浮层错位到图片外。

**正确姿势**：

- **可复用组件的根不声明 position 类**，定位完全交给调用方（`SegmentedTabs` 即此约定，
  滑块测量依赖 positioned 容器，调用方传 `relative` 或 `absolute`）。
- 同理适用 `hidden` vs `block`、`flex` vs `grid` 等一切互斥类。
- 排查特征：样式"看起来没生效/偏到奇怪的位置"时，先检查元素上是否有两个互斥类。

修复：客户端 `3dc01b6` → `6370319` → `b673efc`（三次提交，前两次均为踩坑过程）。

## 2. 验证退出码被管道吞掉（曾把语法错误直接 commit）

**事故**：`npx tsc --noEmit | grep -v warn | head -5; echo "TSC_DONE" && git commit`
—— tsc 报语法错误，但管道里 grep/head 的退出码覆盖了前者，`echo` 自造的成功标签
让 `&&` 链继续执行，**语法错误代码被直接 commit**（后被迫 amend）。

**正确姿势**：

```bash
if npx tsc --noEmit 2>&1 | grep -qE "error TS"; then
  echo "TSC FAILED:"; npx tsc --noEmit 2>&1 | grep "error TS" | head -5
else
  git commit ...
fi
```

- 用条件分支**真校验**，禁止 `cmd | grep; echo TAG && next` 模式。
- 顺带：`||` 兜底写在管道后同样无效（`(a | b) || c` 里 b 的退出码覆盖 a）。

## 3. Windows Git Bash curl 传中文 body = GBK 乱码入库

**事故**：用 `curl -d '{"title":"字段验证-可删除",...}'` 创建测试分享，命令行参数被
Windows 按本地代码页（GBK）发出，服务端按 UTF-8 收 → **数据库存入坏字节**，
管理端显示乱码（生产 1 条 + 预发 2 条，App 内正常创建的数据全部无恙）。

**正确姿势**：

- 含中文的请求体**先写 UTF-8 文件再 `curl -d @payload.json`**。
- 排查特征：管理端乱码但网页本身编码正确、仅个别记录异常 → 怀疑录入链路编码。

## 4. CI 构建失败：.gitignore 误伤 go:embed 资源

**事故**：moderation 词库目录 `internal/moderation/data/` 被全局 `data/` 规则忽略，
CI 检出后 `go:embed data/*.txt` 找不到文件，docker build 失败；本地 go build 通过
（本地有未跟踪文件）导致问题只在 CI 爆出。

**正确姿势**：

- 引入 `//go:embed` 等**构建期依赖文件**时，先确认它们进了版本控制
  （`git ls-files` 验证），并给 `.gitignore` 加精确例外。
- 例外写法：全局规则忽略目录后，需先重新包含目录本身再包含文件：

  ```gitignore
  data/
  !server-go/internal/moderation/data/
  !server-go/internal/moderation/data/*.txt
  ```

修复：api 仓库 `3a2a58c`。

## 5. 新增字段漏掉 service 层映射（API 存了空串）

**事故**：shares 表新增 `preview_scene`，repo/model/handler/校验/返回结构全改了，
唯独 `service.Create` 的 `CreateInput → Share` 映射表漏了该字段 → **创建的分享
永远存空串**（详情返回也空），而单测只覆盖了 Normalize，没覆盖映射。

**正确姿势**：

- 新增透传字段时，沿"请求绑定 → 输入结构 → 领域对象 → 存储列 → 返回视图"整条链
  逐层 grep 字段名，确认每一跳都有。
- 补**集成测试**把输入到存储的完整映射钉死（`share/service_integration_test.go`
  的 `TestCreateRoundTripsEveryField`），防止下一个字段重蹈覆辙。

修复：api 仓库 `0b4a50c`。

## 6. git 子命令受 cwd 影响的排查陷阱

`git ls-tree -r main --name-only` 在仓库**子目录**里运行时只列出该子目录的树 ——
曾在 `src-tauri/` 下跑导致全仓搜 `edge-functions/` 误判"文件不存在"，绕了远路。
全仓排查请在仓库根执行；同理 `git stash show`、`git diff` 的路径过滤也要注意
相对路径基准。

## 7. 环境工具备忘（Windows / WSL 混合开发）

- **GitHub 直连失败**：Windows Git Bash 时通时不通，先重试；WSL 内不通，用
  `gh-proxy.com/https://github.com/...` 前缀或 WSL 的 `gh`（已认证）。
- **gh CLI**：WSL（`/usr/bin/gh`）与 Windows（`C:/Program Files/GitHub CLI/gh.exe`）
  双份已认证；新装的 gh 不在旧终端会话 PATH 里，用完整路径调用。
- **wrangler**：WSL 侧连不上 Cloudflare API（`fetch failed`）；Windows 侧
  `npx wrangler deploy` 直连正常（已 `wrangler login`，2026-11-15）。
- **Cloudflare 代理端口**：v2rayN 监听 `127.0.0.1:10808`，WSL（NAT 模式）不可达；
  Windows 程序直接设 `HTTPS_PROXY=http://127.0.0.1:10808` 即可。
- **worktree 占用分支**：分支被其它 worktree 检出时 `git checkout` 会被拒；
  `git worktree remove` 在 Windows 上可能因路径过长失败但已解除注册，
  残留目录用 `rm -rf` 清理即可。
- **stash 提取未跟踪文件**：未跟踪文件存在 stash 的第三个父提交，
  `git restore --source="stash@{0}^3" -- <path>` 可单独取出。
- **gitignore 目录误伤**：全局规则（如 `data/`）会静默吞掉同名源码资产目录，
  新增构建期资产后用 `git ls-files` 确认是否真的被跟踪。
