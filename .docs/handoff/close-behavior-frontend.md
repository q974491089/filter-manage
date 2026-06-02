# 前端交接：关闭行为改为下拉框

**后端完成日期**：2026-06-02  
**状态**：待前端实现

## 后端变更

`AppSettings.close_to_tray` 类型从 `boolean` 改为 `boolean | null`：

```ts
interface AppSettings {
  close_to_tray: boolean | null  // null=未选择，true=最小化到托盘，false=直接关闭
  close_prompted: boolean        // 不变
  // ...其余字段不变
}
```

旧文件兼容：旧 `app.json` 里的 `true/false` 自动读为对应值，无影响。

## 前端需要实现

### 1. 设置面板：Toggle → 下拉框

把"关闭时最小化到托盘"的 Toggle 换成下拉框：

```tsx
<select
  value={settings.close_to_tray === null ? '' : String(settings.close_to_tray)}
  onChange={(e) => {
    const val = e.target.value === '' ? null : e.target.value === 'true'
    updateSettings({ close_to_tray: val })
  }}
>
  <option value="">未设置</option>
  <option value="true">最小化到托盘</option>
  <option value="false">直接关闭</option>
</select>
```

### 2. 关闭行为逻辑（ClosePromptModal 或 App.tsx）

```
close_to_tray === null  → 弹窗询问用户
close_to_tray === true  → 直接隐藏窗口（hide），不弹窗
close_to_tray === false → 直接退出，不弹窗
```

### 3. 弹窗"记住选择"联动

用户在弹窗勾选"记住"后，把选择写回设置：

```ts
// 用户选择了"最小化到托盘"并勾了记住
await invoke('save_app_settings', {
  settings: { ...currentSettings, close_to_tray: true, close_prompted: true }
})

// 用户选择了"直接关闭"并勾了记住
await invoke('save_app_settings', {
  settings: { ...currentSettings, close_to_tray: false, close_prompted: true }
})

// 用户选择了但没勾记住（保持 null）
await invoke('save_app_settings', {
  settings: { ...currentSettings, close_to_tray: null, close_prompted: false }
})
```

## 行为对照表

| close_to_tray | 点击关闭按钮 | 设置面板下拉框显示 |
|---------------|-------------|-----------------|
| `null` | 弹窗询问 | 未设置 |
| `true` | 直接最小化到托盘 | 最小化到托盘 |
| `false` | 直接关闭 | 直接关闭 |
