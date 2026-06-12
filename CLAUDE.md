# Project Guidelines

> **🚀 Multi-Agent Framework**
> 
> 本项目采用多 Agent 协作框架。你的职责、代码范围、协作规则都在框架中定义。
> 
> **请先读以下文件（按顺序）**：
> 1. **`AGENTS.md`** - Agent 注册表，找到你的角色
> 2. **`.agent/<your-role>.md`** - 你的详细职责文档
> 3. **`.rules/*.md`** - 统一工具规则（必读）
>
> 当前 Claude Code 的角色：**Universal (全栈)**  
> 详见：`.agent/universal.md`

---

## Tool Usage Preferences

### 代码读取（最高优先级）
- **优先使用 `mcp__codegraph__*` 系列工具**读取和理解代码：
  - `codegraph_context` — 了解模块架构、函数关系、调用链（首选）
  - `codegraph_node` — 查看单个符号的定义、调用者、被调用者
  - `codegraph_search` — 按名称搜索符号
  - `codegraph_trace` — 追踪两个符号之间的调用路径
  - `codegraph_explore` — 批量探索多个相关符号
- **codegraph 不够时降级使用** `Read`、`Grep`、`Glob` 等传统工具
- **适用场景**：理解代码结构、查找函数定义、追踪调用链、分析依赖关系、重构前评估影响范围

**详见**：`.rules/tools.md`

### Search & Research
- **优先使用 ctx7 技能** (`npx ctx7@latest`) 查询官方文档（API 语法、配置、版本迁移等）
- **ctx7 没找到的内容，降级使用 Tavily MCP** (`mcp__Tavily__tavily_search` 或 `mcp__Tavily__tavily_research`) 搜索
- 不要使用 `WebSearch`，统一使用 Tavily

### General Rules
- 代码读取优先级：CodeGraph MCP > Read/Grep/Glob > 直接源码
- 查询文档优先级：ctx7 > Tavily > WebSearch
- 遇到不熟悉的技术或 API，先查文档再写代码

## Project Overview
- **Tech Stack**: Tauri 2 (Rust backend) + React/Vite (TypeScript frontend) + Tailwind CSS
- **Purpose**: ICC color profile manager and NVIDIA color settings manager
- **Platform**: Windows 10/11

## Design System

**所有 UI 开发必须遵循 `DESIGN.md` 中的设计规范。**

- 颜色、字体、间距、圆角、组件样式均定义在 `DESIGN.md`
- Tailwind config 已同步所有 token（`tailwind.config.js`）
- 开发新 UI 时先读 `DESIGN.md`，确保视觉一致性
- 品牌风格：Quietly Powerful — 暗色调 + Glassmorphism + 精密感

## Agent 协作规则

本项目采用多 Agent 协作。详见 `AGENTS.md` 和 `.agent/` 目录。

**核心约束：**
1. **后端知识获取**：优先读 `.docs/api/*.md`，不够再读 `src-tauri/src/*.rs` 源码
2. **当前角色是 Universal（全栈）**：前端 + 后端都可以修改
3. **如果切换到其他角色**：严格遵循 `.agent/<role>.md` 的代码范围限制
4. **文档同步**：后端 API 变更必须同步更新文档，详见 `.rules/docs.md`
5. **Agent 交接**：需要协作时，在 `.docs/handoff/` 创建交接文档，详见 `.rules/handoff.md`

## WSL2 开发环境网络配置

> **重要：本项目运行在 WSL2 环境中，存在网络隔离。**

### 核心规则

当使用浏览器工具（如 Chrome DevTools MCP）调试或访问本地开发服务器时：

- ❌ **禁止使用 `localhost` 或 `127.0.0.1`**
- ✅ **必须使用 WSL 网关 IP `192.168.160.1` 访问 Windows 侧的服务**

### 示例

| 场景 | 错误地址 | 正确地址 |
|------|---------|---------|
| Vite/VitePress 开发服务器 | `http://localhost:5173/` | `http://192.168.160.1:5173/` |
| 其他 Windows 侧服务 | `http://localhost:<port>/` | `http://192.168.160.1:<port>/` |

### 原因

WSL2 和 Windows 有独立的网络栈。MCP 的无头 Chrome 运行在 WSL 内部，而开发服务器运行在 Windows 侧。WSL 内的 `localhost` 指向 WSL 自身，不是 Windows。必须通过 WSL 网关 IP（`192.168.160.1`）才能访问 Windows 侧的服务。

### 前提条件

Windows 侧的开发服务器必须监听 `0.0.0.0`（而非仅 `localhost`），否则 WSL 无法访问。对于 Vite/VitePress，启动时加 `--host 0.0.0.0` 参数。
