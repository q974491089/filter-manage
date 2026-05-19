# Filter Manage - ICC & NVIDIA Color Settings Manager

一个Windows桌面应用程序，用于管理ICC颜色配置文件和NVIDIA显卡颜色设置。

## 为什么选择 Filter Manage

市面上从不缺滤镜软件，也不缺游戏优化工具。那为什么我们还要做 Filter Manage？

因为我们发现，现有的工具，大多不够纯粹。

**那些"臃肿"的现状：** 我们调研了目前主流的 ICC 滤镜软件和工具，发现它们无一例外都变得越来越"重"——游戏加速、系统清理、硬件监控、甚至社交资讯……我本来只想安安静静调个画面，结果被强制塞了一堆用不上的功能。更糟糕的是，很多软件为了开发省事采用 Electron 架构，导致安装包动辄上百 MB，运行起来还吃内存。

**玩个游戏，凭什么这么累？** 你一定经历过这样繁琐的"仪式"：准备开黑时先打开颜色管理手动切换 ICC 配置文件；再打开 NVIDIA 控制面板逐项去拉亮度、对比度、伽马和数字振动；打完游戏想日常使用又得把参数改回来；下次再玩？对不起，请把上面的步骤重新来一遍。

**Filter Manage 的"纯粹美学"：** 我们不想做全能的"瑞士军刀"，我们只想做一把锋利的"手术刀"——几 MB 的轻量（拒绝 Electron，用 Rust 原生构建）、无感运行不常驻（软件退出后滤镜依然生效）、绝对零干扰（没有广告、弹窗和冗余功能）。

我们把选择权还给用户，把性能留给游戏。

## 功能特性

- **ICC配置文件管理**
  - 扫描系统ICC配置文件目录
  - 列出所有可用的ICC配置文件
  - 一键切换ICC配置文件
  - 导入/导出ICC配置文件

- **NVIDIA颜色设置**
  - 亮度调节（-125到125）
  - 对比度调节（-82到82）
  - 伽马调节（0.1到3.0）
  - 数字振动调节（0到100）

- **配置管理**
  - 保存当前设置为配置文件
  - 加载已保存的配置
  - 一键切换配置（游戏模式/正常模式）

- **预览功能**
  - 显示参考图片
  - 实时预览颜色调节效果

## 技术栈

- **前端**: React 18 + TypeScript + Tailwind CSS
- **后端**: Rust (Tauri 2.x)
- **打包**: Tauri 2.x (输出Windows安装包)

## 开发工作流程

**推荐：WSL开发 + Windows构建**

```
WSL中编写代码 → 同步到Windows → Windows中构建.exe
```

### 快速开始

#### 1. WSL中开发

```bash
# 前端调试（浏览器访问 http://localhost:5173）
npm run dev

# 同步到Windows
./sync-to-windows.sh
```

#### 2. Windows中构建

打开 **Windows PowerShell**：

```powershell
# 进入项目目录
cd C:\Users\你的用户名\Projects\filter-manage

# 安装依赖（首次）
npm install

# 开发模式（打开Windows桌面窗口）
npm run tauri dev

# 或者构建生产版本
npm run tauri build
```

构建完成后，安装包在：
```
src-tauri\target\release\bundle\
```

**详细工作流程请查看 [WORKFLOW.md](WORKFLOW.md)**

## 项目结构

```
filter-manage/
├── src/                    # 前端源码
│   ├── components/         # React组件
│   │   ├── ProfileList.tsx     # 配置文件列表
│   │   ├── ColorAdjuster.tsx   # 颜色调节器
│   │   ├── PreviewImage.tsx    # 预览图片
│   │   └── ConfigManager.tsx   # 配置管理
│   ├── App.tsx             # 主应用组件
│   └── main.tsx            # 入口文件
├── src-tauri/              # Rust后端
│   ├── src/
│   │   ├── main.rs         # 主入口
│   │   ├── lib.rs          # 库入口
│   │   ├── icc.rs          # ICC配置文件管理
│   │   ├── nvidia.rs       # NVIDIA设置管理
│   │   └── config.rs       # 配置管理
│   ├── Cargo.toml          # Rust依赖配置
│   └── tauri.conf.json     # Tauri配置
├── sync-to-windows.sh      # WSL同步到Windows脚本
├── windows-dev.bat         # Windows开发启动脚本
├── windows-build.bat       # Windows构建脚本
└── WORKFLOW.md             # 详细工作流程
```

## 快捷脚本

| 脚本 | 说明 |
|------|------|
| `./sync-to-windows.sh` | WSL中运行，同步项目到Windows |
| `windows-dev.bat` | Windows中运行，启动开发模式 |
| `windows-build.bat` | Windows中运行，构建生产版本 |

## NVIDIA颜色设置命令

项目使用以下Windows命令来控制NVIDIA颜色设置：

```batch
# 亮度设置 (-125到125，0为50%)
rundll32.exe NvCpl.dll,dtcfg setbrightness 1 all <value>

# 对比度设置 (-82到82，0为50%)
rundll32.exe NvCpl.dll,dtcfg setcontrast 1 all <value>

# 伽马设置 (直接使用控制面板的值)
rundll32.exe NvCpl.dll,dtcfg setgamma 1 all <value>

# 数字振动设置 (0到100)
rundll32.exe NvCpl.dll,dtcfg setdvc all <value>
```

## 参考项目

- [NvidiaDisplayController](https://github.com/therealmariolaurianti/NvidiaDisplayController) - C#实现的NVIDIA显示设置管理器
- [DisplayProfileManager](https://github.com/zac15987/DisplayProfileManager) - Windows显示配置文件管理器
- [vibranceGUI](https://github.com/juv/vibranceGUI) - 自动数字振动控制

## 许可证

MIT License
