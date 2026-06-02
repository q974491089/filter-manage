# DVC 多显示器 — 前端适配

## 后端已完成

`sync_dvc_from_driver` 和 `get_dvc_default_ui_value` 新增 `deviceId: Option<String>` 参数。不传时 fallback 到主显示器（向后兼容，不会报错）。

## 前端建议修改（可选）

为了切换显示器时读取对应显示器的 DVC 值，建议传递 `selectedDeviceId`：

```tsx
// App.tsx 初始化时
const driverDvc = await invoke<number>("sync_dvc_from_driver", { deviceId: selectedDeviceId });

// 恢复默认时
const dvcDefault = await invoke<number>("get_dvc_default_ui_value", { deviceId: selectedDeviceId });
```

## 行为说明

- 不传 `deviceId` 时行为与之前完全一致（读主显示器）
- 传 `deviceId` 后会读取对应显示器的实际 DVC 驱动值
