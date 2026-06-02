# 首次关闭弹窗（缩小到托盘 / 退出）— 前端交接文档

**日期**：2026-05-31
**后端改动**：已完成
**前端需要**：实现/接入 `ClosePromptModal`，并在 `AppSettings` 接口新增 `close_prompted` 字段

---

## 问题背景

点击窗口关闭按钮时，本应弹出「是否缩小到托盘」的弹窗让用户选择，但实际是窗口直接消失、没有弹窗。

**根因**：后端在 `CloseRequested` 里读 `close_to_tray`，默认 `true` → 首次关闭就直接 `prevent_close + hide`，窗口在前端弹窗显示之前已被隐藏。`close_to_tray` 是默认 `true` 的纯布尔值，无法区分「用户选了缩小」和「还没选过」。

**修法**：新增 `close_prompted` 标记（是否已询问过）。后端据此决定是否把首次关闭交给前端弹窗处理。

---

## 后端已完成的改动

| 文件 | 改动 |
|------|------|
| `src-tauri/src/config.rs` | `AppSettings` 新增 `close_prompted: bool`（`#[serde(default)]`，旧设置文件兼容，默认 `false`） |
| `src-tauri/src/lib.rs` | 窗口 `CloseRequested` 逻辑改为按 `close_prompted` 分支 |

### 后端新的关闭行为（契约）

```
CloseRequested:
  close_prompted == false  → prevent_close()，窗口保持可见（交给前端弹窗询问）
  close_prompted == true 且 close_to_tray == true   → prevent_close() + 隐藏窗口
  close_prompted == true 且 close_to_tray == false  → 不拦截，正常退出
```

---

## 前端需要改动

### 1. `AppSettings` 接口新增字段

```ts
interface AppSettings {
  close_to_tray: boolean;
  close_prompted: boolean;   // 新增：是否已通过弹窗选择过关闭行为
  autostart: boolean;
  tray_presets: string[];
  shortcuts: ShortcutBinding[];
}
```

> 调 `save_app_settings` 时**必须带上 `close_prompted`**，不要丢字段（丢了会被后端 `serde(default)` 重置为 `false`，导致每次关闭都弹窗）。

### 2. 修复 `handleClosePromptSelect`（当前有 bug）

当前实现的问题：
- 选"最小化到托盘"后**没有调 `getCurrentWindow().hide()`** → 窗口卡在那里不消失
- 没有设置 `close_prompted: true` → 后端下次关闭还是走弹窗分支
- 用了 localStorage 而非后端 settings 作为判断来源

正确实现：

```ts
const handleClosePromptSelect = async (closeToTray: boolean, remember: boolean) => {
  // 保存到后端（不再用 localStorage）
  try {
    const settings = await invoke<AppSettings>("get_app_settings");
    await invoke("save_app_settings", {
      settings: {
        ...settings,
        close_to_tray: closeToTray,
        close_prompted: remember,   // 勾选「记住」→ true；不勾 → false（下次再问）
      },
    });
  } catch (err) {
    console.error("Failed to save settings:", err);
  }

  setShowClosePrompt(false);

  if (closeToTray) {
    await getCurrentWindow().hide();   // ← 关键：隐藏窗口到托盘
  } else {
    await exit(0);   // 用 @tauri-apps/plugin-process 的 exit（destroy 可能不够）
  }
};
```

### 3. 关闭监听也需要改

当前 `onCloseRequested` 里用 localStorage 判断是否弹窗，应改为读后端 settings：

```ts
await win.onCloseRequested(async (e) => {
  const s = await invoke<AppSettings>("get_app_settings");
  if (s.close_prompted) return; // 已选择过 → 后端自动处理
  e.preventDefault();
  setShowClosePrompt(true);
});
```

---

## 验证

1. 全新状态（`close_prompted=false`）点关闭 → 窗口不消失，弹出选择弹窗。
2. 选「最小化到托盘」+「记住」→ 窗口隐藏到托盘；再次点关闭 → 直接隐藏、不再弹窗。
3. 选「直接退出」+「记住」→ 应用退出；下次启动后点关闭 → 直接退出、不弹窗。
4. 不勾「记住」→ 每次关闭都弹窗。
