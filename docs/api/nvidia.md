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
- 数字振动通过 `nvapi64.dll` 的 `NvAPI_SetDVCLevel` 设置（UI 0-100 映射到 NVAPI 0-63）
- 调节时在当前 ICC 的 `vcgt` 基础 ramp 上叠加，不会覆盖 ICC 效果

## 命令列表

### `get_nvidia_settings`
获取当前内存中的颜色设置值（不读取硬件，返回上次设置的值）。

```ts
const settings: NvidiaSettings = await invoke('get_nvidia_settings')
```

---

### `set_nvidia_brightness`

```ts
await invoke('set_nvidia_brightness', {
  display: 1,    // 保留参数，传 1 即可
  value: 0       // -125 ~ 125
})
```

---

### `set_nvidia_contrast`

```ts
await invoke('set_nvidia_contrast', {
  display: 1,
  value: 0       // -82 ~ 82
})
```

---

### `set_nvidia_gamma`

```ts
await invoke('set_nvidia_gamma', {
  display: 1,
  value: 1.0     // 0.4 ~ 2.8
})
```

---

### `set_nvidia_digital_vibrance`

```ts
await invoke('set_nvidia_digital_vibrance', {
  display: 1,
  value: 50      // 0 ~ 100
})
```

需要系统安装 NVIDIA 驱动（`nvapi64.dll`），非 NVIDIA 显卡调用会返回错误。
