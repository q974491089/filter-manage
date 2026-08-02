> 📦 已归档 2026-08-02 · 仅供历史追溯，非当前开发依据

# 公告功能 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 给应用加一个「公告」能力——维护者不发版即可向所有用户推送通知，用户在顶栏铃铛看历史公告并区分已读，重要公告启动弹窗强触达。

**Architecture:** 复用现有更新服务端（双域名 `UPDATE_API_HOSTS` + Cloudflare Tunnel）。客户端新增 `get_announcements` 命令双域名竞速拉取，随启动的 `check_update` 一并完成；服务端只读透传 `announcements.json`（维护者 scp 上传，无鉴权）；过滤/排序/已读全在前端。前端 hook + 铃铛/面板/弹窗组件，复用 `UpdateModal` 的视觉语言与 `react-markdown` 渲染路径。

**Tech Stack:** Rust/Tauri（`reqwest` + `tokio::select!`）、React/TS、Tailwind（DESIGN.md tokens）、react-markdown。

---

## ⚠️ 关于测试（重要，先读）

**本项目没有任何自动化测试框架**（无 vitest/jest，Rust 无单测惯例），既有约定是「`npm run build` 类型门禁 + `cargo check` 编译门禁 + 手动运行验证」。本计划**遵循该约定**，不擅自引入测试框架（那是独立决策 + 范围蔓延）。因此：

- 纯逻辑（有效期过滤、排序、已读计算）被**抽成独立纯函数模块** `src/lib/announcements.ts`，边界清晰、将来可测。
- 每个任务的「验证」= 明确的构建/类型检查命令 + 明确的手动检查步骤，而非 `pytest` 式断言。
- 末尾提供**可选** Task 9（引入 vitest 只测纯逻辑模块），用户可自行决定是否采纳；核心计划不依赖它。

**环境约束**（见 `WORKFLOW.md` / 项目记忆）：
- 前端 `npm run build` / `npm run dev` 可在 WSL 跑。
- **Rust 编译/构建必须在 Windows PowerShell**，WSL 仅编辑。`cargo check` 用：
  `/mnt/c/Windows/System32/cmd.exe /c "C:\Users\myuser\Projects\filter-manage\src-tauri\cargo_check.bat"`
- 浏览器调试用 `192.168.160.1:5173`，不用 localhost。

---

## 数据契约（三端唯一真相）

`announcements.json`（服务端返回的 JSON 数组）字段结构，Rust 结构 / TS 类型 / 维护者手写 JSON 都对齐它：

```jsonc
// GET /api/announcements  →  Announcement[]
[
  {
    "id": "2026-07-10-maintenance",        // 稳定唯一 id，前端据此记已读
    "title": "服务器维护通知",
    "body": "## 维护时间\n本周六 02:00–04:00 服务短暂不可用。",  // Markdown
    "level": "normal",                      // "normal" | "important"
    "publishedAt": "2026-07-10T08:00:00Z",  // ISO8601 UTC，前端排序用
    "startAt": "2026-07-10T00:00:00Z",      // 可选；缺省=立即生效
    "endAt": "2026-07-13T00:00:00Z"         // 可选；缺省=永不过期
  }
]
```

**行为约定**：
- 服务端只透传文件，**不做过滤/排序**。
- 客户端过滤有效期窗口外的公告（`now < startAt` 或 `now >= endAt`），按 `publishedAt` 倒序。
- 双域名竞速：**首个成功者赢**；首个完成者失败则等另一个；两者都失败 → **返回空列表**（无公告不是错误，静默不打扰，无 GitHub 兜底——这点与 `check_update` 不同）。
- 已读：localStorage `announcements_read_ids`（id 数组），仅本地、不跨设备。

---

## File Structure（先锁定分解）

**新建：**
- `src-tauri/src/announcements.rs` — `Announcement` 结构 + `get_announcements` 命令（双域名竞速，容错空列表）。
- `src/lib/announcements.ts` — 纯逻辑：TS 类型 + `isActive` / `activeSorted` + 已读 localStorage 读写。**无 React 依赖，边界干净。**
- `src/hooks/useAnnouncements.ts` — 启动拉取、已读状态、未读数、重要公告弹窗队列。仿 `useUpdater` 的启动 useEffect 模式。
- `src/components/AnnouncementModal.tsx` — 重要公告启动弹窗，复用 `UpdateModal` 视觉（600px 卡片 + prose Markdown）。
- `src/components/AnnouncementBell.tsx` — 顶栏铃铛按钮 + 未读红点 + 内嵌下拉面板（含面板列表，封装 click-outside）。

