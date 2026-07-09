# 公告功能 PRD

**状态**: 草稿
**创建于**: 2026-07-10
**涉及端**: 前端 ✅ / 客户端 Rust ✅ / 服务端 ✅（改动极小，仅读接口，由维护者自理）

## 背景

当前应用**只能通过发版（更新弹窗的 CHANGELOG）向用户传达信息**。这带来两个痛点：

1. **想说的话必须绑定一次发版** —— 例如「服务器周末维护」「新版本即将上线」「某功能用法提示」这类运营/通知性内容，没有代码改动却要硬凑一个版本才能触达用户，成本高、不及时。
2. **信息只在更新时一次性出现** —— CHANGELOG 看完即走，用户没有一个「回头再看历史通知」的入口。

项目已具备一套成熟的**自建更新服务端**基础设施：

- 服务端独立部署（Cloudflare Tunnel），双域名 `filter-manage-api.xyls.us.kg` / `filter-manage-api.6ya.kdns.fr`。
- 客户端 `src-tauri/src/updater.rs` 的 `check_update` 用 `tokio::select!` **双域名竞速**请求 `/api/check-update`，先到先得、抗单点 DNS 故障。

公告功能可**直接复用这套设施**：服务端加一个只读接口，客户端照抄竞速模式加一个拉取命令。发公告随时可发、无需发版。

## 目标

- 让维护者**不发版即可向所有用户推送通知**（运营通知、活动、维护预告、重要提醒）。
- 用户有一个**常驻、不打扰**的入口查看公告，并能区分已读/未读。
- 少数**重要公告**能强触达（启动弹窗），普通公告静默展示（顶栏铃铛）。
- 复用现有更新服务端与双域名竞速机制，**不新增独立基础设施**，天然抗单点故障。

可量化：

- 发一条新公告到用户可见，**无需任何客户端发版**，仅改服务端数据源。
- 公告拉取**不新增独立网络请求时机**——随启动的 `check_update` 一并完成。

## 范围

### 前端（React/TS）

- **顶栏铃铃图标入口**：常驻，有未读时显示红点/未读数。
- **公告面板**：点击铃铛展开，多条公告列表（Markdown 渲染，复用更新弹窗已有的 `prose prose-invert prose-sm` 样式）。
- **重要公告启动弹窗**：启动拉取后，若存在**未读且 `level === "important"`** 的公告，自动弹窗展示（复用/参照 `UpdateModal` 的视觉与尺寸规范）。
- **已读状态管理**：localStorage 记录已读公告 id 集合；已读的不再触发红点/弹窗。
- **有效期过滤与排序**：客户端按当前时间过滤 `[startAt, endAt]` 窗口外的公告，并按 `publishedAt` 倒序排序（服务端只透传文件，过滤排序全在客户端）。
- 所有新组件遵循 `DESIGN.md`，并按项目规范添加 `data-component` / `data-name` 属性。

### 客户端 Rust（Tauri）

- 新增 `get_announcements` 命令：**双域名竞速**请求 `GET /api/announcements`，解析为 `Vec<Announcement>` 返回前端。实现参照 `updater.rs::check_update` 的 `tokio::select!` 模式。
- 定义 `Announcement` 数据结构（`#[serde(rename_all = "camelCase")]`，与前端 TS 类型一一对应）。
- 在 `lib.rs` 的 `invoke_handler` 注册新命令。
- 复用 `UPDATE_API_HOSTS` 常量（同一服务端）。

### 服务端（改动极小）

- 提供只读接口 `GET /api/announcements`，直接返回 `announcements.json` 文件内容（JSON 数组）。**无鉴权**（与 `/api/check-update` 一致，公告本就是公开信息）。
- 可由 nginx 直接静态托管该文件，或经应用服务器透传；**服务端无需过滤/排序逻辑**，过滤与排序全交给客户端。
- 公告内容维护：维护者手动编辑本地 `announcements.json`，用 **scp**（`FILTER_MANAGE_SSH_TARGET`）上传到服务器路径。**本次不做写接口 / 管理界面**——那是后续独立迭代。
- 部署在与 `check-update` 相同的服务端与双域名之后。

## 非目标

- ❌ **不做**公告写接口 / 发布后台 / 管理界面（维护者用 scp 上传 JSON；写接口+管理界面是后续独立迭代，届时再设计鉴权）。
- ❌ **不做**已读状态跨设备同步（纯本地 localStorage，单机桌面工具足够）。
- ❌ **不做**公告的富交互（点赞、评论、跳转深链、图片附件等），MVP 仅 Markdown 文本。
- ❌ **不做**推送/长连接实时下发；仅在启动随 `check_update` 拉取一次。
- ❌ **不改**现有更新（updater）逻辑本身，只并列新增公告拉取。

## 涉及端与分工提示

