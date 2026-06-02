# Filter Manage — 项目迭代目录

> 本文件是**给 Agent 看的完整技术迭代记录**，比面向用户的 `CHANGELOG.md` 更详尽（含端别、根因、涉及文件）。
> 读文档时先看这里定位「哪个版本动了什么、为什么动、改了哪些文件」，再按链接跳转具体文档。

**列说明**：
- **类型**：新功能 / 修复 / 改进 / 重构
- **端**：前端 / 后端 / 配置 / CI（一条改动可能跨多端）
- 版本号与日期严格对齐 Git tag；未发布工作归入「开发中」块。

---

## 迭代记录

### v0.2.9（开发中 · 未发布）— 系统托盘常驻 + 全局快捷键 + 开机自启 + 配置重构

应用支持最小化到系统托盘常驻运行、全局快捷键一键切换方案、开机自启可选，并修复托盘双实例与配置列表问题；配置存储重构为单文件 app.json，新增原子写+备份兜底。

| 类型 | 端 | 说明 | 涉及文件 | 文档 |
|------|----|------|---------|------|
| 新功能 | 后端 | 系统托盘：`init_tray`/`build_tray_menu`，菜单含「显示主窗口 / 快速方案子菜单 / 退出」，点击方案即应用颜色 | `src-tauri/src/tray.rs`、`lib.rs` | [api/config.md](./api/config.md) |
| 新功能 | 后端 | 全局快捷键：`bind_shortcut`/`unbind_shortcut`/`list_shortcut_bindings`，启动注册、一对一绑定、冲突检测 | `src-tauri/src/shortcut.rs`、`lib.rs` | [api/config.md](./api/config.md) |
| 新功能 | 后端 | 开机自启：`enable_autostart`/`disable_autostart`/`is_autostart_enabled` | `src-tauri/src/lib.rs` | [api/config.md](./api/config.md) |
| 新功能 | 后端 | 应用设置：`AppSettings`/`ShortcutBinding` 结构 + `get_app_settings`/`save_app_settings` | `src-tauri/src/config.rs`、`lib.rs` | [api/config.md](./api/config.md) |
| 新功能 | 配置 | 引入 `tauri-plugin-global-shortcut`、`tauri-plugin-autostart`，tauri 开启 `tray-icon` feature；权限加 `global-shortcut:default`、`autostart:default` | `Cargo.toml`、`capabilities/default.json` | [handoff](./handoff/tray-shortcut-autostart-frontend.md) |
| 新功能 | 前端 | 设置弹窗（常规/快捷键/显示适配三页）：托盘关闭开关、开机自启开关、主题切换、快捷键绑定、托盘方案多选（≤5） | `src/components/SettingsModal.tsx` | [handoff](./handoff/tray-shortcut-autostart-frontend.md) |
| 新功能 | 前端 | 快捷键录入组件 + 首次关闭行为提示弹窗 | `src/components/ShortcutInput.tsx`、`ClosePromptModal.tsx` | [handoff](./handoff/tray-shortcut-autostart-frontend.md) |
| 新功能 | 前端 | 新增依赖 `@tauri-apps/plugin-global-shortcut`、`@tauri-apps/plugin-autostart` | `package.json` | [handoff](./handoff/tray-shortcut-autostart-frontend.md) |
| 修复 | 后端+配置 | 托盘菜单运行时不刷新。**根因**：`tauri.conf.json` 的 `trayIcon` 默认 id 也是 `main`，与代码托盘冲突 → `tray_by_id("main")` 命中错误实例、桌面出现两个托盘。**修复**：移除 conf 的 `trayIcon`，`init_tray` 改用 `with_id("main")` | `src-tauri/src/tray.rs`、`tauri.conf.json` | [api/config.md#refresh_tray_menu](./api/config.md) |
| 修复 | 后端 | `list_configs` 误把设置文件 `__settings__` 列为颜色方案。**根因**：过滤只排除了 `__default__` | `src-tauri/src/config.rs` | [api/config.md#list_configs](./api/config.md) |
| 新功能 | 后端 | `rename_config`：原子重命名配置预设，内置 old=new 短路、not found、already exists 校验 | `src-tauri/src/config.rs`、`lib.rs` | [api/config.md](./api/config.md) |
| 重构 | 后端 | 配置存储重构为单文件 `app.json`（presets + settings 合并），兼容三代旧格式自动迁移（散文件/profiles.json/__settings__.json），所有命令签名不变 | `src-tauri/src/config.rs`、`lib.rs` | [api/config.md](./api/config.md) |
| 改进 | 后端 | `app.json` 写入改为原子操作（tmp→rename）+ 写前备份（app.json.bak）；读取时主文件损坏自动降级读备份，最坏情况返回默认值不崩溃 | `src-tauri/src/config.rs` | — |
| 改进 | 后端 | `AppSettings.close_to_tray` 从 `bool` 改为 `Option<bool>`：`None`=未选择（弹窗询问），`Some(true)`=托盘，`Some(false)`=直接关闭；serde default 兼容旧文件 | `src-tauri/src/config.rs` | [handoff](./handoff/close-behavior-frontend.md) |
| 改进 | 后端 | 托盘无可用方案时隐藏「快速方案」子菜单，避免展示空子菜单 | `src-tauri/src/tray.rs` | — |
| 新功能 | 后端 | 快捷键切换方案时发送 Windows 系统通知（tauri-plugin-notification），通知内容为方案名称 | `src-tauri/src/shortcut.rs`、`Cargo.toml`、`lib.rs`、`capabilities/default.json` | [api/config.md](./api/config.md) |
| 新功能 | 后端 | `AppSettings` 新增 `shortcut_notification` 字段（默认 true），控制快捷键切换时是否弹出系统通知 | `src-tauri/src/config.rs`、`shortcut.rs` | [handoff](./handoff/shortcut-notification-toggle-frontend.md) |
| 修复 | 后端+前端 | 录制已绑定的快捷键时无反应无提示。**根因**：该组合已注册为 OS 全局快捷键，按下被拦截触发应用方案，按键传不到录制框。**修复**：新增 `pause_shortcuts`/`resume_shortcuts`，录制期间暂停全局快捷键 | `src-tauri/src/shortcut.rs`、`lib.rs` | [handoff](./handoff/shortcut-recording-pause-frontend.md) |
| 新功能 | 后端 | 双击托盘图标直接打开主窗口（`on_tray_icon_event` 监听 `DoubleClick`） | `src-tauri/src/tray.rs` | — |
| 新功能 | 后端 | 托盘菜单新增「恢复默认」：应用已保存的默认配置，无则恢复系统 sRGB + DVC 50% | `src-tauri/src/tray.rs` | — |
| 修复 | 后端+前端 | 点击关闭无弹窗、窗口直接消失。**根因**：默认 `close_to_tray=true` 致首次关闭即静默隐藏。**修复**：新增 `close_prompted` 标记，未选择过时只 `prevent_close` 保持窗口可见交前端弹窗 | `src-tauri/src/config.rs`、`lib.rs` | [handoff](./handoff/close-prompt-frontend.md) |
| 修复 | 后端 | 托盘/快捷键应用方案时 ICC 滤镜不生效。**根因**：同步回调里 `block_on(set_icc_profile)` 不可靠。**修复**：直接同步调用 `icc::apply_icc_profile` | `src-tauri/src/tray.rs`、`icc.rs` | — |
| 修复 | 后端 | 快捷键「恢复默认」(`__default__`，icc 为 null) 不重置 ICC；含 ICC 方案 NVIDIA 被覆盖。**根因**：`apply_color_config` 顺序为 NVIDIA→ICC 且 `None` 时不处理 ICC，与前端 `handleApply`(ICC→NVIDIA、null 时恢 sRGB) 不一致。**修复**：改为 ICC 先/NVIDIA 后，`None` 时恢复 sRGB（新增同步 `icc::restore_default_icc`），ICC 缺文件只跳过不阻断 NVIDIA | `src-tauri/src/tray.rs`、`icc.rs` | — |
| 修复 | 后端 | 快捷键绑定报"已绑定到已删除方案"。**根因**：删方案不清理 settings 里的快捷键记录。**修复**：`bind_shortcut` 前清理孤儿绑定（豁免 `__default__`），`init_shortcuts` 跳过已删方案 | `src-tauri/src/shortcut.rs` | — |
| 新功能 | 后端 | 快捷键/托盘应用方案后 emit `config-applied` 事件（payload=方案名），供前端同步 UI 状态（前端待接入） | `src-tauri/src/shortcut.rs`、`tray.rs` | [handoff](./handoff/config-applied-event-frontend.md) |

---

### v0.2.8 — 2026-05-25 · 修复更新检查 + 手动检查更新

修复自动更新检查逻辑，并支持在「关于」中手动检查更新。

| 类型 | 端 | 说明 | 文档 |
|------|----|------|------|
| 修复 | 前端 | 自动更新检查异常修复 | [updater-setup.md](./updater-setup.md) |
| 新功能 | 前端 | 「关于」面板新增手动检查更新入口 | — |

---

### v0.2.7 — 2026-05-25 · 前台样式优化 + 文档站对比图

优化前台界面样式，更新文档站的明暗模式对比图。

| 类型 | 端 | 说明 | 文档 |
|------|----|------|------|
| 改进 | 前端 | 前台整体样式打磨 | — |
| 改进 | 文档 | 文档站更新明/暗模式预览对比图 | — |

---

### v0.2.6 — 2026-05-24 · 更新弹窗改造 + opener 集成

更新提示 UI 升级为弹窗（30 天 snooze）；后端集成 `tauri-plugin-opener` 打开外部链接。

| 类型 | 端 | 说明 | 文档 |
|------|----|------|------|
| 新功能 | 配置 | 集成 `tauri-plugin-opener`，前端经 `@tauri-apps/plugin-opener` 调用 | [handoff/opener-frontend.md](./handoff/opener-frontend.md) |
| 改进 | 前端 | 自动更新提示由 Banner 改为 Modal，展示完整 release notes，支持「30 天内不再提醒」 | — |
| 新功能 | 配置 | `capabilities/default.json` 补 `updater:default`、`process:allow-restart`、`opener:default` | — |
| 修复 | CI | `package-lock.json` 缺 `@tauri-apps/plugin-opener` 节点导致 `npm ci` 失败 | — |

---

### v0.2.5 — 2026-05-23 · 更新器构建修复

修复更新器签名产物与 `latest.json` 上传问题。

| 类型 | 端 | 说明 | 文档 |
|------|----|------|------|
| 修复 | 配置 | `createUpdaterArtifacts` / `updaterJsonPreferNsis` 修正，使签名产物与 `latest.json` 正确生成上传 | [updater-setup.md](./updater-setup.md) |

---

### v0.2.4 — 2026-05-22 · DVC 多显示器支持

数字振动(DVC)按显示器独立设置，不再写死第一个显示器。

| 类型 | 端 | 说明 | 文档 |
|------|----|------|------|
| 新功能 | 后端 | `set_nvidia_digital_vibrance` 按 `deviceId` 设置对应显示器 | [api/nvidia.md](./api/nvidia.md) |
| 新功能 | 后端 | `sync_dvc_from_driver`、`get_dvc_default_ui_value` 新增 `deviceId` 参数 | [api/nvidia.md](./api/nvidia.md) |
| 新功能 | 前端 | 多显示器选择接入 | [handoff/dvc-multi-monitor-frontend.md](./handoff/dvc-multi-monitor-frontend.md) |

---

### v0.2.3 — 2026-05-19 · 文档站与 SEO

文档站更新，补全 SEO 元信息。

| 类型 | 端 | 说明 | 文档 |
|------|----|------|------|
| 改进 | 文档 | 新增 SEO meta、sitemap、robots.txt、canonical 链接 | — |

---

### v0.2.2 — 2026-05-19 · 版本号展示 + 在线自动更新

启动时自动检查 GitHub Releases 新版本，确认后下载安装并重启；界面展示当前版本号。

| 类型 | 端 | 说明 | 文档 |
|------|----|------|------|
| 新功能 | 前端+配置 | 自动更新：启动检查新版本，下载安装后重启 | [updater-setup.md](./updater-setup.md) |
| 新功能 | 前端 | 界面展示当前版本号 | — |

---

### v0.2.1 — 2026-05-18 · 前端样式优化

优化界面整体样式与深色模式表现。

| 类型 | 端 | 说明 | 文档 |
|------|----|------|------|
| 改进 | 前端 | 颜色调节器、配置管理、预览图片等组件样式改进；深色模式优化 | — |

---

### v0.2.0 — 2026-05-17 · 首个公开发布

四大核心模块上线，并支持 ICC 导入导出 / 预览 / 搜索。

| 类型 | 端 | 说明 | 文档 |
|------|----|------|------|
| 新功能 | 后端 | ICC 管理：扫描系统 ICC 目录、切换配置、恢复默认 sRGB | [api/icc.md](./api/icc.md) |
| 新功能 | 后端 | NVIDIA 颜色：亮度/对比度/伽马/数字振动实时调节，ICC vcgt 叠加 | [api/nvidia.md](./api/nvidia.md) |
| 新功能 | 后端 | 配置管理：颜色预设保存/加载/删除，支持启动默认值 | [api/config.md](./api/config.md) |
| 新功能 | 后端 | 显示器枚举：读取 EDID 获取型号，支持多显示器 | [api/icc.md](./api/icc.md) |
| 新功能 | 后端 | ICC 导入/导出/搜索、打开 ICC 目录、图片预览校验 | [api/icc.md](./api/icc.md) |
| 新功能 | 前端 | 拖拽导入、`convertFileSrc` 预览接入 | [frontend-guide.md](./frontend-guide.md) |

---

## 固定参考文档

| 文档 | 内容 |
|------|------|
| [architecture.md](./architecture.md) | 技术栈、项目结构、数据流、ICC×NVIDIA叠加机制 |
| [frontend-guide.md](./frontend-guide.md) | 前端组件结构、新功能接入清单、常用 import |
| [api/icc.md](./api/icc.md) | ICC 全部命令 |
| [api/nvidia.md](./api/nvidia.md) | NVIDIA 颜色全部命令 |
| [api/config.md](./api/config.md) | 配置预设 + 应用设置 + 快捷键 + 自启 + 托盘全部命令 |
| [updater-setup.md](./updater-setup.md) | 自动更新密钥生成与 CI 配置 |