**修改：**
- `src-tauri/src/updater.rs:14` — `UPDATE_API_HOSTS` 改 `pub(crate)`，供 announcements 复用（DRY，不复制常量）。
- `src-tauri/src/lib.rs` — `mod announcements;` + `invoke_handler` 注册 `get_announcements`。
- `src/App.tsx` — 引入 `useAnnouncements`，header-actions 放 `<AnnouncementBell>`，顶部渲染 `<AnnouncementModal>`。
- `.docs/api/announcements.md` — 新建契约文档（`.docs/api/` 记录 Rust/Tauri 命令 API，与 updater.md 同级）。

**分工提示（Tier 1 子 Agent）**：Task 2（Rust/后端）与 Task 3–7（前端）靠数据契约解耦，可并行。执行时按 `.rules/subagent-dispatch.md`：主 Agent 派 backend 子 Agent 做 Task 2、frontend 子 Agent 做 Task 3–7，主 Agent 汇总跑构建验证。服务端由维护者自理，不产出交接文档。

---

### Task 1: 数据契约文档

**Files:**
- Create: `.docs/api/announcements.md`

- [ ] **Step 1: 写契约文档**

创建 `.docs/api/announcements.md`，内容：

````markdown
# Announcements API（公告功能）

> **状态：🚧 实现中** — 本文档是公告功能三端共享的数据契约。plan: `.docs/plans/2026-07-10-announcements.md`，PRD: `.docs/prd/2026-07-10-announcements.md`。

模块：`src-tauri/src/announcements.rs`
注册：`src-tauri/src/lib.rs`（`invoke_handler`）
前端消费：`src/lib/announcements.ts`、`src/hooks/useAnnouncements.ts`

---

## 设计概览

复用更新服务端基础设施（双域名 `UPDATE_API_HOSTS`）。客户端 `get_announcements` 双域名竞速请求 `GET /api/announcements`，随启动的 `check_update` 一并拉取。服务端只透传 `announcements.json`（维护者 scp 上传，**无鉴权**——公告本就公开，且唯一写路径 scp 已由 SSH 密钥鉴权）。过滤/排序/已读全在前端。

## HTTP 接口

`GET {host}/api/announcements` → `200` + JSON 数组（`Announcement[]`）。无鉴权。

## 数据结构

```jsonc
[
  {
    "id": "2026-07-10-maintenance",        // 稳定唯一 id，前端据此记已读
    "title": "服务器维护通知",
    "body": "## 维护时间\n本周六 02:00–04:00…",  // Markdown
    "level": "normal",                      // "normal" | "important"
    "publishedAt": "2026-07-10T08:00:00Z",  // ISO8601 UTC
    "startAt": "2026-07-10T00:00:00Z",      // 可选，缺省=立即生效
    "endAt": "2026-07-13T00:00:00Z"         // 可选，缺省=永不过期
  }
]
```

Rust 结构（`#[serde(rename_all = "camelCase")]`，`startAt/endAt` 用 `#[serde(default)] Option<String>`）与前端 TS 类型一一对应。

## 竞速与降级

`get_announcements` 用 `tokio::select!` 同时请求两个 host：
- **首个成功者赢**，另一个 `abort()`。
- 首个完成者失败（网络错误 / 非 2xx / 反序列化失败）→ **等另一个**。
- 两者都失败 → **返回空列表 `[]`**（无公告不是错误状态，静默不打扰）。
- ⚠️ 与 `check_update` 不同：公告**无 GitHub 兜底**。

## 客户端职责

- 过滤有效期窗口外公告（`now < startAt` 或 `now >= endAt`）。
- 按 `publishedAt` 倒序。
- 已读：localStorage `announcements_read_ids`，仅本地。
- `level === "important"` 且未读 → 启动弹窗；其余仅铃铛面板。

## 内容维护（服务端，维护者自理）

编辑本地 `announcements.json` → `scp` 上传到服务器（`FILTER_MANAGE_SSH_TARGET`，见 `.env.local`）→ 服务端读该文件对外返回。本次不做写接口/管理界面。
````

- [ ] **Step 2: 提交**

```bash
git add .docs/api/announcements.md
git commit -m "docs(api): 新增公告功能数据契约文档"
```

---

### Task 2: Rust — `get_announcements` 命令（后端，可与前端并行）

