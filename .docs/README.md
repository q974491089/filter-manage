# Filter Manage — 项目迭代目录

> 本文件是**给 Agent 看的完整技术迭代记录**，比面向用户的 `CHANGELOG.md` 更详尽（含端别、根因、涉及文件）。
> 读文档时先看这里定位「哪个版本动了什么、为什么动、改了哪些文件」，再按链接跳转具体文档。

**列说明**：
- **类型**：新功能 / 修复 / 改进 / 重构
- **端**：前端 / 后端 / 配置 / CI（一条改动可能跨多端）
- 版本号与日期严格对齐 Git tag；未发布工作归入「开发中」块。

---

## 迭代记录

### v0.4.0 — 2026-08-02 · RGB 增益 + 公告分类 + 保存流程重构

RGB 三通道增益（偏色/白平衡）与三种调节方式换算；公告拆「公告 / 通知」双 Tab；保存动作重构为「更新方案 / 另存为 / 保存方案」三态并禁止删除当前方案；进程监听 WMI 断线自愈。

| 类型 | 端 | 说明 | 涉及文件 | 文档 |
|------|----|------|---------|------|
| 新功能 | 后端+前端 | RGB 增益（内部 -100..+100）：`set_nvidia_rgb_gain`；方案字段 `rgb_r/g/b`；「显示色彩调整」分区；调节方式 NVIDIA/卓伟/AOC 仅 UI 换算 | `nvidia.rs`、`config.rs`、`tray.rs`、`ColorAdjuster.tsx`、`rgbScale.ts`、`App.tsx` | [nvidia.md](./api/nvidia.md)、[config.md](./api/config.md) |
| 新功能 | 后端+前端 | 公告 `category` 字段（`announcement`/`notification`）：铃铛面板拆「公告 / 通知」Tab，各 Tab 独立未读徽标，铃铛仍显示总未读；缺省/未知值归「公告」 | `announcements.rs`、`announcements.ts`、`useAnnouncements.ts`、`AnnouncementBell.tsx`、`App.tsx` | [announcements.md](./api/announcements.md#category分类--客户端-tab) |
| 新功能 | 前端 | 公告正文 Markdown 外链经 `plugin-opener` 用系统浏览器打开（仅 http/https），新增 `AnnouncementMarkdown` 组件 | `src/components/AnnouncementMarkdown.tsx`、`AnnouncementDetailModal.tsx`、`AnnouncementModal.tsx` | [announcements.md](./api/announcements.md) |
| 新功能 | 前端 | 保存流程重构：有当前方案且有改动时显示「更新方案 / 另存为」双按钮，无当前方案时只显示「保存方案」；「更新方案」只写回颜色参数不改名称/图标；另存为禁止重名覆盖 | `App.tsx`、`SaveModal.tsx` | `CONTEXT.md`（领域语言） |
| 新功能 | 前端 | 当前方案不可删除：删除入口禁用并提示，防止删除正在应用的方案后托盘/快捷键引用悬空 | `ConfigManager.tsx` | `CONTEXT.md`（领域语言） |
| 修复 | 后端 | WMI 监听可靠性：断线自愈（指数退避 1s…30s 重订）、脏 `active_rule` 校正、Stopped 多实例防护、可观测 status（`wmi_connected` / `last_error` / `reconnect_attempt`）；无应用层周期轮询 / 无 ETW | `src-tauri/src/process_watcher.rs` | [process_watcher.md](./api/process_watcher.md)、[plans/2026-07-16-process-watcher-wmi-reliability.md](./archive/plans/2026-07-16-process-watcher-wmi-reliability.md) |
| 修复 | 前端 | 启动时从进程监听状态同步快速方案勾选：根因是 reconcile 可能在前端 listen 就绪前 emit `config-applied` 导致事件丢失；挂载时 `get_watcher_status` 补同步，后端先写 `active_rule` 再 emit | `App.tsx`、`src-tauri/src/process_watcher.rs` | — |
| 修复 | 后端+CI | updater 签名双重编码：根因是 CI 用 jq @base64 二次包装 .sig 并带入 JSON 引号，客户端下载后 verify_minisign 必失败；.sig 原文直用 + 客户端归一化历史脏签名 + 更新失败透出错误不再静默关弹窗 | `src-tauri/src/updater.rs`、`.github/workflows/`、`UpdateModal.tsx` | — |
| 改进 | 前端 | 列表/预览布局修正：`min-h-0` / `shrink-0` 修复溢出裁剪，统一圆角 | `ProfileList.tsx`、`PreviewImage.tsx`、`ConfigManager.tsx` | — |
| 改进 | 后端 | 消除 Rust 编译警告 | `src-tauri/src/process_watcher.rs` | — |
| 改进 | 文档/仓库 | 新增 `.rules/coding.md`（AI 编码规范）、`.rules/archive.md`、`.rules/subagent-dispatch.md`、PRD/archive skills、归档区 `.docs/archive/`；停止跟踪 `.writing/` 工作缓冲 | `.rules/`、`.skills/shared/`、`.docs/archive/`、`.gitignore` | — |

---

### v0.3.5 — 2026-07-14 · 应用内公告 + 进程监听对账 + 单实例

顶栏公告（铃铛/重要弹窗/详情），随 check_update 双域名拉取；进程监听订阅后对账，退出恢复默认方案；单实例聚焦已有窗口。

| 类型 | 端 | 说明 | 涉及文件 | 文档 |
|------|----|------|---------|------|
| 新功能 | 后端 | `get_announcements`：复用 `UPDATE_API_HOSTS` 双域名竞速；`GET /api/announcements?type=client`；失败返空列表静默；`id` 兼容 number/string；字段 `pinned`/`sortOrder` 等 | `src-tauri/src/announcements.rs`、`src-tauri/src/lib.rs` | [announcements.md](./api/announcements.md) |
| 新功能 | 前端 | 铃铛未读红点 + 列表预览 + 详情弹窗 + 重要公告启动弹窗；置顶/排序/有效期/已读（localStorage）客户端处理 | `src/components/AnnouncementBell.tsx`、`AnnouncementDetailModal.tsx`、`AnnouncementModal.tsx`、`src/hooks/useAnnouncements.ts`、`src/lib/announcements.ts`、`src/App.tsx` | [announcements.md](./api/announcements.md)、[plans/2026-07-10-announcements.md](./archive/plans/2026-07-10-announcements.md) |
| 新功能 | 后端 | 启用 `tauri-plugin-single-instance`：重复启动聚焦已有窗口 | `src-tauri/Cargo.toml`、`src-tauri/src/lib.rs` | — |
| 修复 | 后端 | 订阅后对账（reconcile）：**根因** WMI 不补发订阅前已运行进程的 Started，`active_rule` 为空导致退出不恢复；订阅成功后扫描进程合成 Started | `src-tauri/src/process_watcher.rs` | [process_watcher.md](./api/process_watcher.md) |
| 修复 | 后端+前端 | `restore_on_exit` 语义明确为恢复默认方案：**根因** `previous_config_name` 从未写入，死分支误导；改为只走 `apply_default_config`，同步 UI/文档 | `process_watcher.rs`、`SettingsModal.tsx`、API/handoff 文档 | [process_watcher.md](./api/process_watcher.md) |
| 改进 | 前端 | 更新弹窗加宽，便于展示较长更新说明 | `src/components/UpdateModal.tsx` | — |
| 改进 | 文档 | 公告 PRD/计划/API；process_watcher reconcile 与默认恢复说明 | `.docs/prd/`、`.docs/plans/`、`.docs/api/` | — |

---

### v0.3.4 — 2026-07-04 · 自定义更新流程 + 双域名竞速

自建更新流程（多镜像 + 测速换源 + 可取消 + minisign 校验），替代 Tauri 内置 updater 直连 GitHub；发版时 CI 自动推送版本清单到自建服务端，客户端 check_update 双域名竞速规避单点 DNS 故障。

| 类型 | 端 | 说明 | 涉及文件 | 文档 |
|------|----|------|---------|------|
| 新功能 | 后端 | 自定义更新流程：check_update / download_update / cancel_update_download / install_update 4 命令 + UpdaterState 共享状态 + minisign 签名校验 + NSIS Passive 模式安装；绕过 tauri-plugin-updater 的 downloadAndInstall，支持运行时切换镜像 | `src-tauri/src/updater.rs`、`src-tauri/src/lib.rs`、`src-tauri/Cargo.toml`（+reqwest/minisign-verify/futures-util） | [updater.md](./api/updater.md) |
| 新功能 | 前端 | useUpdater 重写：invoke + listen 替换 plugin-updater，新增测速（>120s 或 <100KB/s 触发换源）/换源/取消逻辑；UpdateModal 重写：速度展示 + 取消按钮 + 镜像列表换源 UI | `src/hooks/useUpdater.ts`、`src/components/UpdateModal.tsx`、`src/App.tsx` | [custom-updater-frontend.md](./handoff/custom-updater-frontend.md) |
| 新功能 | CI | release.yml 新增「推送版本清单到更新服务」step：从 .sig 文件取签名（jq @base64），构造 ReleaseManifest POST 到服务端 /api/internal/release，让服务端能下发镜像列表 | `.github/workflows/release.yml`、`.skills/shared/release-workflow.md` | [custom-update-flow.md](./plans/custom-update-flow.md) |
| 修复 | 后端 | check_update 双域名竞速；**根因**：单域名 DNS 故障导致检查更新失败，改为 UPDATE_API_HOSTS 双 host tokio::select! 先到先得，落败方 abort | `src-tauri/src/updater.rs` | [updater.md](./api/updater.md) |
| 修复 | CI | 推送 manifest 补 signature；**根因**：v0.3.3 推送时 signature 为空，客户端 verify_minisign 无法校验安装包完整性；改用 jq @base64 从 .sig 文件取签名，跨平台零风险 | `.github/workflows/release.yml` | [custom-update-flow.md](./plans/custom-update-flow.md) |
| 改进 | 前端 | 首页下载按钮样式与功能预览动画优化 | `src/App.tsx`、`src/index.css` | — |
| 改进 | 文档 | 新增 updater API 文档 + 服务端/前端交接文档 + 实现计划；AGENTS.md 角色切换 + .rules/handoff.md 交接格式 + .env.example | `.docs/api/updater.md`、`.docs/handoff/custom-update-*.md`、`.docs/plans/custom-update-flow.md`、`AGENTS.md`、`.rules/handoff.md`、`.env.example` | — |

---

### v0.3.3 — 2026-06-17 · 图标系统迁移

图标系统从字体图标（Material Symbols）迁移到 React 组件图标（react-icons），解决字体加载依赖问题，提升开发体验和打包体积。

| 类型 | 端 | 说明 | 涉及文件 | 文档 |
|------|----|------|---------|------|
| 改进 | 前端 | 图标系统从 Material Symbols 字体迁移到 react-icons 组件；**原因**：解决字体加载依赖问题，提升 TypeScript 支持和 IDE 自动补全，按需加载减少打包体积 | `src/components/Icon.tsx`、`src/lib/icon-map.ts`、16 个组件文件 | — |
| 改进 | 前端 | 新增 Icon 组件封装：支持 outlined/filled 变体，统一图标调用 API | `src/components/Icon.tsx` | — |
| 改进 | 前端 | 新增图标映射表：45+ Material Design 图标从 Material Symbols 名称映射到 react-icons/md 组件 | `src/lib/icon-map.ts` | — |
| 改进 | 前端 | 移除 Material Symbols 字体依赖和 CSS 定义（-25 行） | `src/fonts.css`、`package.json` | — |
| 改进 | 配置 | 依赖变更：移除 `@fontsource-variable/material-symbols-outlined`，新增 `react-icons` | `package.json`、`pnpm-lock.yaml` | — |

---

### v0.3.2 — 2026-06-15 · 字体本地打包 + 弹窗修复

字体从 Google Fonts CDN 改为本地打包，修复国内网络环境下图标显示为英文的问题；修复监听规则弹窗保存按钮不可见。

| 类型 | 端 | 说明 | 涉及文件 | 文档 |
|------|----|------|---------|------|
| 修复 | 前端 | Material Symbols Outlined 和 Inter 字体从 Google Fonts CDN 改为 `@fontsource-variable` 本地打包；**根因**：国内网络无法访问 Google Fonts CDN，字体加载失败导致图标显示为英文占位文字 | `src/fonts.css`、`index.html`、`src/main.tsx`、`package.json` | — |
| 修复 | 前端 | 监听规则弹窗保存按钮不可见；**根因**：进程选择器弹窗内容溢出导致底部操作按钮被裁剪，改为 flex-col 布局确保按钮始终可见 | `src/components/SettingsModal.tsx` | — |
| 改进 | CI | Release workflow 新增云盘手动补传 workflow + AList token 健康检查脚本 | `.github/workflows/upload-drives.yml`、`scripts/update-alist-tokens.sh` | — |
| 改进 | 文档 | 更新 115 网盘分享链接和访问码 | `docs/guide/install.md` | — |

---

### v0.3.1 — 2026-06-14 · 修复全局设置添加规则按钮不显示

修复进程选择器弹窗内容溢出导致设置页 footer 被裁剪，按钮不可见。

| 类型 | 端 | 说明 | 涉及文件 | 文档 |
|------|----|------|---------|------|
| 修复 | 前端 | 全局设置「添加规则」按钮不显示；**根因**：进程选择器弹窗内容溢出导致 footer 被裁剪 | `src/components/SettingsModal.tsx` | — |
| 改进 | 配置 | 版本号同步至 0.3.1 | `src-tauri/tauri.conf.json`、`package.json`、`src-tauri/Cargo.toml` | — |

---

### v0.3.0 — 2026-06-13 · 设计系统优化 + UI 体验改进

设计系统全面优化，移除玻璃拟态效果，统一暗色调风格；字体系统规范化；滑块、Toggle、进程规则列表等组件体验改进。

| 类型 | 端 | 说明 | 涉及文件 | 文档 |
|------|----|------|---------|------|
| 改进 | 配置+前端 | 设计系统优化：移除玻璃拟态（backdrop-filter）、统一暗色调背景、减少透明度使用 | `DESIGN.md`、`src/index.css`、多个组件 | [DESIGN.md](../DESIGN.md) |
| 改进 | 配置+前端 | 字体系统统一：标题 16px/1.4、正文 14px/1.5、标签 12px/1.4，统一行高和文字大小 | `DESIGN.md`、`tailwind.config.js`、多个组件 | [DESIGN.md](../DESIGN.md) |
| 改进 | 前端 | 滑块组件间距优化：标题与轨道间距多次调整，最终稳定为合理间距 | `src/components/ColorAdjuster.tsx` | — |
| 改进 | 前端 | 滑块说明文字改为 info icon + tooltip，减少视觉干扰 | `src/components/ColorAdjuster.tsx` | — |
| 改进 | 前端 | 滑块键盘步进行为改进：移除自定义 handler，依赖原生 step 行为，强制使用 step 值 | `src/components/ColorAdjuster.tsx` | — |
| 改进 | 前端 | Toggle 开关无障碍改进：新增 role、aria-checked、aria-label、tabIndex 属性 | `src/components/Toggle.tsx` | — |
| 新功能 | 前端 | 新增 TextSwitch 组件：支持文字标签的二态切换（如"是/否"） | `src/components/TextSwitch.tsx` | — |
| 改进 | 前端 | 进程规则列表 UI 改进：优化卡片样式、圆角（12px→16px）、间距、悬停效果、"退出时恢复"改用 TextSwitch | `src/components/ConfigManager.tsx` | — |
| 改进 | 前端 | 进程选择器弹窗尺寸优化：从 400×500 调整为 500×600，列表高度 48→56，改进滚动条样式 | `src/components/ConfigManager.tsx` | — |
| 改进 | 前端 | 进程规则空状态增加说明文字："添加进程后，当其运行时自动切换到此方案" | `src/components/ConfigManager.tsx` | — |
| 改进 | 前端 | 配置管理器操作按钮可见性优化：移除 hover 才显示的逻辑，始终显示操作按钮 | `src/components/ConfigManager.tsx` | — |
| 改进 | 前端 | 设置弹窗样式调整：优化标题、分隔线、卡片间距 | `src/components/SettingsModal.tsx` | — |
| 改进 | 配置 | Tailwind 配置同步 DESIGN.md：字体大小、行高、圆角等 token 更新 | `tailwind.config.js` | — |
| 改进 | 文档 | 完善多 Agent 协作框架：`.agent/`（角色定义）、`.rules/`（工具规则）、`.skills/`（共享技能） | `.agent/`、`.rules/`、`.skills/` | [AGENTS.md](../AGENTS.md) |
| 改进 | 文档 | CodeGraph 工具使用规则：优先级说明、工具选择策略、WSL 环境配置 | `.claude/rules/codegraph.md`、`.claude/rules/codegraph-context-priority.md` | — |
| 新功能 | 文档 | 进程监听 API 文档完善 | `.docs/api/process_watcher.md` | [api/process_watcher.md](./api/process_watcher.md) |
| 新功能 | 文档 | 前端交接文档：icon 字段、进程监听 UI | `.docs/handoff/icon-field-frontend.md`、`.docs/handoff/process-watcher-frontend.md` | — |

---

### v0.2.9 — 2026-06-02 · 系统托盘常驻 + 全局快捷键 + 开机自启 + 配置重构

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
| 改进 | CI | 发布流程新增 AList 云盘自动上传：CI 构建后自动登录 AList 并将 exe 上传到夸克/阿里云盘的 `filter-manage/` 目录 | `.github/workflows/release.yml` | — |
| 新功能 | 后端 | 进程监听自动切换方案：后台线程轮询进程列表，匹配规则自动应用配色方案，进程退出恢复。新增 `ProcessRule` + 7 个 Tauri 命令 | `src-tauri/src/process_watcher.rs`、`config.rs`、`lib.rs` | [api/process_watcher.md](./api/process_watcher.md) |
| 重构 | 后端 | 进程监听重构为 WMI 事件驱动（`windows` crate 原生 `IWbemObjectSink`）+ 管理员权限（`build.rs` `requireAdministrator`）+ 按规则定向订阅；新增 `windows-core` 依赖；`WatcherStatus` 新增 `subscribed_processes` 字段 | `src-tauri/src/process_watcher.rs`、`build.rs`、`Cargo.toml` | [api/process_watcher.md](./api/process_watcher.md) |
| 修复 | 后端 | `list_running_processes` 中文进程名乱码。**根因**：使用 ANSI 版 ToolHelp32 API（GBK 字节被当 UTF-8 解码）。**修复**：改用 Unicode API `Process32FirstW` / `PROCESSENTRY32W` + `String::from_utf16_lossy` | `src-tauri/src/process_watcher.rs` | [api/process_watcher.md](./api/process_watcher.md) |
| 改进 | 后端 | `ColorConfig` 新增 `icon: Option<String>` 字段，支持为每个配色方案指定 Material Icon 名称，serde 兼容旧配置（缺失时自动为 `None`） | `src-tauri/src/config.rs` | [api/config.md](./api/config.md) |
| 改进 | 后端 | `list_configs` 返回类型由 `Vec<String>` 改为 `Vec<ColorConfig>`，前端一次调用获取全部配置（含 icon），无需逐个 `load_config` | `src-tauri/src/config.rs`、`tray.rs`、`shortcut.rs` | [api/config.md](./api/config.md) |
| 新功能 | 后端 | `get_running_processes` 返回进程图标（`RunningProcess.icon`），通过 `ExtractIconExW` + GDI + PNG 编码 base64 返回，同名进程去重缓存 | `src-tauri/src/process_watcher.rs` | [api/process_watcher.md](./api/process_watcher.md) |

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
| [api/process_watcher.md](./api/process_watcher.md) | 进程监听自动切换方案命令 |
| [updater-setup.md](./updater-setup.md) | 自动更新密钥生成与 CI 配置 |
