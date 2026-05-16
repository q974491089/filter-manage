# 开发工作流程

## WSL开发 + Windows构建

### 1. 在WSL中开发

```bash
# 进入项目目录
cd /home/myuser/Project/fillter-manage

# 启动前端开发服务器（纯前端调试，不需要Tauri）
npm run dev
```

访问 http://localhost:5173 查看前端界面。

### 2. 同步到Windows

```bash
# 同步项目到Windows文件系统
./sync-to-windows.sh

# 或者指定Windows路径
./sync-to-windows.sh /mnt/c/Users/你的用户名/Projects/filter-manage
```

### 3. 在Windows中构建

打开 **Windows PowerShell**：

```powershell
# 进入项目目录
cd C:\Users\你的用户名\Projects\filter-manage

# 安装依赖
npm install

# 启动Tauri开发（会打开Windows桌面窗口）
npm run tauri dev

# 或者构建生产版本
npm run tauri build
```

构建完成后，安装包在：
```
C:\Users\你的用户名\Projects\filter-manage\src-tauri\target\release\bundle\
```

---

## 完整工作流程图

```
┌─────────────────────────────────────────────────────────────┐
│                        WSL (Linux)                          │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  编辑代码 (VS Code / Vim / etc)                     │   │
│  │  - src/**/*.tsx (前端)                               │   │
│  │  - src-tauri/src/**/*.rs (后端)                      │   │
│  └─────────────────────────────────────────────────────┘   │
│                          │                                  │
│                          ▼                                  │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  npm run dev (前端调试)                              │   │
│  │  http://localhost:5173                              │   │
│  └─────────────────────────────────────────────────────┘   │
│                          │                                  │
│                          ▼                                  │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  ./sync-to-windows.sh                              │   │
│  │  同步到 Windows 文件系统                            │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                       Windows                               │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  npm run tauri dev (Windows桌面应用调试)             │   │
│  │  - 打开原生Windows窗口                              │   │
│  │  - 可以调用NVIDIA API                               │   │
│  │  - 可以管理ICC配置文件                              │   │
│  └─────────────────────────────────────────────────────┘   │
│                          │                                  │
│                          ▼                                  │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  npm run tauri build (构建生产版本)                 │   │
│  │  输出: .exe 安装包                                  │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

---

## 快捷命令

### WSL中

```bash
# 前端开发
npm run dev

# 同步到Windows
./sync-to-windows.sh
```

### Windows PowerShell中

```powershell
# 安装依赖（首次）
npm install

# 开发模式（打开Windows桌面窗口）
npm run tauri dev

# 构建生产版本
npm run tauri build
```

---

## 注意事项

1. **WSL中无法运行Tauri窗口** - 只能在Windows中运行
2. **前端可以在WSL中调试** - 使用 `npm run dev`，访问 localhost:5173
3. **每次修改代码后需要重新同步** - 运行 `./sync-to-windows.sh`
4. **Windows中需要安装Node.js和Rust** - 首次构建前需要安装
