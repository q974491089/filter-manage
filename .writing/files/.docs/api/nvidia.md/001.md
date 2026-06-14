# 后端 API — NVIDIA 颜色设置

源文件：`src-tauri/src/nvidia.rs`

## 数据类型

```ts
interface NvidiaSettings {
  brightness: number        // -125 ~ 125，默认 0
  contrast: number          // -82 ~ 82，默认 0
  gamma: number             // 0.4 ~ 2.8，默认 1.0
  digital_vibrance: number  // 0 ~ 100，默认 50
}
```

## 实现原理

- 亮度/对比度/伽马通过 `SetDeviceGammaRamp` 写入显卡 LUT（256 级查找表）
- 数字振动通过 `nvapi64.dll` 的 `NvAPI_SetDVCLevelEx` 设置（UI 0-100 映射到 NVAPI 范围）
- 调节时在当前 ICC 的 `vcgt` 基础 ramp 上叠加，不会覆盖 ICC 效果
- 所有命令支持 `deviceId` 参数指定目标显示器，不传时 fallback 到主显示器

## 命令列表

### `get_nvidia_settings`
获取指定显示器内存中的颜色设置值。

```ts
const settings: NvidiaSettings = await invoke('get_nvidia_settings', {
  deviceId: '\\\\.\\DISPLAY1'  // 可选，不传则返回主显示器
})
```

---

### `set_nvidia_brightness`

```ts
await invoke('set_nvidia_brightness', {
  deviceId: '\\\\.\\DISPLAY1',  // 可选
  value: 0       // -125 ~ 125
})
```

---

### `set_nvidia_contrast`

```ts
await invoke('set_nvidia_contrast', {
  deviceId: '\\\\.\\DISPLAY1',  // 可选
  value: 0       // -82 ~ 82
})
```

---

### `set_nvidia_gamma`

```ts
await invoke('set_nvidia_gamma', {
  deviceId: '\\\\.\\DISPLAY1',  // 可选
  value: 1.0     // 0.4 ~ 2.8
})
```

---

### `set_nvidia_digital_vibrance`

```ts
await invoke('set_nvidia_digital_vibrance', {
  deviceId: '\\\\.\\DISPLAY1',  // 可选
  value: 50      // 0 ~ 100
})
```

需要系统安装 NVIDIA 驱动（`nvapi64.dll`），非 NVIDIA 显卡调用会返回错误。

---

### `sync_dvc_from_driver`

从驱动读取指定显示器当前 DVC 实际值，同步到内存状态，返回 UI 值（0-100）。

```ts
const dvcValue: number = await invoke('sync_dvc_from_driver', {
  deviceId: '\\\\.\\DISPLAY1'  // 可选
})
```

---

### `get_dvc_default_ui_value`

获取指定显示器 DVC 驱动默认值对应的 UI 值（0-100）。

```ts
const defaultDvc: number = await invoke('get_dvc_default_ui_value', {
  deviceId: '\\\\.\\DISPLAY1'  // 可选
})
```

---

**更新于**：2026-05-21 — 数字振动(DVC)支持多显示器，`sync_dvc_from_driver` 和 `get_dvc_default_ui_value` 新增 `deviceId` 参数
