# 自定义更新流程 - 前端交接

> **状态：✅ 前端已实现** — `useUpdater.ts` + `UpdateModal.tsx` 已重写完成，待联调。配套 [plan](../plans/custom-update-flow.md)。Rust 命令详细规格见 [`.docs/api/updater.md`](../api/updater.md)。

---

## 后端完成状态（给前端的上下文）

### Rust 客户端（Phase 2 ✅）

**已完成的工作：**

- 新增 `src-tauri/src/updater.rs` 模块，4 个 Tauri 命令 + 4 个事件，编译通过
- `check_update` — GET 服务端 API 获取版本信息 + 镜像列表；**服务端不可达时自动降级到 `tauri-plugin-updater` 直连 GitHub**
- `download_update` — reqwest 流式下载到临时文件，每个 chunk emit `update://progress`，支持取消（`AtomicBool` 标志）+ 并发防护
- `cancel_update_download` — 设置取消标志，下个 chunk 边界中断、删临时文件、emit `update://cancelled`
- `install_update` — 校验通过后启动 NSIS（Passive 模式 `/P /R /UPDATE`），`std::process::exit(0)`
- minisign 签名校验链路与 `tauri-plugin-updater` v2.10.1 源码完全一致（base64 decode → `PublicKey::decode` → `Signature::decode` → `verify`）

**新增 Cargo 依赖：**
- `reqwest` 0.13（features: `stream`, `rustls`）
- `minisign-verify` 0.2
- `futures-util` 0.3
- `tokio` 1（features: `fs`, `io-util`）

**`lib.rs` 变更：**
- `.manage(updater::UpdaterState::default())`
- `invoke_handler` 注册 4 个命令

### 服务端（Phase 1 ✅）

**部署信息：**

| 项目 | 值 |
|------|-----|
| 技术栈 | Spring Boot 3.3.6 + Java 17，多阶段 Docker 构建 |
| 容器 | `filter-manage-api`，端口 9877，加入 `1panel-network` |
| 公网地址 | `https://filter-manage-api.xyls.us.kg` |
| Tunnel 路由 | `/upload` → 原有 webhook，其余 → Java 服务 |
| 镜像源 | 7 个（6 个可用 gh-proxy + GitHub 原始），实测筛选自 13 个 |
| 存储 | JSON 文件（`/opt/filter-manage-api/latest.json`） |

**两个接口：**
- `GET /api/check-update?current=x.y.z` — 公开只读，返回版本 + 7 个镜像 URL
- `POST /api/internal/release` — CI 推送，`X-Release-Secret` 鉴权

**GitHub Secrets（CI 用，Phase 4 需要）：**

| Secret | 值 |
|--------|-----|
| `UPDATE_API_URL` | `https://filter-manage-api.xyls.us.kg/api/internal/release` |
| `UPDATE_RELEASE_SECRET` | `e5a4ab36b63e762e93a7108a3ce77cb6288a6921b1495bcae6da0fc81d5efb2c` |

### 降级机制（前端需了解）

Rust `check_update` 的降级对前端透明：
- 服务端正常 → 返回服务端下发的 7 个镜像
- 服务端不可达 → 回退 `tauri-plugin-updater` 直连 GitHub，`mirrors` 只有一个「GitHub 直连」条目
- 前端代码无需区分两种情况，拿到的 `UpdateInfo` 结构一致

---

## 前端需要做的事

把现有 `useUpdater.ts`（依赖 `@tauri-apps/plugin-updater` 的 `check()` / `downloadAndInstall()`）**整体重写**为：调 Rust 自定义命令 + 监听 Rust 事件 + 前端测速 + 慢速换源 UI。

**前端不再直接下载**——下载在 Rust，前端只负责编排和 UI。

---

## 后端契约（来自 `.docs/api/updater.md`）

### TS 类型

```typescript
// src/hooks/useUpdater.ts
export interface Mirror { name: string; url: string }
export interface UpdateInfo {
  hasUpdate: boolean;
  version: string;
  notes: string;        // markdown
  signature: string;    // 前端不用关心，透传在 Rust 内部
  mirrors: Mirror[];    // 按推荐顺序，[0] 为默认源
}
```

### 命令

