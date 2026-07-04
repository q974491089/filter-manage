# 自定义更新流程（多镜像 + 可取消 + 测速换源）实现计划

**状态**: Phase 1-3 完成，待 Phase 4（CI + 联调）
**优先级**: 高
**创建于**: 2026-06-19
**涉及端**: 服务端（Spring Boot）+ 客户端 Rust（Tauri）+ 前端（React）

---

## 背景

当前更新完全依赖 Tauri 内置 updater（`@tauri-apps/plugin-updater`）：

- 前端 `useUpdater.ts:41` 调 `check()` 读 GitHub Release 的 `latest.json`，`useUpdater.ts:73` 调 `downloadAndInstall()` 一把梭，下载源 / 进度 / 签名校验 / 安装 / 重启全在插件内部。
- `tauri.conf.json:33-38` 把 endpoint 写死成 `github.com/.../releases/latest/download/latest.json`，国内用户直连 GitHub 经常超时。
- `UpdateModal.tsx` 只有一个进度条，**没有取消、没有测速、没有换源**。

**内置 updater 的硬限制**：endpoint 编译期固定；`downloadAndInstall` 不暴露中途取消、不支持运行时切换下载 URL、进度回调里拿不到测速钩子。

**目标**：自建更新流程，支持
1. 多镜像（服务端下发，可随时增删，无需发新客户端）
2. 下载中实时测速
3. 慢速时提示换源、取消当前下载、换源重下
4. 复用现有 minisign 签名校验与 NSIS 安装产物

---

## 决策摘要（已拍板）

| # | 决策 | 选择 | 理由 |
|---|------|------|------|
| 1 | 服务端部署 | **`.env.local` 配置的腾讯云服务器 + Java Spring Boot** | 复用现有服务器 + Cloudflare Tunnel；fat jar + systemd |
| 2 | 下载执行层 | **Rust 后端（reqwest 流式）** | 镜像由服务端下发可随时增删；前端 `fetch` 受**编译期** CSP/`http` 白名单限制，新增镜像域名会被拦截。Rust 原生请求不受 CSP 约束，且签名校验/安装本就必须在 Rust |
| 3 | 签名体系 | **复用 minisign + 现有 pubkey** | CI 已生成 `.sig`；客户端用 `tauri.conf.json` 里的 pubkey 校验，零新增密钥管理 |
| 4 | 版本清单来源 | **CI 构建后 POST 推送给服务端** | 复用现有「CI→服务器 webhook」模式；服务端在国内，避免请求时回源 GitHub |

> **下载层为何不选前端 fetch + ReadableStream**：该方案需把 6–10MB 安装包缓冲进 JS 堆再经 IPC 传给 Rust，并且必须为每个镜像域名在 `capabilities` / CSP 中放开 `http` 权限——而这是编译期固定的，与「镜像由服务端动态下发」的核心诉求冲突。Rust 下载无此问题。

---

## 功能范围

| 功能 | 状态 | 说明 |
|------|------|------|
| 服务端 `/api/check-update` | TODO | 返回版本信息 + 多镜像 URL（Spring Boot） |
| 服务端 `/api/internal/release`（CI 推送） | TODO | 接收 CI 上报的版本清单，鉴权 |
| Rust 检查更新命令 | TODO | 调服务端 API，返回结构化更新信息 |
| Rust 流式下载 + 进度事件 | TODO | reqwest 流式下载到临时文件，emit 进度 |
| Rust 取消下载 | TODO | 原子标志位，删临时文件 |
| Rust minisign 校验 | TODO | 用现有 pubkey 校验下载产物 |
| Rust 启动安装 + 重启 | TODO | 启动 NSIS installer 后退出应用 |
| 前端重写 useUpdater | TODO | 调 Rust 命令 + 监听事件 + 测速 |
| 前端 UpdateModal 换源 UI | TODO | 测速展示 + 慢速换源提示 + 镜像选择 |
| CI 推送清单 | TODO | release.yml 构建后 POST 服务端 |

---

## 总体架构

```
                         ┌──────────────────────────────────────┐
   CI (release.yml)      │  腾讯云 <SERVER_HOST> (Spring Boot)     │
   构建完读 latest.json ─┼─► POST /api/internal/release          │
   {version,notes,sig}   │     存储最新清单 (JSON/SQLite)         │
                         │                                        │
   客户端 ───────────────┼─► GET /api/check-update?current=x.y.z  │
                         │     ◄── {hasUpdate,version,notes,       │
                         │          signature, mirrors[]}          │
                         └──────────────────────────────────────┘
                                         │ mirrors[] = 镜像前缀 + GitHub asset URL
                                         ▼
   客户端 Rust reqwest ──► 流式下载选定镜像 ──► 临时文件
        │  emit update://progress (downloaded/total)
        ▼
   前端测速：>120s 或 <100KB/s ──► 弹「换源」──► cancel + 换 URL 重下
        │
        ▼
   下载完 ──► minisign 校验 (pubkey from tauri.conf) ──► 启动 NSIS installer ──► app.exit ──► 重启新版
```

