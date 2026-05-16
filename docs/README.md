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

## 固定参考文档

| 文档 | 内容 |
|------|------|
| [architecture.md](./architecture.md) | 技术栈、项目结构、数据流、ICC×NVIDIA叠加机制 |
| [frontend-guide.md](./frontend-guide.md) | 前端组件结构、新功能接入清单、常用 import |
| [api/icc.md](./api/icc.md) | ICC 全部命令 |
| [api/nvidia.md](./api/nvidia.md) | NVIDIA 颜色全部命令 |
| [api/config.md](./api/config.md) | 配置预设全部命令 |