| 命令 | 调用 | 说明 |
|------|------|------|
| `check_update` | `invoke<UpdateInfo>("check_update")` | 检查更新，返回版本+镜像 |
| `download_update` | `invoke("download_update", { url })` | 下载指定镜像（进度走事件） |
| `cancel_update_download` | `invoke("cancel_update_download")` | 取消当前下载 |
| `install_update` | `invoke("install_update")` | 安装并重启（仅 verified 后） |

### 事件（`@tauri-apps/api/event` 的 `listen`）

| 事件 | payload | 含义 |
|------|---------|------|
| `update://progress` | `{ downloaded: number; total: number }` | 下载进度（字节） |
| `update://verified` | — | 下载完成且校验通过 → 可安装 |
| `update://error` | `{ message: string }` | 失败 |
| `update://cancelled` | — | 取消完成 |

---

## 1. 重写 `src/hooks/useUpdater.ts`

要点：
- `status` 扩展：`"idle" | "checking" | "available" | "downloading" | "verifying" | "ready" | "done" | "error" | "cancelled"`
- 新增 `speed`（字节/秒）、`mirrors`、`currentMirror`、`showMirrorPrompt`
- 测速：按 `update://progress` 的 `downloaded` 增量 / 时间增量算平均速度；`elapsed > 120s || avgSpeed < 100*1024` → `showMirrorPrompt = true`
- 换源：`switchMirror(url)` = 取消 → 等 `update://cancelled` → 用新 url 重下
- 保留现有 30 天 snooze 逻辑

```typescript
import { useEffect, useRef, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface Mirror { name: string; url: string }
export interface UpdateInfo {
  hasUpdate: boolean;
  version: string;
  notes: string;
  signature: string;
  mirrors: Mirror[];
}

export type UpdateStatus =
  | "idle" | "checking" | "available"
  | "downloading" | "verifying" | "ready"
  | "done" | "error" | "cancelled";

const SNOOZE_KEY = "update_snooze_until";
const THIRTY_DAYS = 30 * 24 * 60 * 60 * 1000;
const SLOW_AFTER_MS = 120_000;       // 2 分钟
const SLOW_BELOW_BPS = 100 * 1024;   // 100 KB/s

function checkSnooze(): boolean {
  const v = localStorage.getItem(SNOOZE_KEY);
  return v ? Date.now() < parseInt(v, 10) : false;
}
function setSnooze(): void {
  localStorage.setItem(SNOOZE_KEY, String(Date.now() + THIRTY_DAYS));
}

export function useUpdater() {
  const [status, setStatus] = useState<UpdateStatus>("idle");
  const [version, setVersion] = useState("");
  const [body, setBody] = useState("");
  const [progress, setProgress] = useState(0);      // 0~100
  const [speed, setSpeed] = useState(0);            // 字节/秒
  const [mirrors, setMirrors] = useState<Mirror[]>([]);
  const [currentMirror, setCurrentMirror] = useState<string>("");
  const [showMirrorPrompt, setShowMirrorPrompt] = useState(false);
  const [error, setError] = useState("");
  const [snoozed, setSnoozed] = useState(() => checkSnooze());

  // 测速采样
  const startRef = useRef(0);
  const lastRef = useRef<{ t: number; bytes: number }>({ t: 0, bytes: 0 });

  // 自动检查（启动时）
  useEffect(() => {
    let cancelled = false;
    (async () => {
      if (checkSnooze()) { setSnoozed(true); return; }
      try {
        setStatus("checking");
        const info = await invoke<UpdateInfo>("check_update");
        if (cancelled) return;
        if (info.hasUpdate) {
          setVersion(info.version);
          setBody(info.notes || "");
          setMirrors(info.mirrors);
          setStatus("available");
        } else {
          setStatus("idle");
        }
      } catch (e) {
        if (!cancelled) { setError(String(e)); setStatus("error"); }
      }
    })();
    return () => { cancelled = true; };
  }, []);

  // 事件监听
  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];
    (async () => {
      unlisteners.push(await listen<{ downloaded: number; total: number }>(
        "update://progress", (e) => {
          const { downloaded, total } = e.payload;
          if (total > 0) setProgress(Math.round((downloaded / total) * 100));

          const now = Date.now();
          if (startRef.current === 0) {
            startRef.current = now;
            lastRef.current = { t: now, bytes: downloaded };
          }
          const dt = (now - lastRef.current.t) / 1000;
          if (dt >= 1) {
            const bps = (downloaded - lastRef.current.bytes) / dt;
            setSpeed(bps);
            lastRef.current = { t: now, bytes: downloaded };
            const elapsed = now - startRef.current;
            if (elapsed > SLOW_AFTER_MS || bps < SLOW_BELOW_BPS) {
              setShowMirrorPrompt(true);
            }
          }
        }));
      unlisteners.push(await listen("update://verified", async () => {
        setStatus("verifying");
        setStatus("ready");
        try { await invoke("install_update"); setStatus("done"); }
        catch (e) { setError(String(e)); setStatus("error"); }
      }));
      unlisteners.push(await listen<{ message: string }>("update://error", (e) => {
        setError(e.payload.message); setStatus("error");
      }));
      unlisteners.push(await listen("update://cancelled", () => {
        setStatus("available"); setProgress(0); setSpeed(0);
      }));
    })();
    return () => { unlisteners.forEach((u) => u()); };
  }, []);

  const startDownload = useCallback(async (url?: string) => {
    const target = url || mirrors[0]?.url;
    if (!target) return;
    setCurrentMirror(target);
    setShowMirrorPrompt(false);
    setProgress(0); setSpeed(0);
    startRef.current = 0;
    setStatus("downloading");
    try { await invoke("download_update", { url: target }); }
    catch (e) { setError(String(e)); setStatus("error"); }
  }, [mirrors]);

  const switchMirror = useCallback(async (url: string) => {
    await invoke("cancel_update_download");   // 等 update://cancelled 把 status 复位
    setTimeout(() => startDownload(url), 100); // 简单去抖；也可监听 cancelled 后再触发
  }, [startDownload]);

  const cancel = useCallback(async () => {
    await invoke("cancel_update_download");
    setShowMirrorPrompt(false);
  }, []);

  const dismiss = useCallback(() => { setStatus("idle"); }, []);
  const snooze = useCallback(() => {
    setSnooze(); setSnoozed(true); setStatus("idle");
  }, []);

  // 手动检查（关于面板用），返回结果
  const checkForUpdate = useCallback(async (): Promise<"available" | "latest" | "error"> => {
    try {
      setStatus("checking");
      const info = await invoke<UpdateInfo>("check_update");
      if (info.hasUpdate) {
        setVersion(info.version); setBody(info.notes || "");
        setMirrors(info.mirrors); setStatus("available");
        return "available";
      }
      setStatus("idle"); return "latest";
    } catch (e) { setError(String(e)); setStatus("error"); return "error"; }
  }, []);

  return {
    status, version, body, progress, speed, mirrors, currentMirror,
    showMirrorPrompt, error, snoozed,
    startDownload, switchMirror, cancel, dismiss, snooze, checkForUpdate,
  };
}
```

