# 系统托盘 + 全局快捷键 + 开机自启 — 前端交接文档

**日期**：2026-05-28  
**后端改动**：已完成  
**前端需要**：设置页面 UI + 首次关闭提示弹窗

---

## 后端已完成的改动

| 文件 | 改动 |
|------|------|
| `Cargo.toml` | 添加 `tauri-plugin-global-shortcut`、`tauri-plugin-autostart`，tauri 增加 `tray-icon` feature |
| `tauri.conf.json` | 添加 `trayIcon` 配置 |
| `capabilities/default.json` | 添加 `global-shortcut:default`、`autostart:default` 权限 |
| `src/lib.rs` | 集成托盘、快捷键、自启动插件，监听窗口关闭事件 |
| `src/config.rs` | 新增 `AppSettings`、`ShortcutBinding` 结构体及读写命令 |
| `src/tray.rs` | 新建，系统托盘初始化、菜单构建、方案应用 |
| `src/shortcut.rs` | 新建，全局快捷键注册/注销/触发方案 |

---

## 前端需要安装的依赖

```bash
pnpm add @tauri-apps/plugin-global-shortcut @tauri-apps/plugin-autostart
```

---

## 新增 Tauri 命令一览

### 应用设置

```typescript
// 获取应用设置
const settings = await invoke<AppSettings>('get_app_settings');

// 保存应用设置
await invoke('save_app_settings', { settings });
```

**AppSettings 类型定义：**

```typescript
interface ShortcutBinding {
  shortcut: string;      // 如 "Ctrl+Shift+1"
  config_name: string;   // 方案名
}

interface AppSettings {
  close_to_tray: boolean;        // 关闭时最小化到托盘（默认 true）
  autostart: boolean;            // 开机自启（默认 false）
  tray_presets: string[];        // 托盘展示的方案名列表（空=默认前5个）
  shortcuts: ShortcutBinding[];  // 快捷键绑定
}
```

### 快捷键绑定

```typescript
// 绑定快捷键到方案（一对一，会自动检测冲突）
await invoke('bind_shortcut', { shortcut: 'Ctrl+Shift+1', configName: '游戏模式' });

// 解绑方案的快捷键
await invoke('unbind_shortcut', { configName: '游戏模式' });

// 获取所有绑定
const bindings = await invoke<ShortcutBinding[]>('list_shortcut_bindings');
```

**快捷键格式**：使用 Tauri 标准格式，如 `Ctrl+Shift+1`、`Alt+F1`、`CommandOrControl+K`

### 开机自启

```typescript
// 启用开机自启
await invoke('enable_autostart');

// 禁用开机自启
await invoke('disable_autostart');

// 查询是否已启用
const enabled = await invoke<boolean>('is_autostart_enabled');
```

### 托盘菜单刷新

```typescript
// 方案列表变化时（新增/删除/重命名方案后）调用
await invoke('refresh_tray_menu');
```

---

## 前端需要实现的 UI

### 1. 设置页面

需要一个新的设置页面（或设置弹窗），包含以下区域：

#### 关闭行为
- 勾选框：「关闭窗口时最小化到系统托盘」（对应 `close_to_tray`）
- 说明文字：取消勾选后，关闭窗口将直接退出应用

#### 开机自启
- 勾选框：「开机时自动启动」（对应 `autostart`）
- **默认不勾选**
- 勾选时调用 `enable_autostart`，取消时调用 `disable_autostart`

#### 快捷键绑定
- 列表展示所有已保存的方案
- 每个方案旁边有一个快捷键输入框
- 用户点击输入框后按下快捷键组合，自动填入
- 保存时调用 `bind_shortcut`
- 清除时调用 `unbind_shortcut`
- 冲突时后端会返回错误信息，前端展示提示

#### 托盘快速方案
- 多选列表，展示所有方案
- 用户勾选想在托盘菜单中展示的方案（最多5个）
- 如果不勾选任何方案，默认展示前5个
- 保存后调用 `save_app_settings` + `refresh_tray_menu`

### 2. 首次关闭提示弹窗

**触发条件**：用户第一次点击窗口关闭按钮时（可用 localStorage 记录是否已提示过）

**弹窗内容**：
- 标题：「关闭行为」
- 说明：「关闭窗口后，应用将继续在系统托盘运行，全局快捷键保持生效。」
- 选项 A：「最小化到托盘」（推荐）
- 选项 B：「直接退出应用」
- 勾选框：「记住我的选择」

**实现方式**：
- 前端监听窗口关闭事件（`@tauri-apps/api/window` 的 `onCloseRequested`）
- 首次关闭时弹出自定义弹窗让用户选择
- 用户选择后调用 `save_app_settings` 保存 `close_to_tray` 值
- 后端会根据 `close_to_tray` 值自动处理关闭行为

**注意**：后端已经在窗口关闭事件中读取 `close_to_tray` 设置。前端只需要在首次关闭时弹窗让用户设置这个值即可。

---

## 注意事项

1. 快捷键格式必须符合 Tauri 标准（如 `Ctrl+Shift+1`），无效格式后端会返回错误
2. 方案被删除时，前端应同时调用 `unbind_shortcut` 解绑对应快捷键
3. 方案列表变化后（增删改），前端应调用 `refresh_tray_menu` 刷新托盘菜单
4. `AppSettings` 的 `autostart` 字段仅用于前端 UI 状态同步，实际启用/禁用通过 `enable_autostart`/`disable_autostart` 命令控制
