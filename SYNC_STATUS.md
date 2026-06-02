【文档已完成同步更新】2026-05-24 — v0.2.6 集成 tauri-plugin-opener 插件、补全 updater/process/opener 权限；CHANGELOG 与 docs/README 已同步追加 v0.2.6 条目

【文档已完成同步更新】2026-05-28 — 新增系统托盘常驻、全局快捷键绑定方案、开机自启功能（后端部分）

【文档已完成同步更新】2026-05-31 — 修复托盘菜单运行时不刷新（移除 tauri.conf.json 重复 trayIcon 配置 + 托盘改用 with_id("main")）与 list_configs 误列 __settings__ 两个问题；docs/api/config.md 与 docs/README.md 已同步

【文档已完成同步更新】2026-05-31 — 新增 pause_shortcuts/resume_shortcuts 命令，修复录制已绑定快捷键无反应问题（前端待接入）；docs/api/config.md、docs/README.md、docs/handoff/shortcut-recording-pause-frontend.md 已同步

【文档已完成同步更新】2026-05-31 — 托盘新增双击打开主窗口 + 「恢复默认」菜单项（tray.rs，无新命令）；docs/README.md 迭代记录已同步

【文档已完成同步更新】2026-05-31 — 修复点击关闭无弹窗：AppSettings 新增 close_prompted，未选择过关闭行为时仅 prevent_close 交前端弹窗（前端待接入）；docs/api/config.md、docs/README.md、docs/handoff/close-prompt-frontend.md 已同步

【文档已完成同步更新】2026-05-31 — 修复托盘/快捷键应用方案时 ICC 不生效（改为直接同步调用 icc::apply_icc_profile，不再 block_on async）；docs/README.md 迭代记录已同步（无新命令/无接口变更）

【文档已完成同步更新】2026-05-31 — 修复快捷键「恢复默认」不重置 ICC：apply_color_config 改为 ICC先/NVIDIA后、icc 为 null 时恢复 sRGB（新增同步 icc::restore_default_icc）；docs/README.md 已同步（无接口变更）

【文档已完成同步更新】2026-05-31 — 修复孤儿清理误删 __default__ 绑定 + 新增 config-applied 事件通知前端同步 UI（前端待接入）；docs/README.md、docs/handoff/config-applied-event-frontend.md 已同步

【文档已完成同步更新】2026-06-01 — 快捷键切换方案时发送 Windows 系统通知（tauri-plugin-notification）；docs/README.md 已同步

【文档已完成同步更新】2026-06-01 — 改用 windows crate WinRT API 直接发 Toast 通知并设 tag，实现连续切换时新通知顶替旧通知（移除 tauri-plugin-notification）

【文档已完成同步更新】2026-06-01 — 修复连续切换时第二条通知不弹出：改为每次用递增 tag + 发前先 Remove 上一条，确保每次都弹出且不堆叠

【文档已完成同步更新】2026-06-01 — AppSettings 新增 shortcut_notification 字段（通知开关，默认 true）；交接文档 docs/handoff/shortcut-notification-toggle-frontend.md 已创建
【文档已完成同步更新】2026-06-02 — 新增 rename_config 命令（原子重命名配置预设）
【文档已完成同步更新】2026-06-02 — 配置存储重构为单文件 profiles.json，兼容旧格式迁移
【文档已完成同步更新】2026-06-02 — 合并 profiles.json + __settings__.json 为单文件 app.json