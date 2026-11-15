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

应用不含 VCGT 的 sRGB/其他 ICC 时，不再调用 WcsSetCalibrationManagementState 修改系统校准管理开关。2026-11-15 起，WCS 关联改为查询当前有效配置后按需更新。清空当前输出的 ICC 曲线基底后，通过现有颜色写门闩与合成 renderer 同步写入已有滑块参数，写入失败恢复之前的逻辑基底。无 VCGT 不等于整个方案无效；方案仍包含亮度、对比度、Gamma、RGB 与 DVC。

手动方案统一应用成功后，在调试会话中对标准后端安排约 250ms/1000ms 的只读回读，详见 [debug.md](./debug.md)。数字振动已处理后才建立快照；后续手动操作使旧回读任务停止。

## 方案切换短时重置修正（2026-11-15）

标准后端的方案应用先准备 ICC 关联和曲线基底，再设置目标 DVC，最后一次性写入方案最终 LUT。准备阶段不写旧滑块曲线、不把 DVC 中途复位至 50；无显式 ICC 时仍选择系统 sRGB，最终基底为线性。手动应用 ICC、恢复默认的直接写入语义继续保留。

每次应用从实际输出的 PnP 身份查询 WcsGetUsePerUserProfiles，再按当前有效作用域查询 WcsGetDefaultColorProfileSize/WcsGetDefaultColorProfile；返回缓冲大小按字节处理。当前有效配置与目标文件名相同（忽略大小写，支持完整路径）时跳过 InstallColorProfileW 及三个 WCS setters。配置不同则关联目标；查询失败仍尝试原有设置流程，所有设置失败写入调试日志和颜色状态 warnings，不影响后续曲线应用。使用实时查询，不依赖进程内缓存。

手动 apply_display_color_config 成功后，标准后端安排约 250ms、750ms 的两次短时回读，仅最大高字节偏差大于 2 时补写已经缓存的最终曲线，至多补写两次。读取失败停止，不重复 ICC、DVC、分辨率。独立于调试模式；每次读写持颜色门闩，要求操作代号未变化、同一稳定输出身份、监视器启用、身份无歧义、HDR 明确关闭、实际后端仍是 standard。读取之后再复核代号；任何新手动操作立即使旧任务失效。自动监听继续使用现有恢复调度。

调试日志分类 preset-settle 记录曲线保持或补写；verify-delayed 仍为独立只读诊断。日志捕获到的用户问题为最终曲线在约 250ms 内被重置，重复 WCS 关联和驱动重新配置是重点怀疑对象，实际重置者尚不能据此确定。

## ICC 默认记录查询兼容与闪动排查（2026-11-15，v3 测试）

01:36 的用户日志五次自定义方案均在约 250ms 检测到高字节偏差 42，补写后 750/1000ms 保持；默认方案偏差仅 1、无需补写。用户确认能持续应用但单向闪动，与自定义曲线被重置后补写的过程相吻合。每次 WCS 当前关联查询都报 ERROR_FILE_NOT_FOUND，v2 的重复关联跳过路径未在该环境生效。

修正设备默认 ICC 的查询/设置组合为 CPT_ICC + CPST_NONE（此前沿用的 CPST_PERCEPTUAL 属于 GMMP 渲染意图）。当 WCS 记录不存在或查询不可用时，从目标 GDI 输出创建新的 DC，使用 GetICMProfileW 读取当前输出配置，再比较是否需要注册。没有显式 WCS 默认记录不等于显示器没有有效 ICC；当前系统隐式 sRGB 与目标同名时也应跳过注册。若两种查询均失败，保留原有设置流程并记录两条失败原因；不猜测配置、不用进程缓存。

GetICMProfileW 使用实际字符缓冲，避免部分 Windows 上 NULL 大小探针返回 FALSE/122 的兼容问题；缓冲不足且 API 给出有效长度时最多扩容一次，上限 32768 个 UTF-16 字符。每次均释放 DC。设备目标来自本次应用的显示器，不回退到主屏。短时两次有限补写及所有代号/身份检查继续保留。

微软文档：[有效类型/子类型组合](https://learn.microsoft.com/en-us/windows/win32/api/icm/ne-icm-colorprofilesubtype)、[GetICMProfileW 当前输出配置](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-geticmprofilew)。本机只读探针确认：WCS 无显式默认记录时返回文件不存在，GDI 仍能读到系统 sRGB。真实笔记本的闪动是否消失仍待 v3 复测，尚不能据此确定最终重置者。

## 最终用户验证（2026-11-15，13:56 报告）

v3 用户确认方案应用与单向闪动已恢复正常。13 次方案点击，14 次跳过同一 ICC 关联，0 次关联更新；24 次保持检查未出现补写，21 次延迟回读及 162 次即时回读最大偏差均为 0，5 个过期诊断任务按代号取消，0 次颜色操作错误。WCS 缺少显式默认记录时，目标 GDI 输出查询得到系统 sRGB，按同名关联跳过重复设置。该报告覆盖标准后端问题复测，不据此认定操作系统/驱动的最终曲线写入者。