跨三端，建议按 `.rules/subagent-dispatch.md` 分工：

| 端 | 改动 | 关键文件 |
|----|------|---------|
| **服务端** | 提供 `GET /api/announcements`（读静态 `announcements.json`，无鉴权）；维护者 scp 上传该文件 | 由维护者自行处理，**不需交接文档** |
| **客户端 Rust** | `get_announcements` 命令（双域名竞速）+ `Announcement` 结构 + `lib.rs` 注册 | `src-tauri/src/announcements.rs`（新建）、`src-tauri/src/lib.rs` |
| **前端** | 铃铛入口 + 公告面板 + 重要公告弹窗 + 已读态 + 有效期过滤 | `src/components/`（新建 AnnouncementBell / AnnouncementPanel / AnnouncementModal）、`src/hooks/`（新建 useAnnouncements）|

**契约先行**：`announcements.json` 的字段结构是三端共享的契约——客户端 Rust 的 `Announcement` 结构、前端 TS 类型、维护者手写的 JSON 都要对齐它。先在 `.docs/api/announcements.md` 定稿，前端可先用 mock 数据推进 UI。

### 数据契约（草案，plan 阶段细化）

```jsonc
// GET /api/announcements  →  Announcement[]
{
  "id": "2026-07-10-maintenance",   // 稳定唯一 id，前端据此记已读
  "title": "服务器维护通知",
  "body": "## 维护时间\n本周六 02:00–04:00…",  // Markdown
  "level": "normal",                // "normal" | "important"
  "publishedAt": "2026-07-10T08:00:00Z",
  "startAt": "2026-07-10T00:00:00Z", // 可选，生效时间
  "endAt": "2026-07-13T00:00:00Z"    // 可选，过期时间
}
```

## 验收标准

- [ ] `GET /api/announcements` 返回 `announcements.json` 内容（JSON 数组，字段符合契约），无需鉴权即可访问。
- [ ] 客户端 `get_announcements` 命令双域名竞速成功，任一域名可达即返回；两个都不可达时返回空列表且不报错、不阻塞启动。
- [ ] 应用启动时随 `check_update` 一并拉取公告，**不引入额外独立请求时机**。
- [ ] 顶栏出现铃铛图标；存在未读公告时显示红点/未读数，无未读时不显示。
- [ ] 点击铃铛展开面板，多条公告按 `publishedAt` 倒序、Markdown 正确渲染。
- [ ] 存在未读且 `level === "important"` 的公告时，启动自动弹窗；普通公告不弹窗，仅在铃铛面板内。
- [ ] 用户查看后该公告标记已读（localStorage 持久化）；重启应用不再触发其红点/弹窗。
- [ ] 客户端负责按当前时间过滤有效期窗口外的公告、按 `publishedAt` 倒序排序（服务端不做过滤/排序）。
- [ ] 维护者编辑 `announcements.json` 并 scp 上传后，用户端即可见新公告，**无需客户端发版**（发一条测试公告验证端到端）。
- [ ] 所有新前端组件带 `data-component` / `data-name` 属性，视觉符合 `DESIGN.md`。
- [ ] `.docs/api/announcements.md` 契约文档与实现一致（后端 API 变更须同步文档，见 `.rules/docs.md`）。

## 已知风险 / 依赖 / 坑

- **服务端由维护者自理**：读接口 + scp 上传都由你处理，本仓库不产出服务端交接文档；只需保证 `announcements.json` 结构与 `.docs/api/announcements.md` 契约一致。
- **读接口公开无鉴权**：任何人可读公告（与 check-update 一致，公告本就公开），无隐患；写路径仅 scp，已由 SSH 密钥鉴权，故无 HTTP 写接口、无需鉴权。
- **双域名竞速的降级语义**：更新用的是「都不可达 → 回退 GitHub 直连」。公告**无 GitHub 兜底也可接受**——两域名都不可达时**静默返回空列表**（无公告不是错误状态，不打扰用户）。这一点与 updater 的 fallback 不同，需在实现中明确。
- **已读态仅本地**：换设备/重装会重新出现未读。符合非目标，但需在文档说明预期。
- **时间与时区**：`startAt/endAt/publishedAt` 统一用 UTC ISO8601，前端展示按本地时区格式化，避免过期判断错乱。
- **WSL 构建约束**：客户端 Rust 改动的编译/构建须在 Windows PowerShell 执行（见项目记忆 [dev_environment]），WSL 仅编辑。
- **Markdown 安全**：公告 body 是服务端下发的富文本，前端渲染须复用更新弹窗已验证的 Markdown 渲染路径，避免 XSS（内容源可信但仍走既有安全渲染）。
- **弹窗打扰度**：`important` 才弹窗是刻意设计，服务端发布时须克制使用 `important`，否则退化成「每次启动都弹」的骚扰。属运营约定，非技术约束，文档提示即可。
