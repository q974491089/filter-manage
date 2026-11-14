# AMD 饱和度修复

饱和度继续通过现有 ADLX 路径按当前显示器匹配，UI 中性值与驱动中性值仍使用既有换算。

本轮修复活动输出下的 PnP 枚举：Windows 可能先返回未启用的旧监视器，再返回当前监视器；固定使用第 0 项会把型号传给 ADLX 并误报 `saturation not supported on this display`。现在选择 `DISPLAY_DEVICE_ACTIVE` 子设备，不存在时返回空身份而不回退到旧设备。

回归场景包括旧屏在前、当前屏在前、只有旧屏和空列表。`live_primary_amd_saturation_probe` 是默认忽略的真机只读测试，读取活动主屏的 ADLX 饱和度能力，不写颜色。

**更新于**：2026-11-14 — 已迁移无登录修复分支，真机探针与最终构建由整合验证统一执行。