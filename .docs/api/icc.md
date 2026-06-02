# 后端 API — ICC 配置文件管理

源文件：`src-tauri/src/icc.rs`

所有命令通过 `invoke` 调用：
```ts
import { invoke } from '@tauri-apps/api/core'
```

## 数据类型

```ts
interface IccProfile {
  name: string       // 文件名，如 "sRGB IEC61966-2.1.icc"
  path: string       // 完整路径
  is_active: boolean
}

interface DisplayMonitor {
  name: string       // 显示名称，如 "显示器 1 ASUS VG27A"
  device_id: string  // 如 "\\.\DISPLAY1"
  pnp_id: string     // PnP 设备路径
  is_primary: boolean
}
```

---

## 命令列表

### `get_icc_profiles`
获取系统 ICC 目录中所有 `.icc` / `.icm` 文件。

```ts
const profiles: IccProfile[] = await invoke('get_icc_profiles')
```

---

### `search_icc_profiles`
按名称关键字过滤 ICC 文件（大小写不敏感）。

```ts
const profiles: IccProfile[] = await invoke('search_icc_profiles', {
  query: 'sRGB'   // 空字符串返回全部
})
```

**新增于**：2026-05-16

---

### `import_icc_profile`
将外部 ICC 文件复制到系统 ICC 目录（`C:\Windows\System32\spool\drivers\color\`）。

```ts
const fileName: string = await invoke('import_icc_profile', {
  srcPath: 'C:\\Users\\xxx\\Downloads\\my-profile.icc'
})
```

- 仅接受 `.icc` / `.icm` 扩展名，否则返回错误
- 返回导入后的文件名

**新增于**：2026-05-16

---

### `export_icc_profile`
将系统 ICC 目录中的文件导出到指定目录。

```ts
const destPath: string = await invoke('export_icc_profile', {
  profileName: 'my-profile.icc',  // 仅文件名，不含路径
  destDir: 'C:\\Users\\xxx\\Desktop'
})
// 返回导出后的完整路径
```

**新增于**：2026-05-16

---

### `set_icc_profile`
应用 ICC 配置文件（立即生效）。同时解析 `vcgt` 标签写入显卡 LUT，并更新 NVIDIA 叠加基础。

```ts
await invoke('set_icc_profile', {
  profilePath: 'C:\\Windows\\System32\\spool\\drivers\\color\\my-profile.icc',
  deviceId: null   // null = 主显示器；或传 "\\.\DISPLAY1"
})
```

---

### `get_current_icc_profile`
获取当前激活的 ICC 文件名（从注册表读取）。

```ts
const name: string = await invoke('get_current_icc_profile')
```

---

### `restore_default_icc_profile`
恢复默认 sRGB 配置文件。

```ts
await invoke('restore_default_icc_profile', { deviceId: null })
```

---

### `get_display_monitors`
枚举当前连接的显示器列表。

```ts
const monitors: DisplayMonitor[] = await invoke('get_display_monitors')
```

---

### `open_icc_directory`
用 Windows 资源管理器打开系统 ICC 目录（`C:\Windows\System32\spool\drivers\color`）。

```ts
await invoke('open_icc_directory')
```

**新增于**：2026-05-16

---

### `set_preview_image`
验证本地图片路径（文件存在 + 格式合法），返回路径供前端显示。

```ts
const validPath: string = await invoke('set_preview_image', {
  imagePath: 'C:\\Users\\xxx\\Pictures\\test.jpg'
})
```

支持格式：`jpg / jpeg / png / webp / bmp / gif`

显示图片需用 `convertFileSrc()` 转换：
```ts
import { convertFileSrc } from '@tauri-apps/api/core'
const src = convertFileSrc(validPath)  // asset:// 协议
// <img src={src} />
```

**新增于**：2026-05-16

---

## 前端接入要点

### 点击导入 / 拖拽导入

```ts
import { open } from '@tauri-apps/plugin-dialog'

// 点击导入
const selected = await open({ filters: [{ name: 'ICC', extensions: ['icc', 'icm'] }] })
if (selected) await invoke('import_icc_profile', { srcPath: selected as string })

// 拖拽导入（HTML5 drop）
onDrop={async (e) => {
  e.preventDefault()
  for (const file of Array.from(e.dataTransfer.files)) {
    await invoke('import_icc_profile', { srcPath: (file as any).path })
  }
}}

// 拖拽导入（Tauri 窗口级）
import { listen } from '@tauri-apps/api/event'
await listen('tauri://drag-drop', (event: any) => {
  for (const p of event.payload.paths as string[]) {
    if (p.endsWith('.icc') || p.endsWith('.icm')) {
      invoke('import_icc_profile', { srcPath: p })
    }
  }
})
```

### 导出

```ts
const dir = await open({ directory: true, title: '选择导出目录' })
if (dir) await invoke('export_icc_profile', { profileName: name, destDir: dir as string })
```

### 错误处理

所有命令失败时 `invoke` 抛出 `string` 类型错误：
```ts
try {
  await invoke('import_icc_profile', { srcPath: path })
} catch (err) {
  // err 是 string
}
```