---

## 命令与事件契约（三端共用，**唯一权威**）

> 此契约是前端 / Rust / 服务端三份技术文档的共同基准，任何一端修改都要同步另外两端。

### 数据结构

```typescript
// 前端 TS（与 Rust struct 对齐）
interface Mirror { name: string; url: string }
interface UpdateInfo {
  hasUpdate: boolean;
  version: string;        // 如 "0.3.4"
  notes: string;          // markdown
  signature: string;      // base64 minisign 签名（与镜像无关，同版本唯一）
  mirrors: Mirror[];      // 按推荐顺序排列
}
```

```rust
// Rust（serde，camelCase 序列化给前端）
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Mirror { pub name: String, pub url: String }

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub has_update: bool,
    pub version: String,
    pub notes: String,
    pub signature: String,
    pub mirrors: Vec<Mirror>,
}
```

### Tauri 命令（前端 → Rust）

| 命令 | 签名 | 说明 |
|------|------|------|
| `check_update` | `() -> Result<UpdateInfo, String>` | Rust 读自身版本号，GET 服务端 API；结果存入 Rust state |
| `download_update` | `(url: String) -> Result<(), String>` | 流式下载指定镜像到临时文件；用 state 里的 signature；过程 emit 事件 |
| `cancel_update_download` | `() -> Result<(), String>` | 置取消标志，中断当前下载并删除临时文件 |
| `install_update` | `() -> Result<(), String>` | 启动已校验的 installer 并退出应用（仅在 `verified` 后可调） |

### 事件（Rust → 前端，`@tauri-apps/api/event`）

| 事件名 | payload | 时机 |
|--------|---------|------|
| `update://progress` | `{ downloaded: u64, total: u64 }` | 每个下载 chunk |
| `update://verified` | `{}` | 下载完成且 minisign 校验通过 |
| `update://error` | `{ message: String }` | 下载/校验失败 |
| `update://cancelled` | `{}` | 取消完成 |

> **测速在前端做**：前端按 `update://progress` 的 `downloaded` 增量 / 时间增量算速度，Rust 保持简单（只报字节数）。

---

## 客户端（Rust）方案

详见 → **`.docs/api/updater.md`**

要点：
- **新增模块** `src-tauri/src/updater.rs`，在 `lib.rs` 注册 4 个命令 + `manage` 一个 `UpdaterState`。
- **新增依赖**（`Cargo.toml`）：
  ```toml
  reqwest = { version = "0.12", default-features = false, features = ["stream", "rustls-tls"] }
  minisign-verify = "0.2"
  futures-util = "0.3"
  # tokio 由 tauri 提供，命令用 async fn 即可
  ```
  > 用 `rustls-tls` 避免 Windows 上的 OpenSSL 依赖，保证 CI 可编译。
- **下载**：`reqwest::get(url).bytes_stream()` 逐 chunk 写临时文件（`%TEMP%\filter-manage-update-<ver>.exe`），累计 `downloaded`，每 chunk `app.emit("update://progress", ...)`；每个 chunk 前检查 `AtomicBool` 取消标志。
- **校验**：`minisign-verify` 用 `tauri.conf.json` 的 pubkey 校验临时文件。**解码方式与 tauri-plugin-updater 内部一致**：pubkey 与 signature 均为 base64 包裹的 minisign 文本，先 base64 decode 再交给 minisign-verify。⚠️ 确切 API 写代码时对照 `minisign-verify` docs.rs 验证。
- **安装**：启动下载好的 NSIS installer（参考 tauri-plugin-updater 的 NSIS 安装参数，静默 + 自动重启），随后 `app.exit(0)` 让 installer 替换文件。⚠️ 确切 NSIS 参数对照 tauri-plugin-updater 源码验证。
- **安全红线**：**必须在校验通过后才启动 installer**。下载源是不可信镜像（gh-proxy），跳过校验 = 给恶意镜像投毒机会。

---

## 客户端（前端）方案

详见 → **`.docs/handoff/custom-updater-frontend.md`**