**Files:**
- Create: `src-tauri/src/announcements.rs`
- Modify: `src-tauri/src/updater.rs:14`（`UPDATE_API_HOSTS` → `pub(crate)`）
- Modify: `src-tauri/src/lib.rs`（`mod` + `invoke_handler`）

- [ ] **Step 1: 让 `UPDATE_API_HOSTS` 可跨模块复用**

`src-tauri/src/updater.rs` 第 14 行，把：
```rust
const UPDATE_API_HOSTS: &[&str] = &[
```
改为：
```rust
pub(crate) const UPDATE_API_HOSTS: &[&str] = &[
```
（仅加 `pub(crate)`，数组内容不动。）

- [ ] **Step 2: 写 `announcements.rs`**

创建 `src-tauri/src/announcements.rs`：

```rust
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::updater::UPDATE_API_HOSTS;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Announcement {
    pub id: String,
    pub title: String,
    pub body: String,
    /// "normal" | "important"
    pub level: String,
    /// ISO8601 UTC，前端排序用
    pub published_at: String,
    #[serde(default)]
    pub start_at: Option<String>,
    #[serde(default)]
    pub end_at: Option<String>,
}

/// 双域名竞速拉取公告。
///
/// 语义（区别于 updater::check_update）：
/// - 首个成功者赢，abort 另一个；
/// - 首个完成者失败 → 等另一个；
/// - 两者都失败 → 返回空列表（无公告不是错误，静默不打扰，无 GitHub 兜底）。
#[tauri::command]
pub async fn get_announcements() -> Result<Vec<Announcement>, String> {
    let host_a = UPDATE_API_HOSTS[0];
    let host_b = UPDATE_API_HOSTS[1];

    let client = match reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(_) => return Ok(Vec::new()),
    };

    let fetch = |host: &str| {
        let client = client.clone();
        let url = format!("{host}/api/announcements");
        async move {
            let resp = client.get(&url).send().await.ok()?;
            if !resp.status().is_success() {
                return None;
            }
            resp.json::<Vec<Announcement>>().await.ok()
        }
    };

    let mut fut_a = tokio::spawn(fetch(host_a));
    let mut fut_b = tokio::spawn(fetch(host_b));

    let list: Option<Vec<Announcement>> = tokio::select! {
        a = &mut fut_a => {
            match a.ok().flatten() {
                Some(v) => { fut_b.abort(); Some(v) }
                None => fut_b.await.ok().flatten(),
            }
        }
        b = &mut fut_b => {
            match b.ok().flatten() {
                Some(v) => { fut_a.abort(); Some(v) }
                None => fut_a.await.ok().flatten(),
            }
        }
    };

    Ok(list.unwrap_or_default())
}
```

- [ ] **Step 3: 在 `lib.rs` 注册模块与命令**

`src-tauri/src/lib.rs` 顶部模块声明区（`mod updater;` 附近，第 7 行后）加一行：
```rust
mod announcements;
```

`invoke_handler` 的 `generate_handler!` 列表里，`updater::install_update,`（第 122 行）之后加一行：
```rust
            announcements::get_announcements,
```

（`get_announcements` 无共享状态，**不需要** `.manage(...)`。）

- [ ] **Step 4: 编译验证（Windows）**

Run:
```bash
/mnt/c/Windows/System32/cmd.exe /c "C:\Users\myuser\Projects\filter-manage\src-tauri\cargo_check.bat"
```
Expected: 编译通过，无 error（允许 `Announcement` 未被 Rust 侧读取字段的 dead_code 警告——字段经 serde 序列化给前端，可忽略或按需 `#[allow(dead_code)]`）。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/announcements.rs src-tauri/src/updater.rs src-tauri/src/lib.rs
git commit -m "feat(announcements): 新增 get_announcements 命令（双域名竞速，容错空列表）"
```

---

### Task 3: 前端纯逻辑模块 + TS 类型（前端）

**Files:**
- Create: `src/lib/announcements.ts`

- [ ] **Step 1: 写纯逻辑模块**

创建 `src/lib/announcements.ts`：

```ts
// 公告纯逻辑：类型 + 有效期过滤 + 排序 + 已读 localStorage 读写。
// 无 React 依赖，便于将来单测。字段与 src-tauri/src/announcements.rs 一一对应。

export type AnnouncementLevel = "normal" | "important";

export interface Announcement {
  id: string;
  title: string;
  body: string;                 // Markdown
  level: AnnouncementLevel;
  publishedAt: string;          // ISO8601 UTC
  startAt?: string | null;      // 可选，缺省=立即生效
  endAt?: string | null;        // 可选，缺省=永不过期
}

