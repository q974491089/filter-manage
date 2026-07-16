# AI 编码规范

所有 Agent（Claude / Codex / Grok / OpenCode / Qoder 等）修改本仓库代码时必须遵守。  
目标：**可维护、可验证、不把编译器警告和半成品留给用户。**

---

## 1. 零警告交付（强制）

### 原则

- **Rust**：`cargo check` / `tauri dev` 编译输出中 **不得新增** `warning:`。
- **TypeScript**：不引入新的明显 lint/tsc 错误；改完相关前端后尽量能通过 `tsc`（或项目既有检查）。
- 声称「做完 / 修好了」之前，**必须**用证据验证（见第 6 节），不能只靠“应该没问题”。

### 常见 Rust 警告与正确处理

| 警告 | 原因 | 正确做法 |
|------|------|----------|
| `unused import` | import 了未使用的类型/函数 | **删除** import，不要 `allow` |
| `dead_code`（未使用函数/枚举变体） | 写了却没接线 | **接上真实调用**，或删除；不要长期留死代码 |
| `private_interfaces` | `pub fn` 参数/返回值类型更私有 | 收紧可见性：`fn` / `pub(crate)`，只对外暴露真正需要的 API |
| `non_snake_case`（COM / Windows API） | 接口参数名必须与 IDL 一致 | **保留原名**，在对应 `impl` 上加 `#[allow(non_snake_case)]` 并注释原因 |
| `unused_variables` | 绑定未用 | 前缀 `_` 或删除；不要无意义绑定 |

### 允许 `#[allow(...)]` 的情况（须注释）

仅当 **语言/平台契约强制** 时：

```rust
// COM 接口参数名必须与 Windows IDL 一致（PascalCase），不能改成 snake_case
#[allow(non_snake_case)]
impl IWbemObjectSink_Impl for EventSink_Impl {
    fn Indicate(&self, lObjectCount: i32, apObjArray: *const ...) -> ... { ... }
}
```

**禁止**：

- 为了省事对整文件 `#![allow(dead_code, unused, ...)]`
- 用 `allow` 掩盖「还没写完」的代码
- 留下「以后再用」的未引用函数而不接线（例如写了 `stop_watcher` 却从不在退出路径调用）

### 不是 Tauri 的锅

`tauri dev` 里刷出的 `warning: ... --> src\...` 是 **rustc / cargo** 的警告，不是 Tauri 框架自带噪音。  
改的是我们的 `src-tauri/src/*.rs`，就由我们修干净。

---

## 2. 改代码时的质量底线

### 只改任务需要的代码

- 不顺手大重构、不扩 scope、不「顺便」改无关文件。
- 不主动写用户没要的 markdown / 文档，**除非**任务本身是文档，或 `.rules/docs.md` 要求的 API/迭代同步。

### 匹配项目既有风格

- 先读周围代码再写：命名、错误处理、模块边界、日志前缀。
- 复用已有工具函数与模式（例如进程监听统一 `[process-watcher]` 日志，而不是另起一套）。

### 可见性与 API 面

- 默认私有；需要跨模块再用 `pub(crate)` / `pub`。
- Tauri command、给前端的类型字段变更 → 同步 `.docs/api/`（见 `docs.md`）。

### 错误与静默失败

- 不要用 `let _ =` 吞掉关键失败而不留痕迹（至少 `eprintln!` / 项目日志前缀）。
- 用户可感知的失败路径优先：日志 + 必要时 Toast / 状态字段。

### 前后端事件竞态（本项目常见坑）

- 后端在 `setup` 里可能 **早于** WebView `listen` 就绪就 `emit`。
- 启动时若会 `emit("config-applied")` 等：前端挂载后应能 **从状态查询补同步**（如 `get_watcher_status`），不能只依赖一次性事件。
- 状态写入顺序：先更新可查询状态，再 `emit`（减少 status 与事件不一致）。

---

## 3. Rust / Tauri 专用

### 环境

- **构建与 `cargo` / `tauri` 在 Windows 执行**（见 `WORKFLOW.md`）；WSL 可改代码、读文档，不要用 WSL 目标编译本应用。
- 验证示例：

```powershell
cd src-tauri
cargo check
```

或项目根：`pnpm run tauri dev` / `npm run tauri dev`（用户本地启动，Agent 勿擅自开长驻 dev，除非用户要求）。

### Windows / COM / Win32

- 实现 COM trait 时 **参数名跟 IDL**，用 `#[allow(non_snake_case)]` 标注该 impl。
- 不安全块保持最小；句柄/COM 对象注意释放与取消订阅。
- 进程监听等后台线程：断线要有自愈或明确生命周期，禁止「channel 断了就永久空转」且无日志。

### 依赖

- 不擅自加依赖；若必须加，说明理由并改 `Cargo.toml` / 锁文件（Windows 侧构建更新锁文件）。

---

## 4. 前端（React / TypeScript）专用

- UI 遵循 `DESIGN.md` 与现有 Tailwind token。
- 与后端契约以 `.docs/api/*.md` 为准；新增 invoke 字段要可选兼容或同步文档与类型。
- 不要用 `any` 掩盖类型问题（除非边界层且范围极小）。

---

## 5. 日志约定

| 模块 | 前缀示例 |
|------|----------|
| 进程监听 | `[process-watcher]` |
| ICC | 现有 `ICC:` 风格 |
| 前端调试 | 可用 `[App]` 等，避免刷屏 |

- 状态变化、订阅成功/失败、自动重连、关键 apply 失败：应有日志。
- 不要在热循环里每 tick 打日志。

---

## 6. 完成前验证（强制）

在宣称完成前至少做到：

1. **编译**：改了 `src-tauri` → Windows 上 `cargo check` 无 error、无 **新** warning。  
2. **逻辑**：相关路径手动或日志可核对（例如进程监听：Started / Stopped / reconcile）。  
3. **文档**：API 行为变了 → 按 `.rules/docs.md` 更新 `.docs/api/` 与迭代记录。  
4. **Git**：只 stage 本任务文件；commit 信息遵循 `.rules/git.md`。

验证命令要真实跑过；输出异常时不得声称已通过。

---

## 7. 与其他规则的关系

| 规则 | 文件 |
|------|------|
| 工具 / CodeGraph | `.rules/tools.md` |
| 文档同步 | `.rules/docs.md` |
| 交接 | `.rules/handoff.md` |
| Git | `.rules/git.md` |
| 子 Agent | `.rules/subagent-dispatch.md` |
| **编码质量与零警告（本文）** | `.rules/coding.md` |

优先级（质量相关）：

```
安全与敏感信息（AGENTS.md）
  > 编码零警告与可验证完成（本文）
  > 文档同步 / Git 规范
  > 风格偏好
```

---

## 8. 反模式速查

| 不要 | 要 |
|------|-----|
| 留下一堆 `warning:` 就交付 | `cargo check` 干净再收工 |
| 整文件 `allow(dead_code)` | 删除或接线 |
| 把 COM 参数强行改成 snake_case 导致编不过 | IDL 名 + 局部 allow |
| 只 `emit`、前端启动竞态丢事件 | 可查询状态 + 挂载补同步 |
| 关键路径 `let _ =` 静默失败 | 日志 / 状态字段 |
| 未经用户要求开长驻 `tauri dev` | 用户本地开；Agent 改代码即可 |

---

**更新于**：2026-07-16  
**来源**：WMI 进程监听可靠性改造中的编译警告清理与 UI 竞态修复经验沉淀。
