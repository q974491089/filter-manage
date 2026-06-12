# 前端交接：进程监听自动切换方案

**后端完成日期**：2026-06-09
**状态**：待前端实现

## 后端已完成

新增 `process_watcher` 模块，提供 7 个 Tauri 命令。使用 WMI 事件订阅（非轮询）监听规则中指定的进程，进程启动/退出时自动切换配色方案。应用以管理员权限运行。

## 数据类型

```ts
interface ProcessRule {
  id: string                    // UUID，前端生成（crypto.randomUUID()）
  process_name: string          // 进程名，如 "delta_force.exe"（不区分大小写）
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
  active_rule: ProcessRule | null     // 当前激活的规则（无匹配时为 null）
  active_config_name: string | null   // 当前激活的方案名
  subscribed_processes: string[]      // 当前 WMI 订阅的进程名列表
}
```

## AppSettings 新增字段

```ts
interface AppSettings {
  // ... 现有字段
  process_watcher_enabled: boolean   // 进程监听总开关（默认 true）
  process_notification: boolean      // 自动切换时是否弹 Toast（默认 true）
  process_rules: ProcessRule[]       // 规则列表
}
```

## Tauri 命令

### `get_process_rules` — 获取所有规则

```ts
const rules = await invoke<ProcessRule[]>('get_process_rules')
```

### `add_process_rule` — 新增规则

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

- `process_name` 为空 → `throw "进程名不能为空"`
- `config_name` 不存在 → `throw "Config '...' not found"`
- 同一进程名重复 → `throw "进程 '...' 已存在规则"`（不区分大小写）

### `update_process_rule` — 更新规则

```ts
await invoke('update_process_rule', { rule: updatedRule })
```

### `delete_process_rule` — 删除规则

```ts
await invoke('delete_process_rule', { id: 'uuid-string' })
```

### `get_running_processes` — 枚举当前运行进程

```ts
const processes = await invoke<RunningProcess[]>('get_running_processes')
```

前端可用此命令展示运行中进程列表供用户选择，避免手动输入进程名。建议按 `name` 去重（多个 chrome.exe 只显示一个）。

### `set_process_watcher_enabled` — 开关总控

```ts
await invoke('set_process_watcher_enabled', { enabled: true })
```

### `get_watcher_status` — 获取当前状态

```ts
const status = await invoke<WatcherStatus>('get_watcher_status')
```

## 前端需要实现

### 1. 设置面板 — 进程监听区域

在 `SettingsModal` 中新增"进程监听"区域：

- 总开关（`process_watcher_enabled`）
- 通知开关（`process_notification`）
- 规则列表（可增删改、拖拽排序）
- 每条规则显示：进程名、绑定方案、启用开关、删除按钮

```tsx
// 示例：获取运行中进程供用户选择
async function showProcessPicker() {
  const processes = await invoke<RunningProcess[]>('get_running_processes')
  // 去重并按名称排序（icon 已由后端提取，同名进程共享同一图标）
  const unique = [...new Map(processes.map(p => [p.name.toLowerCase(), p])).values()]
    .sort((a, b) => a.name.localeCompare(b.name))
  // 展示选择列表，使用 icon 显示进程图标
  // <img src={process.icon || '/default-icon.png'} width={24} height={24} />
  // <span>{process.name}</span>
}
```

### 2. 规则 CRUD 交互

```tsx
// 新增规则
async function handleAddRule(processName: string, configName: string) {
  try {
    await invoke('add_process_rule', {
      rule: {
        id: crypto.randomUUID(),
        process_name: processName,
        config_name: configName,
        enabled: true,
        restore_on_exit: true,
      }
    })
    // 刷新规则列表
  } catch (e) {
    showToast('error', `添加失败: ${e}`)
  }
}

// 删除规则
async function handleDeleteRule(id: string) {
  await invoke('delete_process_rule', { id })
}

// 切换规则启用状态
async function handleToggleRule(rule: ProcessRule) {
  await invoke('update_process_rule', {
    rule: { ...rule, enabled: !rule.enabled }
  })
}
```

### 3. 监听 `config-applied` 事件

后端自动切换方案时会 emit `config-applied` 事件（与快捷键/托盘共用），前端已有此事件监听，收到后刷新 UI 状态即可。**无需新增事件类型。**

### 4. 状态栏指示（可选）

可在主界面显示一个小的状态指示器，当进程监听激活时显示当前匹配的规则名：

```tsx
const [watcherStatus, setWatcherStatus] = useState<WatcherStatus | null>(null)

useEffect(() => {
  const poll = async () => {
    const status = await invoke<WatcherStatus>('get_watcher_status')
    setWatcherStatus(status)
  }
  poll()
  const interval = setInterval(poll, 5000)
  return () => clearInterval(interval)
}, [])
```

## 行为说明

- 规则按列表顺序匹配，第一个命中的生效（first-match-wins）
- 进程退出后默认恢复"上一方案"（无记录时恢复默认方案），`restore_on_exit: false` 可关闭
- 使用 WMI 事件订阅，进程启动到方案生效最多延迟 1 秒
- 规则变化时自动重订阅（无需前端额外操作）
- 无启用规则时不创建 WMI 订阅，零开销
- 应用以管理员权限运行（ICC 设置需要），启动时弹 UAC 提示
- 规则中引用的方案被删除时，该规则自动跳过（不会报错崩溃）
- 进程名匹配不区分大小写（`DELTA_FORCE.EXE` == `delta_force.exe`）
- `get_running_processes` 仅在打开进程选择器时调用（一次性快照），关闭弹窗后不再调用
