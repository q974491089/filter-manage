# Update Modal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 UpdateBanner 改为居中模态弹窗，添加 30 天静默期功能

**Architecture:** 使用 localStorage 存储静默期状态，useUpdater hook 管理更新检查和静默期逻辑，UpdateModal 组件负责 UI 展示

**Tech Stack:** React, TypeScript, Tailwind CSS, Tauri Updater Plugin

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/hooks/useUpdater.ts` | Modify | 添加 snooze 逻辑和静默期检查 |
| `src/components/UpdateModal.tsx` | Create | 居中模态弹窗组件 |
| `src/components/UpdateBanner.tsx` | Delete | 被 UpdateModal 替代 |
| `src/App.tsx` | Modify | UpdateBanner → UpdateModal |

---

### Task 1: 增强 useUpdater hook

**Files:**
- Modify: `src/hooks/useUpdater.ts`

- [ ] **Step 1: 添加静默期检查函数**

在 `src/hooks/useUpdater.ts` 文件顶部添加常量和工具函数：

```typescript
const SNOOZE_KEY = "update_snooze_until";
const THIRTY_DAYS = 30 * 24 * 60 * 60 * 1000;

function checkSnooze(): boolean {
  const snoozeUntil = localStorage.getItem(SNOOZE_KEY);
  if (!snoozeUntil) return false;
  return Date.now() < parseInt(snoozeUntil, 10);
}

function setSnooze(): void {
  localStorage.setItem(SNOOZE_KEY, String(Date.now() + THIRTY_DAYS));
}
```

- [ ] **Step 2: 修改 useUpdater hook 返回值和逻辑**

修改 `useUpdater` 函数，在返回值中添加 `snoozed` 状态和 `snooze` 函数：

```typescript
export function useUpdater() {
  const [status, setStatus] = useState<UpdateStatus>("idle");
  const [version, setVersion] = useState("");
  const [progress, setProgress] = useState(0);
  const [error, setError] = useState("");
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);
  const [snoozed, setSnoozed] = useState(() => checkSnooze());

  useEffect(() => {
    let cancelled = false;

    async function checkForUpdate() {
      // 如果在静默期，跳过检查
      if (checkSnooze()) {
        setSnoozed(true);
        return;
      }

      try {
        setStatus("checking");
        const update = await check();
        if (cancelled) return;

        if (update) {
          setVersion(update.version);
          setPendingUpdate(update);
          setStatus("available");
        } else {
          setStatus("idle");
        }
      } catch (e) {
        if (cancelled) return;
        setError(String(e));
        setStatus("error");
      }
    }

    checkForUpdate();
    return () => { cancelled = true; };
  }, []);

  async function installUpdate() {
    if (!pendingUpdate) return;
    try {
      setStatus("downloading");
      let totalLen = 0;
      let downloaded = 0;

      await pendingUpdate.downloadAndInstall((event) => {
        if (event.event === "Started" && event.data.contentLength) {
          totalLen = event.data.contentLength;
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          if (totalLen > 0) setProgress(Math.round((downloaded / totalLen) * 100));
        } else if (event.event === "Finished") {
          setStatus("done");
        }
      });

      await relaunch();
    } catch (e) {
      setError(String(e));
      setStatus("error");
    }
  }

  function dismiss() {
    setStatus("idle");
    setPendingUpdate(null);
  }

  function snooze() {
    setSnooze();
    setSnoozed(true);
    setStatus("idle");
    setPendingUpdate(null);
  }

  return { status, version, progress, error, snoozed, installUpdate, dismiss, snooze };
}
```

- [ ] **Step 3: 验证 TypeScript 编译**

运行类型检查确认无错误：

```bash
cd /mnt/c/Users/myuser/Projects/filter-manage && npx tsc --noEmit
```

Expected: 无错误输出

- [ ] **Step 4: Commit**

```bash
git add src/hooks/useUpdater.ts
git commit -m "feat(updater): add snooze logic and 30-day cooldown support"
```

---

### Task 2: 创建 UpdateModal 组件

**Files:**
- Create: `src/components/UpdateModal.tsx`

- [ ] **Step 1: 创建 UpdateModal 组件**

创建 `src/components/UpdateModal.tsx`，实现居中模态弹窗：

```tsx
import { useState } from "react";
import { useUpdater } from "../hooks/useUpdater";

