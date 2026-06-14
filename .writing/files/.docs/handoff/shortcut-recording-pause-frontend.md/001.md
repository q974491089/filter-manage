# 快捷键录制时暂停全局快捷键 — 前端交接文档

**日期**：2026-05-31
**后端改动**：已完成
**前端需要**：在 `ShortcutInput` 录制状态进入/退出时调用新命令

---

## 问题背景

录制一个**已被绑定**的快捷键时「没有任何反应、也没有提示」。

**根因**：全局快捷键注册在操作系统层。`ShortcutInput` 用 `window` 的 `keydown` 录制按键，但待录制的组合若已是注册中的全局快捷键，按下会被 OS 层全局快捷键**直接拦截并触发「应用方案」**，`keydown` 传不到录制框 → `onChange` 不触发 → `bind_shortcut` 根本没被调用 → 既无绑定也无错误提示。后端的冲突 `Err` 在此 UI 下永远到不了。

**修法**：录制期间临时**暂停所有全局快捷键**，录制结束/取消后**恢复**。

> 不能在前端直接用 `@tauri-apps/plugin-global-shortcut` 的 `unregisterAll()`：它会把后端注册的「应用方案」处理器一并注销，且 JS 无法重新注册这些 Rust 处理器。必须走下面的后端命令。

---

## 后端已完成的改动

| 文件 | 改动 |
|------|------|
| `src-tauri/src/shortcut.rs` | 新增 `pause_shortcuts`、`resume_shortcuts` 两个命令 |
| `src-tauri/src/lib.rs` | 注册上述两个命令 |

### 新增命令

```typescript
// 进入录制：注销所有全局快捷键，让按键能被输入框捕获
await invoke('pause_shortcuts');

// 退出录制（成功/取消/Esc/失焦 都要调用）：从设置重新注册全部全局快捷键
await invoke('resume_shortcuts');
```

- `pause_shortcuts`：`unregister_all()`，不改设置。
- `resume_shortcuts`：先 `unregister_all()` 再从 `__settings__.json` 重新注册全部（幂等，即使 `bind_shortcut` 已注册新键也不会重复冲突）。

---

## 前端需要实现（`src/components/ShortcutInput.tsx`）

在录制状态切换时调用上述命令。要点：**任何退出录制的路径都必须 `resume_shortcuts`，否则全局快捷键会一直处于禁用状态。**

建议改动（示意，非强制写法）：

```tsx
import { invoke } from "@tauri-apps/api/core";

// 点击进入录制
const startListening = async () => {
  try { await invoke("pause_shortcuts"); } catch (e) { console.error(e); }
  setIsListening(true);
};

// 统一的退出录制收尾（成功录制、Esc、点击外部、组件卸载都走这里）
const stopListening = async () => {
  setIsListening(false);
  try { await invoke("resume_shortcuts"); } catch (e) { console.error(e); }
};
```

对应改造点：
1. 输入框 `onClick` 由 `setIsListening(true)` 改为 `startListening()`。
2. `handleKeyDown` 里捕获到组合键后（`onChange(...)` 之后）、以及按 `Escape` 分支，都改为调用 `stopListening()`。
3. `useEffect` cleanup（组件卸载或 `isListening` 关闭）兜底调用 `resume_shortcuts`，防止异常退出后快捷键一直禁用。
4. 录制结束后由 `SettingsModal.handleShortcutChange` 调 `bind_shortcut`；此时若与其它方案冲突，后端返回 `Err`，前端已有的 `showToast("error", ...)` 即可正常弹出冲突提示（修复后这条路径才真正可达）。

**顺序**：进入录制 `pause` → 捕获组合键 → `bind_shortcut`（绑定+保存）→ `resume`（重注册全部）。

---

## 验证

1. 给方案 A 绑定 `Ctrl+Shift+1`。
2. 再给方案 B 录制同一个 `Ctrl+Shift+1`：
   - 录制框能正常捕获到按键（不再被拦截、不再触发方案 A）。
   - `bind_shortcut` 返回冲突错误 → 弹出「快捷键 'Ctrl+Shift+1' 已绑定到方案 'A'」提示。
3. 录制中途按 Esc 或点别处取消 → 原有快捷键恢复生效。