/** 是否在有效期窗口内（startAt/endAt 缺省或无法解析视为不限） */
export function isActive(a: Announcement, nowMs: number): boolean {
  if (a.startAt) {
    const s = Date.parse(a.startAt);
    if (!Number.isNaN(s) && nowMs < s) return false;
  }
  if (a.endAt) {
    const e = Date.parse(a.endAt);
    if (!Number.isNaN(e) && nowMs >= e) return false;
  }
  return true;
}

/** 过滤有效 + 按 publishedAt 倒序（最新在前） */
export function activeSorted(list: Announcement[], nowMs: number): Announcement[] {
  return list
    .filter((a) => isActive(a, nowMs))
    .slice()
    .sort((a, b) => Date.parse(b.publishedAt) - Date.parse(a.publishedAt));
}

const READ_KEY = "announcements_read_ids";

/** 读已读 id 集合，坏数据容错为空集 */
export function loadReadIds(): Set<string> {
  try {
    const raw = localStorage.getItem(READ_KEY);
    if (!raw) return new Set();
    const arr = JSON.parse(raw);
    return Array.isArray(arr) ? new Set(arr.map(String)) : new Set();
  } catch {
    return new Set();
  }
}

export function saveReadIds(ids: Set<string>): void {
  localStorage.setItem(READ_KEY, JSON.stringify([...ids]));
}
```

- [ ] **Step 2: 类型检查**

Run: `npm run build`
Expected: `tsc` 通过、vite 构建成功，无类型错误。

- [ ] **Step 3: 提交**

```bash
git add src/lib/announcements.ts
git commit -m "feat(announcements): 前端纯逻辑模块（类型/有效期过滤/排序/已读存储）"
```

---

### Task 4: 前端 hook `useAnnouncements`（前端）

**Files:**
- Create: `src/hooks/useAnnouncements.ts`

- [ ] **Step 1: 写 hook**

创建 `src/hooks/useAnnouncements.ts`：

```ts
import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  type Announcement,
  activeSorted,
  loadReadIds,
  saveReadIds,
} from "../lib/announcements";

export function useAnnouncements() {
  const [items, setItems] = useState<Announcement[]>([]);
  const [readIds, setReadIds] = useState<Set<string>>(() => loadReadIds());
  const [panelOpen, setPanelOpen] = useState(false);
  const [modalItems, setModalItems] = useState<Announcement[]>([]);

  // 启动拉取（随 check_update 一并完成；失败静默）
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const raw = await invoke<Announcement[]>("get_announcements");
        if (cancelled) return;
        const active = activeSorted(raw, Date.now());
        setItems(active);
        const read = loadReadIds();
        const importantUnread = active.filter(
          (a) => a.level === "important" && !read.has(a.id)
        );
        if (importantUnread.length > 0) setModalItems(importantUnread);
      } catch (e) {
        console.error("[Announcements] fetch failed:", e);
        // 静默：无公告即可，不打扰用户
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const isRead = useCallback((id: string) => readIds.has(id), [readIds]);

  const unreadCount = items.reduce(
    (n, a) => (readIds.has(a.id) ? n : n + 1),
    0
  );

  const markRead = useCallback((id: string) => {
    setReadIds((prev) => {
      if (prev.has(id)) return prev;
      const next = new Set(prev);
      next.add(id);
      saveReadIds(next);
      return next;
    });
  }, []);

  const markAllRead = useCallback(() => {
    setReadIds((prev) => {
      const next = new Set(prev);
      items.forEach((a) => next.add(a.id));
      saveReadIds(next);
      return next;
    });
  }, [items]);

  // 关闭重要公告弹窗：把弹窗内的公告标记已读
  const dismissModal = useCallback(() => {
    setModalItems((cur) => {
      setReadIds((prev) => {
        const next = new Set(prev);
        cur.forEach((a) => next.add(a.id));
        saveReadIds(next);
        return next;
      });
      return [];
    });
  }, []);

  return {
    items,
    unreadCount,
    isRead,
    panelOpen,
    setPanelOpen,
    markRead,
    markAllRead,
    modalItems,
    dismissModal,
  };
}
```

- [ ] **Step 2: 类型检查**

Run: `npm run build`
Expected: 通过（此时 hook 未被引用，仅验证类型自洽）。

- [ ] **Step 3: 提交**

```bash
git add src/hooks/useAnnouncements.ts
git commit -m "feat(announcements): useAnnouncements hook（启动拉取/已读/未读数/重要弹窗队列）"
```

---

### Task 5: 重要公告启动弹窗 `AnnouncementModal`（前端）

**Files:**
- Create: `src/components/AnnouncementModal.tsx`

复用 `UpdateModal` 的 available 卡片视觉（600px 宽、glass 面板、prose Markdown）。

- [ ] **Step 1: 写组件**

创建 `src/components/AnnouncementModal.tsx`：

```tsx
import Markdown from "react-markdown";
import type { Announcement } from "../lib/announcements";
import { Icon } from "./Icon";

