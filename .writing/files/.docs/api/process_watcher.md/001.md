# 后端 API — 进程监听

源文件：`src-tauri/src/process_watcher.rs`

## 概述

进程监听模块通过 WMI（Windows Management Instrumentation）事件订阅，实时监听规则中指定进程的启动和退出，自动切换配色方案。进程退出后可选恢复上一方案。应用以管理员权限运行。

## 数据类型

```ts
interface ProcessRule {
  id: string                    // UUID
  process_name: string          // 进程名（不区分大小写），如 "delta_force.exe"
  config_name: string           // 绑定的预设名
  enabled: boolean              // 是否启用
  restore_on_exit: boolean      // 进程退出时是否恢复上一方案（默认 true）
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
}
```

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
开关进程监听总控。关闭时取消 WMI 订阅（零开销），开启时重新订阅。

```ts
await invoke('set_process_watcher_enabled', { enabled: true })
```

---

### `get_watcher_status`
获取当前进程监听运行状态。

```ts
const status = await invoke<WatcherStatus>('get_watcher_status')
```

---

## 工作原理

1. 应用以 `requireAdministrator` 权限启动，UAC 提升后运行
2. `init_watcher()` 创建主 watcher 线程
3. 读取启用规则，生成 WQL 查询，通过 `windows` crate 原生 WMI API（`IWbemLocator` → `IWbemServices` → `ExecNotificationQueryAsync` + `IWbemObjectSink` 回调）创建事件订阅
4. WMI 监听线程接收 `__InstanceOperationEvent`，区分 creation/deletion 事件，转发到主 watcher 线程
5. 匹配规则（first-match-wins）→ `tray::apply_color_config` + emit `config-applied` + Toast
6. 进程退出 + `restore_on_exit: true` → 恢复上一方案或默认方案
7. 规则变化时自动取消旧订阅 + 创建新订阅（动态重订阅）
8. 无启用规则时不创建 WMI 订阅，零开销

**WQL 查询示例**（按规则定向订阅）：
```sql
SELECT * FROM __InstanceOperationEvent WITHIN 1
WHERE TargetInstance ISA 'Win32_Process'
AND (TargetInstance.Name = 'delta_force.exe' OR TargetInstance.Name = 'cs2.exe')
```

**新增于**：2026-06-09
**更新于**：2026-06-10 — 重构为 WMI 事件驱动 + 管理员权限 + 按规则定向订阅
**更新于**：2026-06-10 — `list_running_processes` 改用 Unicode API（`Process32FirstW` / `PROCESSENTRY32W`）修复中文进程名乱码
