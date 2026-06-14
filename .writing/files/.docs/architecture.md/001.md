# 架构概览

## 技术栈

| 层 | 技术 |
|----|------|
| 前端 | React 18 + TypeScript + Tailwind CSS + Vite |
| 后端 | Rust + Tauri 2.x |
| 包管理 | pnpm |
| 平台 | Windows 10/11 |

## 项目结构

```
filter-manage/
├── src/                        # 前端
│   ├── components/
│   │   ├── ProfileList.tsx     # ICC 列表（含搜索、导入、导出）
│   │   ├── ColorAdjuster.tsx   # NVIDIA 颜色滑块
│   │   ├── PreviewImage.tsx    # 图片预览
│   │   ├── ConfigManager.tsx   # 配置预设管理
│   │   └── SaveModal.tsx       # 保存配置弹窗
│   └── App.tsx                 # 主布局 + 显示器选择
├── src-tauri/src/
│   ├── icc.rs                  # ICC 管理（含导入/导出/搜索）
│   ├── nvidia.rs               # NVIDIA 颜色调节（gamma ramp）
│   ├── config.rs               # 配置预设持久化
│   └── lib.rs                  # 命令注册入口
└── docs/                       # 本文档
    ├── api/                    # 后端 API 文档
    └── frontend-guide.md       # 前端开发指南
```

## 数据流

```
用户操作（前端）
    ↓ invoke()
Tauri 命令（Rust）
    ↓
Windows API / 文件系统
    ├── SetDeviceGammaRamp  → 显卡 LUT（NVIDIA 颜色 + ICC vcgt 叠加）
    ├── WCS API             → 系统颜色管理注册
    ├── NVAPI               → 数字振动
    └── %APPDATA%\filter-manage\  → 配置预设 JSON
```

## ICC 与 NVIDIA 颜色的叠加机制

1. 应用 ICC 时，解析文件中的 `vcgt` 标签，写入显卡 LUT，同时保存到 `ICC_BASE_RAMP`
2. 调节 NVIDIA 亮度/对比度/伽马时，在 `ICC_BASE_RAMP` 基础上叠加计算，再写入 LUT
3. 两者互不覆盖，切换 ICC 后 NVIDIA 调节值保持不变

## 配置存储路径

- 颜色配置预设：`%APPDATA%\filter-manage\<name>.json`
- 系统 ICC 目录：`C:\Windows\System32\spool\drivers\color\`
