# Agent 协作规范

本项目采用多 Agent 协作模式。各 Agent 有明确职责边界，通过文档和代码约定协作。

## Agent 角色

| Agent | 职责 | 代码范围 |
|-------|------|----------|
| **Frontend Agent** (Claude Code) | React/TypeScript 前端开发、UI/UX、Tailwind 样式 | `src/`、`tailwind.config.js`、`index.css`、`vite.config.ts` |
| **Backend Agent** | Rust/Tauri 后端开发、Windows API 集成 | `src-tauri/` |

## 协作规则

### Frontend Agent (当前 Agent)

**职责：**
- 所有前端代码开发（React 组件、样式、交互）
- 读取 `docs/api/*.md` 了解后端 API 接口
- 调用 `invoke()` 与后端交互

**代码阅读规范（强制）：**

> **⚠️ 核心原则：直接回答，不要委托探索**
> 
> 对于"X 怎么工作的"、架构、调用链、"X 在哪"等问题，**直接回答**——通常只需 **一次 `codegraph_explore` 调用**。

**codegraph 工具选择策略：**

| 意图 | 工具 |
|------|------|
| **几乎所有问题**——"X 怎么工作"、架构、bug、"X 在在哪" | `codegraph_explore`（**首选，第一个调用**） |
| **"X 怎么到达 Y？/ 流程"** | `codegraph_explore`，命名跨越流程的 symbol |
| **"X 这个 symbol 叫什么？"（只要位置）** | `codegraph_search` |
| **"谁调用了它？" / "它调用了谁？"** | `codegraph_callers` / `codegraph_callees` |
| **看某个特定 symbol 的完整源码** | `codegraph_node(includeCode=true)` |
| **"目录 X 里有什么？"** | `codegraph_files` |

**反模式（禁止）：**
- ❌ 不要用 grep 重新验证 codegraph 的结果
- ❌ 不要先 grep 再查 symbol
- ❌ 不要用 `codegraph_search` + `codegraph_node` 链式调用来理解一个区域
- ❌ 不要循环调用 `codegraph_node`

**后端知识获取优先级：**
1. **优先读取 `docs/api/` 目录** — API 文档是最权威的接口参考
2. **其次读取 `src-tauri/src/*.rs`** — 当文档不够详细时，读取后端源码理解实现细节
3. **切勿修改后端代码** — 如需修改后端，提出修改建议，由 Backend Agent 执行

**后端代码修改建议格式：**
```
建议修改文件：src-tauri/src/xxx.rs
修改原因：[为什么需要改]
修改内容：[具体改什么]
影响范围：[会影响哪些前端功能]
```

### Backend Agent

**职责：**
- Rust/Tauri 后端开发
- Windows API 集成（NVIDIA、ICC、WCS）
- 维护 `docs/api/*.md` 文档
- 根据 Frontend Agent 的建议修改后端代码

**文档维护义务：**
- 新增或修改 Tauri 命令后，**必须同步更新** `docs/api/` 中对应的文档
- 保持文档与代码一致

## 通信协议

### 前端 → 后端（需求传递）

当前端需要新的后端能力时，按以下格式提出：

```
【前端需求】
功能：[功能描述]
调用方式：invoke("command_name", { param: type })
期望返回：{ field: type }
使用场景：[在哪个组件/交互中使用]
```

### 后端 → 前端（API 变更通知）

当后端 API 发生变更时，在 `docs/api/` 中更新文档，并在 commit message 中标注 `[API CHANGE]`。

## 文件边界

| 操作 | Frontend Agent | Backend Agent |
|------|---------------|---------------|
| 修改 `src/` | 可以 | 不可以 |
| 修改 `src-tauri/` | 不可以（提建议） | 可以 |
| 修改 `docs/` | 可以 | 可以 |
| 修改 `tailwind.config.js` | 可以 | 不可以 |
| 修改 `package.json` | 可以 | 不可以 |
| 修改 `Cargo.toml` | 不可以 | 可以 |
