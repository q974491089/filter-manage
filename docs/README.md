# Filter Manage — 项目迭代目录

每次功能更新在此追加一条记录。读文档时先看这里定位，再按链接跳转具体文档。

---

## 迭代记录

### v0.1 — 2026-05-13 · 项目初始化

基础框架搭建，四大核心模块上线。

| 模块 | 说明 | 文档 |
|------|------|------|
| ICC 管理 | 扫描系统 ICC 目录、切换配置文件、恢复默认 sRGB | [api/icc.md](./api/icc.md) |
| NVIDIA 颜色 | 亮度/对比度/伽马/数字振动实时调节，ICC vcgt 叠加 | [api/nvidia.md](./api/nvidia.md) |
| 配置管理 | 颜色配置预设保存/加载/删除，支持启动默认值 | [api/config.md](./api/config.md) |
| 显示器枚举 | 读取 EDID 获取显示器型号，支持多显示器切换 | [api/icc.md](./api/icc.md) |

---

### v0.2 — 2026-05-16 · ICC 导入导出 + 图片预览 + 搜索

新增 4 个后端接口，前端待实现。

| 功能 | 说明 | 文档 |
|------|------|------|
| 打开 ICC 目录 | 用资源管理器打开系统 ICC 文件夹 | [api/icc.md#open_icc_directory](./api/icc.md) |
| ICC 搜索 | 按名称关键字过滤 ICC 列表 | [api/icc.md#search_icc_profiles](./api/icc.md) |
| ICC 导入 | 拖拽或点击选文件 → 复制到系统 ICC 目录 | [api/icc.md#import_icc_profile](./api/icc.md) |
| ICC 导出 | 将系统 ICC 文件导出到用户指定目录 | [api/icc.md#export_icc_profile](./api/icc.md) |
| 图片预览上传 | 验证本地图片路径，供前端 `<img>` 显示 | [api/icc.md#set_preview_image](./api/icc.md) |
| 前端接入说明 | 拖拽实现方式、`convertFileSrc` 用法 | [frontend-guide.md](./frontend-guide.md) |

---

### v0.2.1 — 2026-05-17 · 前端样式优化

优化前端界面整体样式展示效果。

| 功能 | 说明 | 文档 |
|------|------|------|
| 样式优化 | 颜色调节器、配置管理、预览图片等组件样式改进 | — |
| 深色模式 | 优化深色模式下的视觉效果 | — |

---

### v0.3 — 2026-05-19 · 在线自动更新

启动时自动检查 GitHub Releases 新版本，用户确认后下载安装并重启。

| 功能 | 说明 | 文档 |
|------|------|------|
| 自动更新 | 启动时检查新版本，下载安装后重启 | [updater-setup.md](./updater-setup.md) |

---

### v0.3.1 — 2026-05-21 · DVC 多显示器支持

数字振动(DVC)支持按显示器独立设置，不再写死第一个显示器。

| 功能 | 说明 | 文档 |
|------|------|------|
| DVC 多显示器 | `set_nvidia_digital_vibrance` 按 deviceId 设置对应显示器 | [api/nvidia.md](./api/nvidia.md) |
| sync/default 多显示器 | `sync_dvc_from_driver`、`get_dvc_default_ui_value` 新增 deviceId 参数 | [api/nvidia.md](./api/nvidia.md) |

---

### v0.2.6 — 2026-05-24 · 更新通知改造 + opener 集成

更新提示 UI 升级为弹窗，新增 30 天 snooze；后端集成 `tauri-plugin-opener` 用于打开外部链接。

| 功能 | 说明 | 文档 |
|------|------|------|
| opener 插件集成 | 注册 `tauri-plugin-opener`，前端通过 `@tauri-apps/plugin-opener` 调用 | [handoff/opener-frontend.md](./handoff/opener-frontend.md) |
| 更新弹窗 | 自动更新提示由 Banner 改为 Modal，展示完整 release notes | — |
| 更新 snooze | 用户可选择「30 天内不再提醒」 | — |
| 权限补全 | `capabilities/default.json` 添加 `updater:default`、`process:allow-restart`、`opener:default` | — |

---

## 固定参考文档

| 文档 | 内容 |
|------|------|
| [architecture.md](./architecture.md) | 技术栈、项目结构、数据流、ICC×NVIDIA叠加机制 |
| [frontend-guide.md](./frontend-guide.md) | 前端组件结构、新功能接入清单、常用 import |
| [api/icc.md](./api/icc.md) | ICC 全部命令 |
| [api/nvidia.md](./api/nvidia.md) | NVIDIA 颜色全部命令 |
| [api/config.md](./api/config.md) | 配置预设全部命令 |
| [updater-setup.md](./updater-setup.md) | 自动更新密钥生成与 CI 配置 |
