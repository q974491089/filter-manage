# 前端交接：配置重命名 + 编辑功能

**后端完成日期**：2026-06-02  
**状态**：待前端实现

## 后端已完成

新增 `rename_config` Tauri 命令：

```ts
// 原子重命名配置预设
await invoke('rename_config', { oldName: '游戏模式', newName: '电竞模式' })
```

- `oldName === newName` → 直接成功（幂等）
- 旧名不存在 → `throw "Config '...' not found"`
- 新名已占用 → `throw "Config '...' already exists"`

**改参数值**：无需新命令，用现有流程：`load_config` → 修改字段 → `save_config`（覆盖）

## 前端需要实现

### 场景一：重命名

在配置列表的每一项上加"重命名"入口（右键菜单或操作按钮），触发内联编辑或弹窗：

```ts
async function handleRename(oldName: string, newName: string) {
  const trimmed = newName.trim()
  if (!trimmed || trimmed === oldName) return
  try {
    await invoke('rename_config', { oldName, newName: trimmed })
    await invoke('refresh_tray_menu')   // 托盘菜单同步更新
    // 刷新配置列表
  } catch (e) {
    // 展示错误提示，e 是字符串
  }
}
```

注意：重命名后需要同步刷新**托盘菜单**（`refresh_tray_menu`）和**快捷键绑定列表**（如果该配置有绑定的快捷键，快捷键绑定里的 `config_name` 字段不会自动更新，需前端提示用户重新绑定，或后续由后端扩展 rename_config 连带更新快捷键绑定）。

### 场景二：编辑参数值

```ts
async function handleEditConfig(name: string, patch: Partial<ColorConfig>) {
  const config = await invoke<ColorConfig>('load_config', { name })
  const updated = { ...config, ...patch }
  await invoke('save_config', { config: updated })
}
```

## 行为说明

- 配置名约束建议前端加：不允许空字符串、不允许包含 `/\:*?"<>|`（Windows 文件名非法字符）
- 重命名期间可展示 loading 状态（操作通常 <10ms，可省略）
