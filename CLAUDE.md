# Project Guidelines

## Tool Usage Preferences

### Search & Research
- **优先使用 ctx7 技能** (`npx ctx7@latest`) 查询官方文档（API 语法、配置、版本迁移等）
- **ctx7 没找到的内容，降级使用 Tavily MCP** (`mcp__Tavily__tavily_search` 或 `mcp__Tavily__tavily_research`) 搜索
- 不要使用 `WebSearch`，统一使用 Tavily

### General Rules
- 查询文档优先级：ctx7 > Tavily > WebSearch
- 遇到不熟悉的技术或 API，先查文档再写代码

## Project Overview
- **Tech Stack**: Tauri 2 (Rust backend) + React/Vite (TypeScript frontend) + Tailwind CSS
- **Purpose**: ICC color profile manager and NVIDIA color settings manager
- **Platform**: Windows 10/11

## Design System

**所有 UI 开发必须遵循 `DESIGN.md` 中的 Lumina Pro 设计规范。**

- 颜色、字体、间距、圆角、组件样式均定义在 `DESIGN.md`
- Tailwind config 已同步所有 token（`tailwind.config.js`）
- 开发新 UI 时先读 `DESIGN.md`，确保视觉一致性
- 品牌风格：Quietly Powerful — 暗色调 + Glassmorphism + 精密感

## Agent 协作规则

本项目采用多 Agent 协作。详见 `docs/agents.md`。

**核心约束：**
1. **后端知识获取**：优先读 `docs/api/*.md`，不够再读 `src-tauri/src/*.rs` 源码
2. **切勿修改后端代码**（`src-tauri/` 目录）— 需要修改时提建议，由 Backend Agent 执行
3. **前端代码范围**：`src/`、`tailwind.config.js`、`index.css`、`package.json`
4. **文档共管**：`docs/` 目录前后端 Agent 都可以修改