要点：
- **重写 `src/hooks/useUpdater.ts`**：`check()`/`downloadAndInstall()` 全部换成 `invoke(...)` + `listen(...)`；新增 `speed`、`mirrors`、`currentMirror`、`showMirrorPrompt` 状态；status 增加 `verifying` / `ready` / `cancelled`。
- **测速逻辑**：记录下载起始时间与上次采样；按 `update://progress` 算瞬时/平均速度；`elapsed > 120s || avgSpeed < 100KB/s` → `setShowMirrorPrompt(true)`。
- **换源**：`switchMirror(url)` = `invoke('cancel_update_download')` → 等 `update://cancelled` → `invoke('download_update', { url })`。signature 与镜像无关（同版本唯一），换源只换 URL。
- **重写 `src/components/UpdateModal.tsx`**：进度态加测速文字 + 取消按钮；慢速时展示镜像列表（来自 check 结果）供选择。遵循 `DESIGN.md`，所有元素带 `data-component` / `data-name`。
- **权限**：`process:allow-restart` 已在 `capabilities/default.json`，无需新增 webview 权限（下载在 Rust）。

---

## 服务端（Spring Boot）方案

详见 → **`.docs/handoff/custom-update-server.md`**

要点：
- **`GET /api/check-update?current=x.y.z`**（公开只读）：semver 比较 `current` 与已存储 `version` → `hasUpdate`；`mirrors[]` 在请求时由「镜像前缀列表（配置）+ GitHub asset URL」拼出。
- **`POST /api/internal/release`**（CI 内部，`X-Release-Secret` 鉴权）：接收 `{version, notes, signature, assetName}`，持久化为「最新版本清单」。
- **镜像列表放在服务端配置**（`application.yml`），增删镜像改配置即可，**无需发新客户端**——这是整个方案的核心收益。
- **无需 CORS**：客户端是 Rust 原生请求，不走浏览器。
- **无需私钥**：服务端只透传 base64 签名，不接触 minisign 私钥。
- **部署**：fat jar + systemd（`filter-manage-update.service`），监听 `127.0.0.1:<port>`，Cloudflare Tunnel 加一条路由暴露 `/api/check-update`。

---

## CI 改动（`.github/workflows/release.yml`）

构建步骤后新增一步：读 `latest.json`（tauri-action 已生成，含 `version` / `notes` / `platforms.windows-x86_64.signature` / `url`），POST 到服务端 `/api/internal/release`，带 `X-Release-Secret`。

新增 GitHub Secret：
- `UPDATE_API_URL` — 服务端 `/api/internal/release` 公网地址（用主域名 `https://filter-manage-api.xyls.us.kg/api/internal/release`；客户端 `check_update` 已做双域名竞速，CI 推送用单主域名即可）
- `UPDATE_RELEASE_SECRET` — 与服务端 systemd 环境变量一致

**`release.yml` 新增 step**（放在 `Build Tauri app` 之后，与 `Trigger server upload` 并列；runs-on 是 windows-latest，必须 `shell: bash`）：

```yaml
  - name: 推送版本清单到更新服务
    shell: bash
    env:
      UPDATE_API_URL: ${{ secrets.UPDATE_API_URL }}
      UPDATE_RELEASE_SECRET: ${{ secrets.UPDATE_RELEASE_SECRET }}
      NOTES: ${{ steps.changelog.outputs.body }}   # 复用 Extract changelog step 的输出
    run: |
      VERSION="${GITHUB_REF_NAME#v}"
      # signature: 从构建产物 .sig 文件读（base64 文本）—— v0.3.3 漏带，这次必须补
      SIG_FILE=$(find src-tauri/target -name "*.exe.sig" | head -1)
      if [ -z "$SIG_FILE" ]; then echo "::error::.sig not found, cannot push manifest"; exit 1; fi
      SIG=$(jq -Rs @base64 < "$SIG_FILE" | tr -d '\n')
      # assetName: setup.exe 文件名
      ASSET=$(basename "$(find src-tauri/target -name "*-setup.exe" | head -1)")
      jq -n --arg v "$VERSION" --arg n "$NOTES" --arg s "$SIG" --arg a "$ASSET" \
        '{version:$v,notes:$n,signature:$s,assetName:$a}' \
      | curl -sf -X POST "$UPDATE_API_URL" \
          -H "Content-Type: application/json" \
          -H "X-Release-Secret: $UPDATE_RELEASE_SECRET" \
          --data @-
```

