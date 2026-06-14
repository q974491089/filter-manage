# 快捷键/托盘应用方案后前端 UI 同步 — 前端交接文档

**日期**：2026-05-31  
**后端改动**：已完成  
**前端需要**：监听 `config-applied` 事件，同步 UI 状态

---

## 问题背景

通过快捷键或托盘菜单应用方案后，前端 UI 没有同步更新：
- 左侧 ICC 列表没有勾选当前生效的 ICC
- 右侧快速方案没有高亮当前方案
- NVIDIA 滑块值没有更新

**根因**：快捷键/托盘是纯后端操作，不经过前端 `handleApply`，前端 state 不知道发生了变化。

---

## 后端已完成的改动

| 文件 | 改动 |
|------|------|
| `src-tauri/src/shortcut.rs` | 快捷键触发后 `app.emit("config-applied", &config_name)` |
| `src-tauri/src/tray.rs` | 托盘菜单点击方案/恢复默认后 `app.emit("config-applied", name)` |

### 事件定义

```
事件名: "config-applied"
payload: string  — 被应用的方案名（如 "游戏模式"），恢复默认时为 "__default__"
```

---

## 前端需要实现

在 `App.tsx`（或顶层组件）监听 `config-applied` 事件，收到后同步 UI 状态。

```ts
import { listen } from "@tauri-apps/api/event";

useEffect(() => {
  const unlisten = listen<string>("config-applied", async (event) => {
    const configName = event.payload;

    if (configName === "__default__") {
      // 恢复默认：重置所有 UI 状态
      const def = await invoke<ColorConfig | null>("load_default_config");
      if (def) {
        setBrightness(def.brightness);
        setContrast(def.contrast);
        setGamma(def.gamma);
        setDigitalVibrance(def.digital_vibrance);
        setActiveProfile("Default");
        setSelectedConfig("");
      }
    } else {
      // 应用了某个方案：加载该方案数据更新 UI
      try {
        const cfg = await invoke<ColorConfig>("load_config", { name: configName });
        setBrightness(cfg.brightness);
        setContrast(cfg.contrast);
        setGamma(cfg.gamma);
        setDigitalVibrance(cfg.digital_vibrance);
        setActiveProfile(cfg.icc_profile || "Default");
        setSelectedConfig(configName);
      } catch (err) {
        console.error("Failed to sync UI after config-applied:", err);
      }
    }
  });

  return () => { unlisten.then(fn => fn()); };
}, []);
```

**要点**：
- 事件 payload 是方案名（string），不是完整 config 对象——需要前端再 `load_config` 一次拿完整数据
- `__default__` 走 `load_default_config`（和 `handleRestore` 逻辑一致）
- 更新 `selectedConfig` 让右侧方案列表高亮
- 更新 `activeProfile` 让左侧 ICC 列表勾选
- 更新滑块值让 NVIDIA 调节器同步

---

## 验证

1. 快捷键应用方案 A → 前端滑块/ICC 勾选/方案高亮全部同步
2. 托盘菜单点方案 B → 同上
3. 快捷键/托盘"恢复默认" → 滑块归零、ICC 回 Default、方案取消高亮
