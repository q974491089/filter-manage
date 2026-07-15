# 后端 API — 进程监听

源文件：`src-tauri/src/process_watcher.rs`

## 概述

进程监听模块通过 WMI（Windows Management Instrumentation）事件订阅，实时监听规则中指定进程的启动和退出，自动切换配色方案。进程退出后可选恢复**默认方案**（用户保存的默认配置；无则系统 sRGB + DVC 50%）。应用以管理员权限运行。

**可靠性设计（2026-07-16）**：WMI 是**唯一持续事件源**——不引入应用层周期进程轮询，不引入 ETW。订阅失败或 event channel 断开后，主 watcher 线程按指数退避自动重订（无需重启应用）。诊断日志统一前缀 `[process-watcher]`。

## 数据类型

```ts
interface ProcessRule {
  id: string                    // UUID
  process_name: string          // 进程名（不区分大小写），如 "delta_force.exe"
  config_name: string           // 绑定的预设名
  enabled: boolean              // 是否启用
  restore_on_exit: boolean      // 进程退出时是否恢复默认方案（默认 true）
}

interface RunningProcess {
  name: string     // 进程名，如 "chrome.exe"
  pid: number      // 进程 ID
  icon: string | null  // 进程图标（base64 PNG data URL），提取失败时为 null
}

interface WatcherStatus {
  enabled: boolean                    // 总开关状态
  active_rule: ProcessRule | null     // 当前激活的规则
  active_config_name: string | null   // 当前激活的方案名
  subscribed_processes: string[]      // 当前 WMI 订阅的进程名列表
  // ── 健康/可观测字段（WMI 可靠性加固）──
  wmi_connected: boolean              // 当前是否持有有效事件订阅（Live）
  last_error: string | null           // 最近一次订阅/断开错误；订阅成功时清空
  reconnect_attempt: number           // 当前退避重连次数；Live 时为 0
}
```

> **字段落地说明**：`wmi_connected` / `last_error` / `reconnect_attempt` 为计划字段（见 `.docs/plans/2026-07-16-process-watcher-wmi-reliability.md`）。若实现时 Rust 侧命名或类型有微调，以 `process_watcher.rs` 中 `WatcherStatus` 为准并回写本文档。前端旧代码可忽略新字段（serde 多字段无妨）。

## AppSettings 扩展字段

```ts
interface AppSettings {
  // ... 现有字段
  process_watcher_enabled: boolean   // 进程监听总开关（默认 true）
  process_notification: boolean      // 自动切换时是否弹 Toast（默认 true）
  process_rules: ProcessRule[]       // 规则列表
}
```

**更新于**：2026-06-09 — 新增 `process_watcher_enabled`、`process_notification`、`process_rules` 字段

## 命令列表

### `get_process_rules`
获取所有进程监听规则。

```ts
const rules = await invoke<ProcessRule[]>('get_process_rules')
```

---

### `add_process_rule`
新增进程监听规则。保存后自动触发 WMI 重订阅。

```ts
await invoke('add_process_rule', {
  rule: {
    id: crypto.randomUUID(),
    process_name: 'delta_force.exe',
    config_name: '三角洲方案',
    enabled: true,
    restore_on_exit: true,
  }
})
```

- `process_name` 为空 → 返回错误
- `config_name` 不存在 → 返回错误 `"Config '...' not found"`
- 同一进程名已存在 → 返回错误 `"进程 '...' 已存在规则"`（不区分大小写）

---

### `update_process_rule`
更新已有规则（按 `id` 匹配）。保存后自动触发 WMI 重订阅。

```ts
await invoke('update_process_rule', { rule: updatedRule })
```

- `id` 不存在 → 返回错误

---

### `delete_process_rule`
删除规则。保存后自动触发 WMI 重订阅。

```ts
await invoke('delete_process_rule', { id: 'uuid-string' })
```

- `id` 不存在 → 返回错误

---

### `get_running_processes`
枚举当前系统运行中的进程列表，供前端展示进程选择器。每个进程附带提取的应用图标（base64 PNG data URL），同名进程仅提取一次图标（按进程名去重缓存）。

```ts
const processes = await invoke<RunningProcess[]>('get_running_processes')
```

