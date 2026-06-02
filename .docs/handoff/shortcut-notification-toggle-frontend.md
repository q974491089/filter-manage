# 交接文档：快捷键切换通知开关

## 后端已完成

- `AppSettings` 新增字段 `shortcut_notification: bool`，默认 `true`
- 旧配置文件无此字段时自动反序列化为 `true`（`#[serde(default = "default_true")]`）
- 快捷键触发时读取该字段，为 `false` 时跳过系统通知

## 前端需要实现

在设置弹窗的**快捷键页**（或通用设置页）加一个开关：

### 读取当前值

```ts
import { invoke } from '@tauri-apps/api/core'

const settings = await invoke<AppSettings>('get_app_settings')
// settings.shortcut_notification: boolean
```

### 保存修改

```ts
await invoke('save_app_settings', {
  settings: { ...settings, shortcut_notification: false }
})
```

### 开关组件示例

```tsx
<label className="flex items-center gap-2">
  <input
    type="checkbox"
    checked={settings.shortcut_notification}
    onChange={e =>
      saveSettings({ ...settings, shortcut_notification: e.target.checked })
    }
  />
  切换方案时显示系统通知
</label>
```

## 行为说明

- 开启（默认）：按快捷键切换方案时，右下角弹出 Toast 通知显示方案名
- 关闭：静默切换，无任何通知弹出
- 设置实时生效，无需重启
