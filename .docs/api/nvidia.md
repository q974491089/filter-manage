# 后端 API — NVIDIA 颜色设置

源文件：`src-tauri/src/nvidia.rs`

## 数据类型

```ts
interface NvidiaSettings {
  brightness: number        // -125 ~ 125，默认 0
  contrast: number          // -82 ~ 82，默认 0
  gamma: number             // 0.4 ~ 2.8，默认 1.0
  digital_vibrance: number  // 0 ~ 100，默认 50
  rgb_r: number             // -100 ~ 100，默认 0（无偏色）
  rgb_g: number             // -100 ~ 100，默认 0
  rgb_b: number             // -100 ~ 100，默认 0
}
```

## 实现原理

- 亮度/对比度/伽马通过 `SetDeviceGammaRamp` 写入显卡 LUT（256 级查找表）
- RGB 增益在 B/C/G + ICC 叠加之后，对 R/G/B 三通道分别乘 scale（内部 -100..+100 → 约 0.52..1.48）
- 数字振动通过 `nvapi64.dll` 的 `NvAPI_SetDVCLevelEx` 设置（UI 0-100 映射到 NVAPI 范围）
- DVC 的 display handle 用 `NvAPI_GetAssociatedNvidiaDisplayHandle` 按设备名（`\\.\DISPLAYn`）取。
  不能拿 `\\.\DISPLAYn` 的编号当 `NvAPI_EnumNvidiaDisplayHandle` 的索引 —— 后者只枚举已连接的
  NVIDIA 输出，两套编号无对应关系（实测混合输出笔记本上 Intel 占 `DISPLAY1-4`、NVIDIA 主屏在
  `DISPLAY5`，按编号取索引 4 会拿到 `END_ENUMERATION(-7)`，DVC 整条链失效）
- 明确指定了 `deviceId` 却取不到 handle 时**返回错误而非回退**：那台显示器不由 NVIDIA 输出，
  悄悄改到别的屏比失败更糟。只有不指定 `deviceId`（或老驱动缺 `GetAssociated` 入口）才回退第一个 NVIDIA 显示器
- UI 标度固定 0..100，读写两侧都经 `driver_to_ui_level` / `ui_to_driver_level` 与驱动标度互转
- **厂商分派**：数字振动的四个命令内部按"NVIDIA 优先，NVIDIA 明确不在时转 AMD ADLX"分派。
  "明确不在"= 没有 `nvapi64.dll` / `NvAPI_Initialize` 失败 / 该显示器不由 NVIDIA 输出，
  按显示器判断而非按机器。AMD 侧实现见 [amd.md](./amd.md)；命令名与签名不变
- 数字振动失败不阻断其余调节：托盘/进程监听应用方案、恢复默认 ICC 时它只记日志，
  保证 AMD / Intel 机器上亮度、对比度、伽马、RGB 增益、ICC 仍然照常生效
- 调节时在当前 ICC 的 `vcgt` 基础 ramp 上叠加，不会覆盖 ICC 效果
- 所有命令支持 `deviceId` 参数指定目标显示器，不传时 fallback 到主显示器
- 前端「调节方式」（NVIDIA / 卓伟 / AOC）仅换算 RGB 滑条显示标度，后端始终收内部 -100..+100

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

### `set_nvidia_rgb_gain`

设置 RGB 增益（偏色）。内部统一标度，与前端调节方式无关。

```ts
await invoke('set_nvidia_rgb_gain', {
  deviceId: '\\\\.\\DISPLAY1',  // 可选
  r: 0,   // -100 ~ 100，0 = 无偏色
  g: 0,
  b: 0,
})
```

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

### `get_dvc_capability`

探测指定显示器能否调数字振动。前端据此决定滑块是否可交互、应用方案时是否调用 DVC ——
非 NVIDIA 显卡（或非 NVIDIA 输出的显示器）上应当禁用而不是每次操作弹错误。

```ts
const cap = await invoke<{
  supported: boolean
  vendor: 'nvidia' | 'amd' | null   // 命中的后端；前端据此把标签换成"色彩饱和度"
  reason: string | null      // 不支持的原因，可直接展示
  driver_min: number         // 驱动实际标度，仅供诊断
  driver_max: number
  default_ui_value: number   // 驱动默认值换算到 UI 标度
}>('get_dvc_capability', {
  deviceId: '\\.\DISPLAY1'  // 可选
})
```

本命令不返回 `Result`，探测失败即 `supported: false`，不会 reject。

---

**更新于**：2026-07-24 — 新增 RGB 增益 `rgb_r/g/b` 与 `set_nvidia_rgb_gain`（内部 -100..+100）

**更新于**：2026-05-21 — 数字振动(DVC)支持多显示器，`sync_dvc_from_driver` 和 `get_dvc_default_ui_value` 新增 `deviceId` 参数

**更新于**：2026-08-29 — DVC 改用 `NvAPI_GetAssociatedNvidiaDisplayHandle` 按设备名取 handle（修复主屏非 `DISPLAY1` 时数字振动完全失效）；NVAPI 失败信息补上接口名、status 与显示器名

**更新于**：2026-09-04 — 新增 `get_dvc_capability` 能力探测；指定 `deviceId` 取不到 handle 时改为报错（不再静默回退到别的显示器）；写入路径补上 UI ↔ 驱动标度换算；DVC 失败不再阻断其余颜色调节

**更新于**：2026-09-13 — 数字振动命令加入 AMD ADLX 分派（`amd.rs`），`get_dvc_capability` 新增 `vendor` 字段