仅在前端打开进程选择器时调用（一次性快照），非 Windows 返回空数组。

使用 Windows ToolHelp32 Unicode API（`Process32FirstW` / `PROCESSENTRY32W`），正确支持中文进程名。图标通过 `ExtractIconExW` 从 exe 文件提取，经 GDI 读取像素 + PNG 编码 + base64 返回。

**更新于**：2026-06-11 — `RunningProcess` 新增 `icon` 字段（base64 PNG data URL）

---

### `set_process_watcher_enabled`
开关进程监听总控。关闭时取消 WMI 订阅（零开销），开启时重新订阅。关闭时会清空 `active_rule`，并在原规则 `restore_on_exit` 为 true 时恢复默认方案。

```ts
await invoke('set_process_watcher_enabled', { enabled: true })
```

---

### `get_watcher_status`
获取当前进程监听运行状态（含 WMI 连接健康字段）。

```ts
const status = await invoke<WatcherStatus>('get_watcher_status')
// status.wmi_connected / status.last_error / status.reconnect_attempt
```

---

## 工作原理

1. 应用以 `requireAdministrator` 权限启动，UAC 提升后运行
2. `init_watcher()` 创建主 watcher 线程
3. 读取启用规则，生成 WQL 查询，通过 `windows` crate 原生 WMI API（`IWbemLocator` → `IWbemServices` → `ExecNotificationQueryAsync` + `IWbemObjectSink` 回调）创建事件订阅
4. WMI 监听线程接收 `__InstanceOperationEvent`，区分 creation/deletion 事件，转发到主 watcher 线程
5. 匹配规则（first-match-wins）→ `tray::apply_color_config` + emit `config-applied` + Toast
6. 进程退出 + `restore_on_exit: true` → `tray::apply_default_config()` 恢复默认方案
7. 规则变化时自动取消旧订阅 + 创建新订阅（动态重订阅）
8. 无启用规则时不创建 WMI 订阅，零开销
9. **订阅后对账（reconcile）**：WMI 的 `__InstanceOperationEvent` 只上报订阅之后的启动/退出事件，不补发订阅前已在运行进程的「启动」事件。因此在每次（初始 / 重新）订阅成功后，会扫描一次当前进程名，对第一个正在运行的受监听进程合成一次 `Started` 事件（复用 `handle_event`），从而登记 `active_rule` 并应用配色。这修复了「先开游戏后开应用时，游戏退出不触发恢复默认方案」的问题。

**WQL 查询示例**（按规则定向订阅）：
```sql
SELECT * FROM __InstanceOperationEvent WITHIN 1
WHERE TargetInstance ISA 'Win32_Process'
AND (TargetInstance.Name = 'delta_force.exe' OR TargetInstance.Name = 'cs2.exe')
```

---

## 可靠性行为

> 设计目标：消除「WMI 静默死亡后必须重启应用才恢复」的问题。详见计划
> [2026-07-16-process-watcher-wmi-reliability.md](../plans/2026-07-16-process-watcher-wmi-reliability.md)。

### 事件源边界

| 做什么 | 不做什么 |
|--------|----------|
| WMI `__InstanceOperationEvent` 作为**唯一持续**事件源 | ❌ 应用层周期扫进程表作主路径 |
| 订阅成功后**一次性** `reconcile_running_processes` | ❌ ETW / Kernel-Process |
| 收到 `Stopped` 时**事件触发**的一次进程名快照（多实例防护） | ❌ 定时轮询健康检查 |

### 订阅状态机与自动重订

```
Idle ──subscribe_ok──► Live
  ▲                      │
  │                      ├── channel Disconnected / monitor 退出 / 订阅失败
  │                      ▼
  └──── backoff ──── Reconnecting
```

- **订阅失败**或 **event channel 断开**（`RecvTimeoutError::Disconnected`）后，**禁止**永久丢弃 `event_rx` 后空转。
- 有启用规则且总开关开启时，主线程按指数退避自动重订：
  - 序列：`1s → 2s → 4s → 8s → 16s → 30s`（封顶 30s）
  - 订阅成功后 `reconnect_attempt` 清零、`wmi_connected = true`、`last_error` 清空
