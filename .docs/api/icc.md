# ICC 与显示器颜色后端

## 颜色后端命令

- `get_display_color_status({ deviceId?: string })`：只读查询目标显示器能力、HDR、驱动、活动后端与失败原因，返回 `DisplayColorStatus`。
- `set_display_color_backend({ deviceId: string, backend: "standard" | "nvidia" })`：在颜色写门闩内切换，返回更新后的 `DisplayColorStatus`。
- `apply_display_color_config({ deviceId?: string, config: ColorConfig })`：统一应用分辨率和颜色，返回 `{ status, resolution, warnings }`；不支持的分辨率保留当前模式，颜色继续应用。
- `open_display_device_manager()`：打开 Windows 设备管理器，供用户检查监视器设备状态。

状态字段：`device_id`、`stable_key`、`monitor_state`、`gpu_vendor`、`hdr_enabled`、`preferred_backend`、`active_backend`、`nvidia_support`、`reason`、`warnings`。

标准模式保持 WCS/GDI；NVIDIA 模式读取同一 ICC 的 VCGT 并经驱动私有输出曲线写入。偏好只保存在本机输出身份下，不进入方案或分享数据。驱动接口成功仅表示写入成功，不能证明屏幕实际效果。实验路径要求当前环境的新鲜远程 `nvidia_color_backend` 开关授权，缓存和过期响应不能授权；失败关闭路径恢复标准后端。

## ICC 校验与失败回滚

读取 ICC 时校验文件头、声明长度与标签边界，区分缺少 VCGT 和损坏 VCGT。解析限制在标签声明范围内。标准写入失败还原本机曲线基底；NVIDIA 切换、默认恢复与启动清理以实际输出身份及完整曲线所有权为条件。

## 显示器枚举

`enumerate_display_monitors` 在活动适配器下继续枚举监视器子设备，选择带 `DISPLAY_DEVICE_ACTIVE` 标记的 PnP 身份；旧的未启用子设备不会被当作当前屏幕。没有活动身份则留空，不回退到历史屏幕。

## 自动监听颜色恢复

游戏启动、显示模式变化与切回前台触发有限恢复，保持每次激活的总写入预算及冷却；检查窗口过期后保留所有权供事件重新打开窗口。手动调整使代号失效，旧任务不能复活。NVIDIA 模式按驱动曲线处理，避免用 GDI 回读误判；标准模式继续回读并在偏差超过容差时恢复。

**更新于**：2026-11-14 — 无登录分支提取现有颜色后端、监听后续修复与 AMD 活动显示器识别修复。
## 无 VCGT 的系统 ICC（2026-11-14）

应用不含 VCGT 的 sRGB/其他 ICC 时，仍执行原有 WCS 关联，但不再调用 WcsSetCalibrationManagementState 修改系统校准管理开关。清空当前输出的 ICC 曲线基底后，通过现有颜色写门闩与合成 renderer 同步写入已有滑块参数，写入失败恢复之前的逻辑基底。无 VCGT 不等于整个方案无效；方案仍包含亮度、对比度、Gamma、RGB 与 DVC。

手动方案统一应用成功后，在调试会话中对标准后端安排约 250ms/1000ms 的只读回读，详见 [debug.md](./debug.md)。数字振动已处理后才建立快照；后续手动操作使旧回读任务停止。