> **注**：`switchMirror` 这里用了简单 `setTimeout` 去抖；更稳妥是在 `update://cancelled` 监听里读一个「待切换 url」ref 后再触发 `startDownload`，避免取消未完成就重下。实现时择一即可。

---

## 2. 重写 `src/components/UpdateModal.tsx`

变更：
- 「立即更新」点击改调 `startDownload()`（不再是 `installUpdate`）
- 下载态：进度条 + **速度文字** + **取消按钮**
- `showMirrorPrompt` 时：展示镜像列表供选择（「下载过慢，换个源？」）
- 沿用现有 DESIGN token（`bg-surface-container`、`font-headline-sm`、`p-xl` 等）
- **每个元素加 `data-component` / `data-name`**（项目规范）

```tsx
import { useState } from "react";
import Markdown from "react-markdown";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useUpdater } from "../hooks/useUpdater";
import { Icon } from "./Icon";

const CHANGELOG_URL = "https://filter-manage.6ya.site/changelog.html";

function fmtSpeed(bps: number): string {
  if (bps <= 0) return "—";
  if (bps < 1024 * 1024) return `${(bps / 1024).toFixed(0)} KB/s`;
  return `${(bps / 1024 / 1024).toFixed(1)} MB/s`;
}

export default function UpdateModal() {
  const {
    status, version, body, progress, speed, mirrors, currentMirror,
    showMirrorPrompt, startDownload, switchMirror, cancel, dismiss, snooze,
  } = useUpdater();
  const [snoozeChecked, setSnoozeChecked] = useState(false);

  if (status === "idle" || status === "checking" || status === "error" || status === "cancelled")
    return null;

  // 下载 / 校验 / 安装态
  if (status === "downloading" || status === "verifying" || status === "ready" || status === "done") {
    const verifying = status === "verifying" || status === "ready";
    return (
      <div data-component="UpdateModal" data-name="downloading-overlay"
           className="fixed inset-0 z-[200] flex items-center justify-center bg-black/40 backdrop-blur-sm">
        <div data-component="UpdateModal" data-name="downloading-card"
             className="bg-surface-container/80 backdrop-blur-md border border-outline-variant/20 rounded-xl p-xl shadow-2xl shadow-black/40 w-[400px]">
          <div className="flex items-center gap-md mb-lg">
            <Icon name="download" className="text-[24px] text-primary" />
            <h2 className="font-headline-sm text-headline-sm text-on-surface">
              {verifying ? "正在校验并安装" : "正在更新"}
            </h2>
          </div>

          <div className="flex items-baseline justify-between mb-md">
            <p className="font-body-md text-on-surface-variant">正在下载 v{version}... {progress}%</p>
            {!verifying && (
              <span data-component="UpdateModal" data-name="speed"
                    className="font-label-sm text-label-sm text-on-surface-variant/70">{fmtSpeed(speed)}</span>
            )}
          </div>

          <div className="w-full bg-surface-variant/50 rounded-full h-2 mb-lg">
            <div className="bg-primary h-2 rounded-full transition-all duration-300"
                 style={{ width: `${verifying ? 100 : progress}%` }} />
          </div>

          {/* 换源提示 */}
          {showMirrorPrompt && !verifying && (
            <div data-component="UpdateModal" data-name="mirror-prompt"
                 className="mb-lg p-md rounded-lg bg-surface-variant/30 border border-outline-variant/20">
              <p className="font-label-md text-label-md text-on-surface mb-sm">下载过慢，换个源？</p>
              <div className="flex flex-col gap-xs">
                {mirrors.map((m) => (
                  <button key={m.url}
                          data-component="UpdateModal" data-name={`mirror-${m.name}`}
                          onClick={() => switchMirror(m.url)}
                          disabled={m.url === currentMirror}
                          className="flex items-center justify-between px-sm py-xs rounded text-label-sm font-label-sm text-on-surface-variant hover:bg-primary/10 hover:text-primary disabled:opacity-40 disabled:cursor-default transition-colors">
                    <span>{m.name}</span>
                    {m.url === currentMirror && <span className="text-label-sm">当前</span>}
                  </button>
                ))}
              </div>
            </div>
          )}

          {!verifying && (
            <button data-component="UpdateModal" data-name="cancel-download"
                    onClick={cancel}
                    className="w-full px-lg py-sm rounded-lg border border-outline-variant/50 text-on-surface-variant font-label-md text-label-md hover:bg-surface-variant/50 transition-all">
              取消
            </button>
          )}
        </div>
      </div>
    );
  }

  // available 态（发现新版本）
  const handleDismiss = () => (snoozeChecked ? snooze() : dismiss());

  return (
    <div data-component="UpdateModal" data-name="available-overlay"
         className="fixed inset-0 z-[200] flex items-center justify-center bg-black/40 backdrop-blur-sm">
      <div data-component="UpdateModal" data-name="available-card"
           className="bg-surface-container/80 backdrop-blur-md border border-outline-variant/20 rounded-xl p-xl shadow-2xl shadow-black/40 w-[400px] max-h-[80vh] flex flex-col">
        <div className="flex items-center gap-md mb-lg">
          <Icon name="system_update" className="text-[24px] text-primary" />
          <h2 className="font-headline-sm text-headline-sm text-on-surface">发现新版本</h2>
          <div className="flex-1" />
          <button data-component="UpdateModal" data-name="open-changelog"
                  onClick={() => openUrl(CHANGELOG_URL)}
                  className="flex items-center gap-xs px-sm py-xs rounded text-on-surface-variant hover:text-primary hover:bg-primary/10 transition-colors"
                  title="查看完整更新日志">
            <Icon name="open_in_new" className="text-[16px]" />
            <span className="font-label-sm text-label-sm">完整日志</span>
          </button>
        </div>

        <p className="font-body-md text-on-surface-variant mb-lg">
          新版本 <span className="text-primary font-medium">v{version}</span> 可用
        </p>

        {body && (
          <div className="mb-lg flex-1 min-h-0">
            <h3 className="font-label-md text-label-md text-on-surface-variant mb-sm">更新内容</h3>
            <div className="bg-surface-variant/30 rounded-lg p-md h-full max-h-[300px] overflow-y-auto">
              <div className="prose prose-invert prose-sm max-w-none text-on-surface-variant">
                <Markdown>{body}</Markdown>
              </div>
            </div>
          </div>
        )}

        <div className="flex gap-md mb-lg">
          <button data-component="UpdateModal" data-name="install-now"
                  onClick={() => startDownload()}
                  className="flex-1 px-lg py-sm rounded-lg bg-primary text-on-primary font-label-md text-label-md font-medium hover:opacity-90 active:scale-[0.98] transition-all">
            立即更新
          </button>
          <button data-component="UpdateModal" data-name="later"
                  onClick={handleDismiss}
                  className="flex-1 px-lg py-sm rounded-lg border border-outline-variant/50 text-on-surface-variant font-label-md text-label-md hover:bg-surface-variant/50 active:scale-[0.98] transition-all">
            稍后
          </button>
        </div>

        <label data-component="UpdateModal" data-name="snooze-toggle"
               className="flex items-center gap-sm cursor-pointer group">
          <input type="checkbox" checked={snoozeChecked}
                 onChange={(e) => setSnoozeChecked(e.target.checked)}
                 className="w-4 h-4 rounded border-outline-variant/50 bg-surface-variant/30 text-primary focus:ring-primary/50 focus:ring-offset-0" />
          <span className="font-label-md text-label-md text-on-surface-variant group-hover:text-on-surface transition-colors">
            30天不再提示
          </span>
        </label>
      </div>
    </div>
  );
}
```

