# 多显示器 NVIDIA 调节 — 前端交接

## 后端已完成的改动

- `set_nvidia_brightness`、`set_nvidia_contrast`、`set_nvidia_gamma`、`set_nvidia_digital_vibrance`、`get_nvidia_settings` 参数从 `display: i32` 改为 `deviceId: Option<String>`
- 不传 `deviceId` 时自动使用主显示器
- 每个显示器独立维护亮度/对比度/伽马/数字振动状态
- gamma ramp 写入指定显示器（不再影响其他屏幕）

## 前端需要实现

### 1. 添加显示器选择器

在 ColorAdjuster 上方或内部添加一个 select，数据源：

```tsx
import { invoke } from "@tauri-apps/api/core";

interface DisplayMonitor {
  name: string;       // 如 "显示器 1 VG27AQ"
  device_id: string;  // 如 "\\\\.\\DISPLAY1" — 这是传给后端的值
  pnp_id: string;
  is_primary: boolean;
}

const monitors = await invoke<DisplayMonitor[]>("get_display_monitors");
```

### 2. 修改调用参数

所有 NVIDIA 调节命令的参数从 `{ display: 1, value }` 改为 `{ deviceId, value }`：

```tsx
// 选中的显示器 device_id
const [selectedDeviceId, setSelectedDeviceId] = useState<string | undefined>();

// 调用示例
await invoke("set_nvidia_brightness", { deviceId: selectedDeviceId, value });
await invoke("set_nvidia_contrast", { deviceId: selectedDeviceId, value });
await invoke("set_nvidia_gamma", { deviceId: selectedDeviceId, value });
await invoke("set_nvidia_digital_vibrance", { deviceId: selectedDeviceId, value });
```

### 3. 切换显示器时加载该显示器的当前状态

```tsx
const loadSettingsForDisplay = async (deviceId: string) => {
  const settings = await invoke<NvidiaSettings>("get_nvidia_settings", { deviceId });
  setBrightness(settings.brightness);
  setContrast(settings.contrast);
  setGamma(settings.gamma);
  setDigitalVibrance(settings.digital_vibrance);
};
```

### 4. 涉及的文件

- `src/components/ColorAdjuster.tsx` — 添加 select + 改调用参数
- `src/App.tsx` — 加载配置时也需要传 `deviceId`（第149-151行）

## 注意事项

- `deviceId` 为 `undefined`/不传时后端自动用主显示器，所以默认不选也能工作
- `get_display_monitors` 已有，无需新增依赖
- 数字振动（DVC）目前 NVAPI 只支持全局设置，传 `deviceId` 会存储但实际 DVC 是全局生效的
