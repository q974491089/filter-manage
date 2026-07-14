# Announcements API（公告功能）

> **状态：🚧 实现中** — 本文档是公告功能三端共享的数据契约。plan: `.docs/plans/2026-07-10-announcements.md`，PRD: `.docs/prd/2026-07-10-announcements.md`。

模块：`src-tauri/src/announcements.rs`
注册：`src-tauri/src/lib.rs`（`invoke_handler`）
前端消费：`src/lib/announcements.ts`、`src/hooks/useAnnouncements.ts`

---

## 设计概览

复用更新服务端基础设施（双域名 `UPDATE_API_HOSTS`）。客户端 `get_announcements` 双域名竞速请求 `GET /api/announcements?type=client`，随启动的 `check_update` 一并拉取。服务端按 `type` 过滤对应渠道的公告（桌面端固定用 `client`）。过滤有效期/排序/已读仍在前端。

## HTTP 接口

`GET {host}/api/announcements?type=client` → `200` + JSON 数组（`Announcement[]`）。无鉴权。

- **`type`（必带）**：渠道过滤。桌面客户端固定传 `type=client`。
  - ⚠️ **不带 `type` 会返回空数组 `[]`**（v0.3.4 早期客户端未带此参数，导致铃铛始终为空）。
  - `type=all` 需鉴权（返回 `{"error":"unauthorized"}`），前端不使用。

## 数据结构

```jsonc
[
  {
    "id": "2026-07-10-maintenance",        // 稳定唯一 id，前端据此记已读
    "title": "服务器维护通知",
    "body": "## 维护时间\n本周六 02:00–04:00…",  // Markdown
    "level": "normal",                      // "normal" | "important"
    "pinned": true,                         // 可选，置顶：排最前 + 标题旁渲染「置顶」tag。缺省=false
    "sortOrder": 0,                         // 可选，排序权重，越小越靠前；缺省视为很大
    "publishedAt": "2026-07-10T08:00:00Z",  // ISO8601 UTC
    "startAt": "2026-07-10T00:00:00Z",      // 可选，缺省=立即生效
    "endAt": "2026-07-13T00:00:00Z"         // 可选，缺省=永不过期
  }
]
```

Rust 结构（`#[serde(rename_all = "camelCase")]`，`startAt/endAt/pinned/sortOrder` 用 `#[serde(default)]`）与前端 TS 类型一一对应。

## 竞速与降级

`get_announcements` 用 `tokio::select!` 同时请求两个 host：
- **首个成功者赢**，另一个 `abort()`。
- 首个完成者失败（网络错误 / 非 2xx / 反序列化失败）→ **等另一个**。
- 两者都失败 → **返回空列表 `[]`**（无公告不是错误状态，静默不打扰）。
- ⚠️ 与 `check_update` 不同：公告**无 GitHub 兜底**。

## 客户端职责

- 过滤有效期窗口外公告（`now < startAt` 或 `now >= endAt`）。
- 排序：**置顶（`pinned`）优先** → `sortOrder` 升序 → `publishedAt` 倒序。置顶项标题旁渲染「置顶」tag。
- 已读：localStorage `announcements_read_ids`，仅本地。
- `level === "important"` 且未读 → 启动弹窗；其余仅铃铛面板。

## 内容维护（服务端，维护者自理）

编辑本地 `announcements.json` → `scp` 上传到服务器（`FILTER_MANAGE_SSH_TARGET`，见 `.env.local`）→ 服务端读该文件对外返回。本次不做写接口/管理界面。