---

## 行为说明

1. 启动自动 `check_update`；有新版 → 「发现新版本」弹窗（展示 notes + 镜像数）
2. 点「立即更新」→ `startDownload()` 用 `mirrors[0]` 下载，进度条 + 实时速度
3. 下载 >2 分钟或 <100KB/s → 弹「换个源？」镜像列表（7 个源可选）
4. 选其他源 → `cancel_update_download` → 用新 url 重下（进度归零重来）
5. 下载完 → Rust 校验 → `update://verified` → 自动 `install_update` → 应用重启
6. 任意阶段可「取消」

---

## 测试场景

1. **正常**：有新版 → 立即更新 → 进度跑满 → 重启到新版本
2. **慢速换源**：mock 慢镜像（或断开第一个源）→ 2 分钟后出现换源提示 → 换源后正常完成
3. **取消**：下载中点取消 → 回到「发现新版本」态，临时文件被删
4. **校验失败**：篡改下载文件 → `update://error` → 显示「签名校验失败」
5. **无更新**：`check_update` 返回 `hasUpdate:false` → 无 UI
6. **snooze**：勾选 30 天不再提示 → 重启应用不再弹
7. **降级**：断掉服务端 → `check_update` 回退 GitHub 直连，`mirrors` 只有「GitHub 直连」一个条目

## 注意事项

- 下载/校验/安装全在 Rust，前端只编排；前端**不碰**镜像下载，故无需改 `capabilities` / CSP
- `data-component` / `data-name` 为项目强制规范，新增元素都要带
- 遵循 `DESIGN.md`（暗色 + 既有 token），不要引入新配色
- `@tauri-apps/plugin-updater` 的 `check()` / `downloadAndInstall()` 不再使用，但**插件本身保留**（Rust 降级路径依赖它）

## 实现改进（相比参考代码）

实际实现时对交接文档的参考代码做了两处改进：

1. **`switchMirror` 改用 ref 模式**（代替 `setTimeout(100)`）：取消后待切换的 url 存入 `pendingMirrorRef`，在 `update://cancelled` 事件监听里读取并自动重下，避免竞态
2. **`update://verified` 加 800ms 延迟**：让用户短暂看到"正在校验"状态，再进入"正在安装"，而不是两个 `setStatus` 连调被 React 批量更新吞掉

---

**关联**：[plan](../plans/custom-update-flow.md) · [Rust API](../api/updater.md) · [服务端交接](./custom-update-server.md)