function fmtDate(iso: string): string {
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return "";
  return new Date(t).toLocaleDateString();
}

export default function AnnouncementModal({
  items,
  onDismiss,
}: {
  items: Announcement[];
  onDismiss: () => void;
}) {
  if (items.length === 0) return null;

  return (
    <div
      data-component="AnnouncementModal"
      data-name="announcement-overlay"
      className="fixed inset-0 z-[200] flex items-center justify-center bg-black/40 backdrop-blur-sm"
    >
      <div
        data-component="AnnouncementModal"
        data-name="announcement-card"
        className="bg-surface-container/80 backdrop-blur-md border border-outline-variant/20 rounded-xl p-xl shadow-2xl shadow-black/40 w-[600px] max-h-[85vh] flex flex-col"
      >
        <div className="flex items-center gap-md mb-lg">
          <Icon name="notifications" filled className="text-[24px] text-primary" />
          <h2 className="font-headline-sm text-headline-sm text-on-surface">重要公告</h2>
          <div className="flex-1" />
        </div>

        <div className="flex-1 min-h-0 overflow-y-auto flex flex-col gap-lg">
          {items.map((a) => (
            <div key={a.id} data-component="AnnouncementModal" data-name="announcement-item">
              <div className="flex items-baseline gap-sm mb-sm">
                <h3 className="font-title-sm text-title-sm text-on-surface">{a.title}</h3>
                <span className="font-label-sm text-label-sm text-on-surface-variant/60">
                  {fmtDate(a.publishedAt)}
                </span>
              </div>
              <div className="bg-surface-variant/30 rounded-lg p-md">
                <div className="prose prose-invert prose-sm max-w-none text-on-surface-variant">
                  <Markdown>{a.body}</Markdown>
                </div>
              </div>
            </div>
          ))}
        </div>

        <div className="flex justify-end mt-lg">
          <button
            data-component="AnnouncementModal"
            data-name="announcement-dismiss"
            onClick={onDismiss}
            className="px-lg py-sm rounded-lg bg-primary text-on-primary font-label-md text-label-md font-medium hover:opacity-90 active:scale-[0.98] transition-all"
          >
            我知道了
          </button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: 类型检查**

Run: `npm run build`
Expected: 通过。

- [ ] **Step 3: 提交**

```bash
git add src/components/AnnouncementModal.tsx
git commit -m "feat(announcements): 重要公告启动弹窗组件"
```

---

### Task 6: 顶栏铃铛 + 下拉面板 `AnnouncementBell`（前端）

**Files:**
- Create: `src/components/AnnouncementBell.tsx`

铃铛按钮 + 未读红点；点击展开下拉面板列出所有有效公告（Markdown），未读带圆点，点击未读项标记已读，面板头有「全部已读」。面板自带透明 backdrop 实现 click-outside。

- [ ] **Step 1: 写组件**

创建 `src/components/AnnouncementBell.tsx`：

```tsx
import Markdown from "react-markdown";
import type { Announcement } from "../lib/announcements";
import { Icon } from "./Icon";

function fmtDate(iso: string): string {
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return "";
  return new Date(t).toLocaleDateString();
}

export default function AnnouncementBell({
  items,
  unreadCount,
  isRead,
  open,
  setOpen,
  markRead,
  markAllRead,
}: {
  items: Announcement[];
  unreadCount: number;
  isRead: (id: string) => boolean;
  open: boolean;
  setOpen: (v: boolean) => void;
  markRead: (id: string) => void;
  markAllRead: () => void;
}) {
  return (
    <div data-component="AnnouncementBell" data-name="bell-wrapper" className="relative">
      <button
        data-component="AnnouncementBell"
        data-name="bell-button"
        onClick={() => setOpen(!open)}
        className="relative w-8 h-8 rounded-full flex items-center justify-center text-on-surface-variant hover:bg-primary/10 transition-colors"
        title="公告"
      >
        <Icon name="notifications" className="text-[20px]" />
        {unreadCount > 0 && (
          <span
            data-name="unread-badge"
            className="absolute -top-0.5 -right-0.5 min-w-[16px] h-4 px-1 rounded-full bg-error text-on-error text-[10px] leading-4 font-bold flex items-center justify-center"
          >
            {unreadCount > 9 ? "9+" : unreadCount}
          </span>
        )}
      </button>

      {open && (
        <>
          {/* click-outside backdrop */}
          <div
            data-name="bell-backdrop"
            className="fixed inset-0 z-[150]"
            onClick={() => setOpen(false)}
          />
          <div
            data-component="AnnouncementBell"
            data-name="bell-panel"
            className="absolute right-0 top-[calc(100%+8px)] z-[160] w-[380px] max-h-[70vh] flex flex-col bg-surface-container/95 backdrop-blur-md border border-outline-variant/20 rounded-xl shadow-2xl shadow-black/40 overflow-hidden"
          >
            <div className="flex items-center justify-between px-md py-sm border-b border-outline-variant/20">
              <span className="font-title-sm text-title-sm text-on-surface">公告</span>
              {unreadCount > 0 && (
                <button
                  data-name="mark-all-read"
                  onClick={markAllRead}
                  className="font-label-sm text-label-sm text-on-surface-variant hover:text-primary transition-colors"
                >
                  全部已读
                </button>
              )}
            </div>

            <div className="flex-1 overflow-y-auto">
              {items.length === 0 ? (
                <div data-name="bell-empty" className="px-md py-lg text-center font-label-md text-label-md text-on-surface-variant/60">
                  暂无公告
                </div>
              ) : (
                items.map((a) => {
                  const unread = !isRead(a.id);
                  return (
                    <button
                      key={a.id}
                      data-component="AnnouncementBell"
                      data-name="announcement-row"
                      onClick={() => unread && markRead(a.id)}
                      className="w-full text-left px-md py-sm border-b border-outline-variant/10 hover:bg-surface-variant/30 transition-colors"
                    >
                      <div className="flex items-center gap-sm mb-xs">
                        {unread && <span className="w-2 h-2 rounded-full bg-primary shrink-0" />}
                        {a.level === "important" && (
                          <span className="px-1.5 py-0.5 rounded bg-error/20 text-error font-label-sm text-[10px] leading-none">重要</span>
                        )}
                        <span className="font-title-sm text-title-sm text-on-surface truncate">{a.title}</span>
                        <span className="flex-1" />
                        <span className="font-label-sm text-label-sm text-on-surface-variant/60 shrink-0">{fmtDate(a.publishedAt)}</span>
                      </div>
                      <div className="prose prose-invert prose-sm max-w-none text-on-surface-variant/80 pl-0">
                        <Markdown>{a.body}</Markdown>
                      </div>
                    </button>
                  );
                })
              )}
            </div>
          </div>
        </>
      )}
    </div>
  );
}
```

- [ ] **Step 2: 类型检查**

Run: `npm run build`
Expected: 通过。

- [ ] **Step 3: 提交**

```bash
git add src/components/AnnouncementBell.tsx
git commit -m "feat(announcements): 顶栏铃铛 + 未读红点 + 下拉公告面板"
```

---

### Task 7: 接入 `App.tsx`（前端）

**Files:**
- Modify: `src/App.tsx`（import、header-actions、modal 渲染）

- [ ] **Step 1: 加 import**

`src/App.tsx` 第 13 行 `import ClosePromptModal ...` 之后加：
```tsx
import AnnouncementBell from "./components/AnnouncementBell";
import AnnouncementModal from "./components/AnnouncementModal";
```
第 16 行 `import { useUpdater } ...` 之后加：
```tsx
import { useAnnouncements } from "./hooks/useAnnouncements";
```

- [ ] **Step 2: 实例化 hook**

第 68 行 `const updater = useUpdater();` 之后加一行：
```tsx
  const ann = useAnnouncements();
```

- [ ] **Step 3: 顶部渲染重要公告弹窗**

第 434 行 `<UpdateModal updater={updater} />` 之后加：
```tsx
      <AnnouncementModal items={ann.modalItems} onDismiss={ann.dismissModal} />
```

- [ ] **Step 4: header-actions 放铃铛**

`src/App.tsx` 的 `data-name="header-actions"` 区块内，设置按钮（`data-name="settings-button"`）**之前**插入铃铛，并在其后加一条竖分隔线。即把设置按钮那段前面加：
```tsx
          <AnnouncementBell
            items={ann.items}
            unreadCount={ann.unreadCount}
            isRead={ann.isRead}
            open={ann.panelOpen}
            setOpen={ann.setPanelOpen}
            markRead={ann.markRead}
            markAllRead={ann.markAllRead}
          />

```
（放在现有 `<button data-name="settings-button" ...>` 这一行之前。铃铛与设置按钮同为圆形图标按钮，视觉自然并排。）

- [ ] **Step 5: 类型检查 + 构建**

Run: `npm run build`
Expected: `tsc` + vite 构建通过，无类型/引用错误。

- [ ] **Step 6: 手动验证 UI（用临时 mock，因服务端接口可能未就绪）**

在 `src/hooks/useAnnouncements.ts` 的启动 useEffect 里，**临时**把 `const raw = await invoke<Announcement[]>("get_announcements");` 替换为 mock：
```ts
        const raw: Announcement[] = [
          { id: "t1", title: "普通公告", body: "这是一条**普通**公告。", level: "normal", publishedAt: "2026-07-09T08:00:00Z" },
          { id: "t2", title: "重要维护", body: "## 维护\n本周六 02:00–04:00 维护。", level: "important", publishedAt: "2026-07-10T08:00:00Z" },
          { id: "t3", title: "已过期", body: "看不到我", level: "normal", publishedAt: "2026-01-01T00:00:00Z", endAt: "2026-02-01T00:00:00Z" },
        ];
```
Run: `npm run dev`，浏览器访问 `http://192.168.160.1:5173/`（WSL→Windows）。
Expected 手动核对：
- 启动即弹「重要公告」弹窗，含「重要维护」，不含 t1/t3；点「我知道了」关闭。
- 顶栏铃铛有红点、未读数为 2（t1+t2，t3 已过期被过滤）。
- 点铃铛展开面板：t2、t1 倒序（最新在前），t3 不出现；t2 带「重要」标签。
- 点某条未读 → 红点数减 1；点「全部已读」→ 红点消失。
- 刷新页面：已读状态保持（localStorage）。

- [ ] **Step 7: 还原 mock**

把 Step 6 的 mock 改回 `const raw = await invoke<Announcement[]>("get_announcements");`。
Run: `npm run build`
Expected: 通过。

> ⚠️ **不要提交 mock**。确认 diff 里 `useAnnouncements.ts` 已还原为 `invoke` 调用。

- [ ] **Step 8: 提交**

```bash
git add src/App.tsx
git commit -m "feat(announcements): 接入顶栏铃铛与重要公告弹窗"
```

---

### Task 8: 端到端验证 + 文档收尾

**Files:**
- Modify: `.docs/api/announcements.md`（状态 🚧→✅）

- [ ] **Step 1: 桌面端联调（Windows PowerShell）**

前置：维护者已把一个测试 `announcements.json`（含至少 1 条 normal + 1 条 important）scp 到服务端，`GET /api/announcements` 可访问。

Windows PowerShell 项目目录：
```powershell
npm run tauri dev
```
Expected 手动核对（真实接口）：
- 应用启动，重要公告弹窗出现；铃铛红点/未读数正确。
- 断网启动：无弹窗、铃铛无红点、无报错（`get_announcements` 返回空列表，静默）。
- 服务端删掉该测试公告后重启应用：铃铛「暂无公告」。

- [ ] **Step 2: 验证「不发版即可见」（PRD 核心验收）**

维护者改 `announcements.json` 加一条新 normal 公告 → scp 上传 → **不重新构建客户端**，仅重启应用 → 新公告出现在铃铛面板。

- [ ] **Step 3: 更新契约文档状态**

`.docs/api/announcements.md` 顶部 `> **状态：🚧 实现中**` 改为 `> **状态：✅ 已实现**`。

- [ ] **Step 4: 提交**

```bash
git add .docs/api/announcements.md
git commit -m "docs(api): 公告功能实现完成，契约状态置为已实现"
```

---

### Task 9（可选）: 为纯逻辑模块引入 vitest

> 仅当用户明确想要自动化测试时做。项目当前无测试框架，这是一次独立的基础设施决策。

**Files:**
- Modify: `package.json`（devDep + `test` script）
- Create: `src/lib/announcements.test.ts`

- [ ] **Step 1: 装 vitest（Windows PowerShell）**

```powershell
npm i -D vitest
```
`package.json` scripts 加：`"test": "vitest run"`。

- [ ] **Step 2: 写纯逻辑测试**

创建 `src/lib/announcements.test.ts`：

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { isActive, activeSorted, loadReadIds, saveReadIds, type Announcement } from "./announcements";

const base: Announcement = { id: "x", title: "t", body: "b", level: "normal", publishedAt: "2026-07-10T00:00:00Z" };
const NOW = Date.parse("2026-07-10T12:00:00Z");

describe("isActive", () => {
  it("缺省窗口恒有效", () => expect(isActive(base, NOW)).toBe(true));
  it("未到 startAt 无效", () => expect(isActive({ ...base, startAt: "2026-07-11T00:00:00Z" }, NOW)).toBe(false));
  it("已过 endAt 无效", () => expect(isActive({ ...base, endAt: "2026-07-10T06:00:00Z" }, NOW)).toBe(false));
  it("窗口内有效", () => expect(isActive({ ...base, startAt: "2026-07-10T00:00:00Z", endAt: "2026-07-11T00:00:00Z" }, NOW)).toBe(true));
});

describe("activeSorted", () => {
  it("过滤过期并按 publishedAt 倒序", () => {
    const list: Announcement[] = [
      { ...base, id: "old", publishedAt: "2026-07-08T00:00:00Z" },
      { ...base, id: "expired", endAt: "2026-01-01T00:00:00Z" },
      { ...base, id: "new", publishedAt: "2026-07-10T00:00:00Z" },
    ];
    expect(activeSorted(list, NOW).map((a) => a.id)).toEqual(["new", "old"]);
  });
});

describe("readIds 存取", () => {
  beforeEach(() => localStorage.clear());
  it("往返一致", () => {
    saveReadIds(new Set(["a", "b"]));
    expect([...loadReadIds()].sort()).toEqual(["a", "b"]);
  });
  it("坏数据容错空集", () => {
    localStorage.setItem("announcements_read_ids", "not-json");
    expect(loadReadIds().size).toBe(0);
  });
});
```

- [ ] **Step 3: 跑测试**

Run: `npm test`（需 `environment: jsdom` 才有 localStorage — 若报错，`npm i -D jsdom` 并在文件顶部加 `// @vitest-environment jsdom`）。
Expected: 全绿。

- [ ] **Step 4: 提交**

```bash
git add package.json src/lib/announcements.test.ts package-lock.json
git commit -m "test(announcements): vitest 覆盖有效期过滤/排序/已读存储"
```

---

## Self-Review

**Spec coverage（对照 PRD 验收标准）：**
- [x] 读接口契约 → Task 1、Task 8
- [x] `get_announcements` 双域名竞速 + 空列表容错 → Task 2
- [x] 随启动拉取，不新增独立请求时机 → Task 4（hook 启动 useEffect，与 useUpdater 并列）
- [x] 铃铛 + 未读红点 → Task 6
- [x] 面板多条倒序 + Markdown → Task 6 + Task 3（activeSorted）
- [x] 重要未读启动弹窗，普通不弹 → Task 5 + Task 4（modalItems 仅收 important 未读）
- [x] 查看标记已读 + localStorage 持久化 → Task 4 + Task 3
- [x] 客户端有效期过滤/排序 → Task 3
- [x] 不发版即可见 → Task 8 Step 2
- [x] data-component/data-name + DESIGN.md → Task 5/6（组件均带属性、用 token 类）
- [x] 契约文档一致 → Task 1 + Task 8 Step 3

**Type consistency：** Rust `Announcement`(camelCase: id/title/body/level/publishedAt/startAt/endAt) ↔ TS `Announcement`(同名) ↔ 契约 JSON 三处一致。hook 暴露的 `items/unreadCount/isRead/panelOpen/setPanelOpen/markRead/markAllRead/modalItems/dismissModal` 与 Task 6/7 的 props、Task 5 的 `items/onDismiss` 一致。

**Placeholder scan：** 无 TBD/TODO；每个代码步骤含完整代码；每个验证步骤含确切命令与预期。

**已知风险回顾：**
- mock 泄漏风险 → Task 7 Step 7 显式还原 + 提交前查 diff。
- Rust dead_code 警告（字段仅 serde 用）→ Task 2 Step 4 已说明可忽略/`#[allow]`。
- 竞速误杀 → Task 2 用「首成功赢、首失败等另一个」修正，区别于 check_update。
