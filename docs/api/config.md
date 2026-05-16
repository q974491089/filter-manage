# 后端 API — 配置管理

源文件：`src-tauri/src/config.rs`

配置文件存储路径：`%APPDATA%\filter-manage\<name>.json`

## 数据类型

```ts
interface ColorConfig {
  name: string
  brightness: number
  contrast: number
  gamma: number
  digital_vibrance: number
  icc_profile: string | null   // ICC 文件名，可为 null
}
```

## 命令列表

### `save_config`
保存（或覆盖）一个颜色配置预设。

```ts
await invoke('save_config', {
  config: {
    name: '游戏模式',
    brightness: 10,
    contrast: 5,
    gamma: 1.2,
    digital_vibrance: 80,
    icc_profile: null
  }
})
```

---

### `load_config`
按名称加载配置预设。

```ts
const config: ColorConfig = await invoke('load_config', { name: '游戏模式' })
```

---

### `list_configs`
列出所有用户配置预设名称（不含内部默认配置）。

```ts
const names: string[] = await invoke('list_configs')
```

---

### `delete_config`

```ts
await invoke('delete_config', { name: '游戏模式' })
```

---

### `save_default_config`
保存启动默认值（仅首次调用生效，文件已存在时跳过）。

```ts
await invoke('save_default_config', { config })
```

---

### `load_default_config`
加载启动默认值，文件不存在时返回 `null`。

```ts
const config: ColorConfig | null = await invoke('load_default_config')
```

---

### `overwrite_default_config`
强制覆盖默认值（用户手动设置默认时调用）。

```ts
await invoke('overwrite_default_config', { config })
```
