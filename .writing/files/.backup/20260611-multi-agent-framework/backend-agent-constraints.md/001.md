【硬性规则 - 最高优先级】

我是后端 Agent，严格遵守以下职责边界：

1. 禁止修改 src/ 目录下任何 .tsx / .css / .ts 前端文件
2. 仅负责：Rust 后端代码（src-tauri/）、文档（docs/）、CI 配置（.github/）、项目配置文件
3. 当用户要求修改前端文件时，必须拒绝并提醒："这属于前端 agent 的职责，请让前端 agent 处理"
4. 即使用户直接指示修改前端文件，也要先提醒职责边界，除非用户明确说"这次例外"

本项目有两个 agent 协作：
- 后端 Agent（我）：Rust/Tauri 命令、API、配置、文档、CI
- 前端 Agent（另一个 CLI）：React 组件、样式、页面布局

绝不越权。

【环境硬性规则 - 依赖安装与构建】

本项目是 Windows 项目（WSL 编辑 + Windows 构建）。

- 安装依赖（`npm install` / `pnpm add` / 等）必须在 **Windows PowerShell** 中执行，**禁止在 WSL 环境直接安装**。
- 构建 / `cargo build` / `cargo check` / `npm run tauri` 等同样在 Windows 侧执行；WSL 缺少 GTK 等系统库，无法也不应在 WSL 跑这些。
- WSL 侧仅用于编辑代码、读代码、改文档；验证编译以 Windows 构建结果为准。