- 无启用规则或总开关关闭 → 保持 Idle，**不**重试
- `Resubscribe`（规则/开关变更）时 backoff 清零并立即重订
- 用户**无需重启应用**即可恢复监听

### reconcile（仅订阅成功后，一次）

- 仅在（初始 / 重新 / 退避成功后）**订阅成功**时调用 `reconcile_running_processes`
- **非周期**：不会每 N 秒扫进程表
- 可纠正脏状态：
  - 若 `active_rule` 指向的进程已不在跑 → 先走 Stopped 语义（restore + 清空），再扫当前进程补 Started
  - 若 `active_rule` 对应规则已被禁用/删除 → 清空（关监听时按需 restore）

### `active_rule` 写入与清理

| 场景 | 行为 |
|------|------|
| `Started` / reconcile 命中 | **仅当** `load_config` + `apply_color_config` **都成功**时设置 `active_rule`；apply/load 失败不登记，避免退出时误 restore |
| `Stopped` | 清空前检查：若同名进程仍有实例在跑 → **忽略**本次 Stopped（多实例防护，事件触发的一次快照，非轮询） |
| 关闭总开关 / 无启用规则 | 停止 WMI；若有 `active_rule` 且 `restore_on_exit` → restore；清空 `active_rule`、`subscribed_processes`；`wmi_connected = false` |
| 禁用/删除当前激活规则 | 清空 `active_rule`，按需 restore |

### 设置变更与重订阅

| 路径 | 是否触发重订阅 |
|------|----------------|
| `add_process_rule` / `update_process_rule` / `delete_process_rule` | ✅ 自动 `send_resubscribe` |
| `set_process_watcher_enabled` | ✅ 自动 `send_resubscribe` |
| `save_app_settings` 变更 `process_rules` 或 `process_watcher_enabled` | ✅ **预期行为**（Task 5）：比较旧/新 settings，有变化则 `resubscribe`。修复 ConfigManager 等经整包 settings 写规则后订阅不更新的问题 |

### 诊断日志

统一 stderr 前缀 **`[process-watcher]`**，便于 `tauri dev` 终端过滤：

| 场景 | 日志示例 |
|------|----------|
| 订阅成功 | `WMI subscribed: names=[cs2.exe]` |
| 订阅失败 | `WMI subscribe failed: ...` |
| channel 断开 | `WMI event channel disconnected, will reconnect` |
| 退避重连 | `reconnect in {N}s` |
| Started / Stopped | `event Started name=cs2.exe` / `event Stopped name=cs2.exe` |
| reconcile | `reconcile Started name=cs2.exe` / `reconcile skip: no matching running process` |
| 多实例忽略 | `Stopped ignored, instance still running: cs2.exe` |
| apply/load 失败 | `apply_color_config failed: ...` / `load_config failed for '...': ...` |

### 期望体验矩阵

| 场景 | 期望 |
|------|------|
| 软件先开 → 再开游戏 | WMI Creation → 切方案（订阅保持存活） |
| 游戏先开 → 再开软件 | 启动 reconcile → 切方案 |
| WMI 线程/订阅意外断开 | 自动退避重订，无需用户重启 |
| 关总开关 / 删规则 | 清 `active_rule`，按需 restore |
| 同名进程双开，关一个 | 不 restore（仍有实例在跑） |
| 开发者看日志 | 能过滤 `[process-watcher]` 看到 subscribe / disconnect / reconcile / started / stopped |

---

**新增于**：2026-06-09  
**更新于**：2026-06-10 — 重构为 WMI 事件驱动 + 管理员权限 + 按规则定向订阅  
**更新于**：2026-06-10 — `list_running_processes` 改用 Unicode API（`Process32FirstW` / `PROCESSENTRY32W`）修复中文进程名乱码  
**更新于**：2026-07-11 — 新增订阅后对账（reconcile）：修复「先开游戏后开应用时，游戏退出不恢复默认方案」问题  
**更新于**：2026-07-14 — 明确 `restore_on_exit` 语义为恢复默认方案；移除未使用的 previous_config 分支  
**更新于**：2026-07-16 — WMI 可靠性：断线自愈（指数退避重订）、WatcherStatus 健康字段、active_rule 校正、多实例 Stopped 防护、`[process-watcher]` 日志前缀；明确无应用层周期轮询 / 无 ETW
