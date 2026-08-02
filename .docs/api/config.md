# 后端 API — 配置管理

源文件：`src-tauri/src/config.rs`

## 存储结构

**当前格式（v1）**：所有配置（颜色预设 + 应用设置）统一存储在单个文件，便于整体导入导出和云端同步。

```
%APPDATA%\filter-manage\
  app.json    ← 唯一配置文件
```

`app.json` 结构：

```json
{
  "version": 1,
  "default_preset": "__default__",
  "presets": [
    { "name": "游戏模式", "icon": "sports_esports", "brightness": 10, "contrast": 5, "gamma": 1.2, "digital_vibrance": 80, "icc_profile": null }
  ],
  "settings": {
    "close_to_tray": true,
    "close_prompted": false,
    "autostart": false,
    "tray_presets": [],
    "shortcuts": [{ "shortcut": "Ctrl+Shift+1", "config_name": "游戏模式" }],
    "shortcut_notification": true
  }
}
```

**云端同步说明**：上传/下载整个 `app.json` 即可恢复所有配置。不希望跨设备同步的字段（如 `autostart`）由前端在上传前排除，后端不感知云同步逻辑。

**旧格式迁移**：启动时自动兼容三代旧格式（散文件 `*.json`、`profiles.json`、`__settings__.json`），迁移后删除旧文件，幂等。

**更新于**：2026-06-02 — 合并 profiles.json + __settings__.json 为单文件 app.json

## 数据类型

```ts
interface ColorConfig {
  name: string
  icon?: string | null
  brightness: number
  contrast: number
  gamma: number
  digital_vibrance: number
  rgb_r?: number   // -100 ~ 100，默认 0；旧方案缺字段按 0
  rgb_g?: number
  rgb_b?: number
  icc_profile: string | null
}
```

**更新于**：2026-07-24 — `ColorConfig` 新增 `rgb_r` / `rgb_g` / `rgb_b`（`#[serde(default)]`，旧方案自动为 0）

**更新于**：2026-06-11 — `ColorConfig` 新增 `icon` 字段（`Option<String>`，Material Icon 名称，旧配置自动为 `null`）

## 命令列表

### `save_config`
保存（或覆盖）一个颜色配置预设。

```ts
await invoke('save_config', {
  config: {
    name: '游戏模式',
    icon: 'sports_esports',
    brightness: 10,
    contrast: 5,
    gamma: 1.2,
    digital_vibrance: 80,
    icc_profile: null
  }
})
```

---

### `load_config`
按名称加载配置预设。

```ts
const config: ColorConfig = await invoke('load_config', { name: '游戏模式' })
```

---

### `list_configs`
列出所有用户配置预设（不含内部默认配置 `__default__` 与设置文件 `__settings__`），返回完整 `ColorConfig` 列表（含 `icon` 字段），按名称排序。

```ts
const configs: ColorConfig[] = await invoke('list_configs')
```

**更新于**：2026-06-11 — 返回类型由 `string[]` 改为 `ColorConfig[]`，前端无需再逐个调用 `load_config` 获取 `icon`

---

### `delete_config`

```ts
await invoke('delete_config', { name: '游戏模式' })
```

---

### `rename_config`

原子重命名配置预设（读旧文件 → 更新 `name` 字段 → 写新文件 → 删旧文件）。

```ts
await invoke('rename_config', { oldName: '游戏模式', newName: '电竞模式' })
```

- `oldName === newName` 时直接返回成功（幂等）
- 旧名不存在 → 返回错误 `"Config '...' not found"`
- 新名已存在 → 返回错误 `"Config '...' already exists"`

**新增于**：2026-06-02

---

### `save_default_config`
保存启动默认值（仅首次调用生效，文件已存在时跳过）。

```ts
await invoke('save_default_config', { config })
```

---

### `load_default_config`
加载启动默认值，文件不存在时返回 `null`。

```ts
const config: ColorConfig | null = await invoke('load_default_config')
```

---

### `overwrite_default_config`
强制覆盖默认值（用户手动设置默认时调用）。

```ts
await invoke('overwrite_default_config', { config })
```

---

## 应用设置 API

**新增于**：2026-05-28

设置文件存储路径：`%APPDATA%\filter-manage\__settings__.json`

### 数据类型

```ts
interface ShortcutBinding {
  shortcut: string;      // 快捷键，如 "Ctrl+Shift+1"
  config_name: string;   // 绑定的方案名
}

interface AppSettings {
  close_to_tray: boolean;        // 关闭时最小化到托盘（默认 true）
  close_prompted: boolean;       // 是否已通过弹窗选择过关闭行为（默认 false）
  autostart: boolean;            // 开机自启（默认 false）
  tray_presets: string[];        // 托盘展示的方案名列表（空=默认前5个）
  shortcuts: ShortcutBinding[];  // 快捷键绑定
}
```

### `get_app_settings`

获取应用设置，文件不存在时返回默认值。

```ts
const settings = await invoke<AppSettings>('get_app_settings');
```

**更新于**：2026-05-31 — `AppSettings` 新增 `close_prompted` 字段（首次关闭弹窗用，`serde(default)` 兼容旧文件）

### `save_app_settings`

保存应用设置。

```ts
await invoke('save_app_settings', { settings });
```

### `bind_shortcut`

绑定快捷键到方案（一对一）。冲突时返回错误。

```ts
await invoke('bind_shortcut', { shortcut: 'Ctrl+Shift+1', configName: '游戏模式' });
```

### `unbind_shortcut`

解绑方案的快捷键。

```ts
await invoke('unbind_shortcut', { configName: '游戏模式' });
```

### `list_shortcut_bindings`

获取所有快捷键绑定列表。

```ts
const bindings = await invoke<ShortcutBinding[]>('list_shortcut_bindings');
```

### `pause_shortcuts`

暂停（注销）所有全局快捷键。前端录制新快捷键时调用，避免待录制的组合键被已注册的全局快捷键拦截，导致录制框收不到按键。

```ts
await invoke('pause_shortcuts');
```

**新增于**：2026-05-31

### `resume_shortcuts`

恢复所有全局快捷键。录制结束/取消（含 Esc、失焦、组件卸载）后调用，从 `__settings__.json` 重新注册全部绑定（幂等）。

```ts
await invoke('resume_shortcuts');
```

**新增于**：2026-05-31

### `enable_autostart`

启用开机自启。

```ts
await invoke('enable_autostart');
```

### `disable_autostart`

禁用开机自启。

```ts
await invoke('disable_autostart');
```

### `is_autostart_enabled`

查询开机自启是否已启用。

```ts
const enabled = await invoke<boolean>('is_autostart_enabled');
```

### `refresh_tray_menu`

刷新托盘菜单（方案列表变化后调用）。

```ts
await invoke('refresh_tray_menu');
```

**更新于**：2026-05-31 — 托盘以 `with_id("main")` 创建，使 `tray_by_id("main")` 能正确找到并热更新菜单；并移除 `tauri.conf.json` 中重复的 `trayIcon` 配置（其默认 id 也是 `main`，会与代码托盘冲突导致刷新作用在错误的托盘上）