> signature 从构建产物 `*.exe.sig` 文件取（tauri-action 用 `TAURI_SIGNING_PRIVATE_KEY` 生成），`base64` 编码后放入 manifest。客户端 `verify_minisign` 会 base64 解码后用 minisign-verify 校验安装包完整性。
> 推送失败应阻断 CI（`curl -sf` 失败非零退出），因为服务端拿不到新清单会导致客户端永远查不到更新。

---

## 实现阶段

### Phase 1 — 服务端 ✅ 已完成（Spring Boot + Docker）
- [x] Spring Boot 3.3.6 + Java 17 工程，多阶段 Docker 构建
- [x] `GET /api/check-update` — 返回版本信息 + 7 个镜像 URL（6 个可用代理 + GitHub 原始）
- [x] `POST /api/internal/release` — CI 推送版本清单，`X-Release-Secret` 鉴权
- [x] JSON 文件存储版本清单
- [x] Docker 部署：容器 `filter-manage-api`，端口 9877，加入 `1panel-network`
- [x] Cloudflare Tunnel 路由：`https://filter-manage-api.xyls.us.kg`
  - `/upload` → 原有 webhook
  - 其余路径 → Java 服务
- [x] 镜像站实测：13 个代理站筛出 6 个可用 + GitHub 原始 = 7 个源

### Phase 2 — 客户端 Rust ✅ 已完成（编译通过）
- [x] 加依赖，建 `updater.rs`，实现 4 命令 + 事件 + 取消标志 + 并发防护
- [x] `check_update` 打通服务端（含降级到 tauri-plugin-updater 直连 GitHub）
- [x] 流式下载 + 进度事件 + minisign 校验（从 tauri-plugin-updater 源码验证 API）
- [ ] NSIS 安装 + 重启（待真机验证）
- [x] 更新 `.docs/api/updater.md`（移除所有 ⚠️ 占位，补充完整实现）

### Phase 3 — 前端 ✅ 已完成
- [x] 重写 `useUpdater.ts` — `invoke` + `listen` 替换 `plugin-updater`，新增测速 / 换源 / 取消逻辑（ref 模式避免竞态）
- [x] 重写 `UpdateModal.tsx` — 速度展示 + 取消按钮 + 镜像列表换源 UI + `data-component` / `data-name`
- [x] `App.tsx` 集成点兼容（`checkForUpdate` 签名不变，`<UpdateModal />` 无 props）
- [ ] 联调全流程（待 Rust 真机验证后）

### Phase 4 — CI + 收尾
- [ ] release.yml 加清单推送步骤 + GitHub Secrets
- [ ] 端到端：发版 → 客户端检查 → 慢速换源 → 校验 → 安装
- [ ] （可选）移除 `tauri-plugin-updater` 依赖与 `tauri.conf.json` 的 `plugins.updater`

---

## 待确认问题

- [x] **服务端公网 URL / 路由**：已部署 — `https://filter-manage-api.xyls.us.kg`，`/upload` 走原有 webhook，其余路径走 Java 服务（Rust 常量 `UPDATE_API_BASE` 已同步）
- [x] **清单存储介质**：JSON 文件（服务端已实现，`/opt/filter-manage-api/latest.json`）
- [x] **NSIS 安装参数**：已从 tauri-plugin-updater v2.10.1 源码确认 — Passive 模式 `/P /R /UPDATE`
- [x] **`current` 兜底**：建议缺参时返回最新但 `hasUpdate=false`（已在服务端文档中明确）
- [x] **降级策略**：已实现 — 服务端不可达时回退 `tauri-plugin-updater` 直连 GitHub `latest.json`

---

## 相关文件

| 文件 | 状态 | 端 |
|------|------|----|
| `src-tauri/src/updater.rs` | ✅ 已创建（编译通过） | Rust |
| `src-tauri/src/lib.rs` | ✅ 已改（注册命令 + state） | Rust |
| `src-tauri/Cargo.toml` | ✅ 已改（+4 依赖） | Rust |
| `.docs/api/updater.md` | ✅ 已重写（完整实现，无占位） | Rust 文档 |
| `src/hooks/useUpdater.ts` | ✅ 已重写（invoke + listen + 测速 + 换源） | 前端 |
| `src/components/UpdateModal.tsx` | ✅ 已重写（速度/取消/换源 UI） | 前端 |
| `.github/workflows/release.yml` | 待改（清单推送） | CI |
| 服务端工程 `filter-manage-api` | ✅ 已部署（Docker + Tunnel） | 服务端 |

## 配套技术文档

- **Rust 客户端 API** → `.docs/api/updater.md`
- **前端交接** → `.docs/handoff/custom-updater-frontend.md`
- **服务端交接** → `.docs/handoff/custom-update-server.md`