export default function UpdateModal() {
  const { status, version, progress, error, installUpdate, dismiss, snooze } = useUpdater();
  const [snoozeChecked, setSnoozeChecked] = useState(false);

  // 不显示弹窗的状态
  if (status === "idle" || status === "checking" || status === "error") return null;

  // 下载中状态
  if (status === "downloading" || status === "done") {
    return (
      <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/40 backdrop-blur-sm">
        <div className="bg-surface-container/80 backdrop-blur-md border border-outline-variant/20 rounded-xl p-xl shadow-2xl shadow-black/40 w-[360px]">
          <div className="flex items-center gap-md mb-lg">
            <span className="material-symbols-outlined text-[24px] text-primary">download</span>
            <h2 className="font-headline-sm text-headline-sm text-on-surface">正在更新</h2>
          </div>
          <p className="font-body-md text-on-surface-variant mb-md">
            正在下载 v{version}... {progress}%
          </p>
          <div className="w-full bg-surface-variant/50 rounded-full h-2">
            <div
              className="bg-primary h-2 rounded-full transition-all duration-300"
              style={{ width: `${progress}%` }}
            />
          </div>
        </div>
      </div>
    );
  }

  // 更新可用状态
  const handleDismiss = () => {
    if (snoozeChecked) {
      snooze();
    } else {
      dismiss();
    }
  };

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/40 backdrop-blur-sm">
      <div className="bg-surface-container/80 backdrop-blur-md border border-outline-variant/20 rounded-xl p-xl shadow-2xl shadow-black/40 w-[360px]">
        {/* Header */}
        <div className="flex items-center gap-md mb-lg">
          <span className="material-symbols-outlined text-[24px] text-primary">system_update</span>
          <h2 className="font-headline-sm text-headline-sm text-on-surface">发现新版本</h2>
        </div>

        {/* Version Info */}
        <p className="font-body-md text-on-surface-variant mb-lg">
          新版本 <span className="text-primary font-medium">v{version}</span> 可用
        </p>

        {/* Actions */}
        <div className="flex gap-md mb-lg">
          <button
            onClick={installUpdate}
            className="flex-1 px-lg py-sm rounded-lg bg-primary text-on-primary font-label-md text-label-md font-medium hover:opacity-90 active:scale-[0.98] transition-all"
          >
            立即更新
          </button>
          <button
            onClick={handleDismiss}
            className="flex-1 px-lg py-sm rounded-lg text-on-surface-variant font-label-md text-label-md hover:bg-surface-variant/50 active:scale-[0.98] transition-all"
          >
            稍后
          </button>
        </div>

        {/* Snooze Checkbox */}
        <label className="flex items-center gap-sm cursor-pointer group">
          <input
            type="checkbox"
            checked={snoozeChecked}
            onChange={(e) => setSnoozeChecked(e.target.checked)}
            className="w-4 h-4 rounded border-outline-variant/50 bg-surface-variant/30 text-primary focus:ring-primary/50 focus:ring-offset-0"
          />
          <span className="font-label-md text-label-md text-on-surface-variant group-hover:text-on-surface transition-colors">
            30天不再提示
          </span>
        </label>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: 验证组件语法**

运行 TypeScript 检查确认组件无语法错误：

```bash
cd /mnt/c/Users/myuser/Projects/filter-manage && npx tsc --noEmit src/components/UpdateModal.tsx
```

Expected: 无错误输出

- [ ] **Step 3: Commit**

```bash
git add src/components/UpdateModal.tsx
git commit -m "feat(ui): create UpdateModal component with Glassmorphism style"
```

---

### Task 3: 替换 App.tsx 中的 UpdateBanner

**Files:**
- Modify: `src/App.tsx:8` (import)
- Modify: `src/App.tsx:270` (component usage)

- [ ] **Step 1: 更新 import 语句**

修改 `src/App.tsx` 第 8 行，将 UpdateBanner 替换为 UpdateModal：

```typescript
// Before
import UpdateBanner from "./components/UpdateBanner";

// After
import UpdateModal from "./components/UpdateModal";
```

- [ ] **Step 2: 更新组件使用**

修改 `src/App.tsx` 第 270 行，将 `<UpdateBanner />` 替换为 `<UpdateModal />`：

```tsx
// Before
<UpdateBanner />

// After
<UpdateModal />
```

- [ ] **Step 3: 验证编译**

运行构建确认无错误：

```bash
cd /mnt/c/Users/myuser/Projects/filter-manage && npm run build
```

Expected: 构建成功，无错误

- [ ] **Step 4: Commit**

```bash
git add src/App.tsx
git commit -m "refactor: replace UpdateBanner with UpdateModal in App"
```

---

### Task 4: 删除旧的 UpdateBanner 组件

**Files:**
- Delete: `src/components/UpdateBanner.tsx`

- [ ] **Step 1: 删除 UpdateBanner.tsx**

```bash
rm /mnt/c/Users/myuser/Projects/filter-manage/src/components/UpdateBanner.tsx
```

- [ ] **Step 2: 验证构建**

运行构建确认删除后无引用错误：

```bash
cd /mnt/c/Users/myuser/Projects/filter-manage && npm run build
```

Expected: 构建成功，无错误

- [ ] **Step 3: Commit**

```bash
git add -A src/components/UpdateBanner.tsx
git commit -m "chore: remove old UpdateBanner component"
```

---

### Task 5: 手动测试验证

**Files:**
- 无文件改动

- [ ] **Step 1: 启动开发服务器**

```bash
cd /mnt/c/Users/myuser/Projects/filter-manage && npm run tauri dev
```

- [ ] **Step 2: 验证弹窗显示**

1. 等待应用启动
2. 如果有更新可用，应显示居中模态弹窗
3. 确认弹窗在 header 之上（z-index > 50）
4. 确认 Glassmorphism 样式正确（半透明背景 + 模糊效果）

- [ ] **Step 3: 测试"30天不再提示"功能**

1. 勾选"30天不再提示"复选框
2. 点击"稍后"按钮
3. 刷新应用
4. 确认更新弹窗不再显示

- [ ] **Step 4: 测试静默期过期**

1. 在浏览器控制台执行：`localStorage.removeItem("update_snooze_until")`
2. 刷新应用
3. 确认更新弹窗重新显示

- [ ] **Step 5: 测试下载进度**

1. 点击"立即更新"按钮
2. 确认进度条正常显示
3. 确认下载完成后应用重启

---

## Summary

| Task | Description | Files |
|------|-------------|-------|
| 1 | 增强 useUpdater hook | `src/hooks/useUpdater.ts` |
| 2 | 创建 UpdateModal 组件 | `src/components/UpdateModal.tsx` |
| 3 | 替换 App.tsx 中的 UpdateBanner | `src/App.tsx` |
| 4 | 删除旧的 UpdateBanner | `src/components/UpdateBanner.tsx` |
| 5 | 手动测试验证 | 无 |

**Total Steps:** 17
**Estimated Time:** 15-20 分钟
