> 📦 已归档 2026-11-15 · 仅供历史追溯，非当前开发依据

# 方案分辨率兼容处理 - 前端交接

## 后端已完成

### 新增命令：`apply_preset_resolution`

**参数**（Tauri 自动做 camelCase → snake_case 映射，前端照常传 camelCase）：

```rust
struct Params {
    device_id: Option<String>,   // deviceId：目标显示器（get_display_monitors 返回的 device_id）；null / 缺省 = 主显示器
    resolution: Option<String>,  // resolution：方案里的分辨率字段，"宽x高" 或 "宽x高@Hz"；null = 恢复本应用此前改过的分辨率
}
```

**返回值**：`Result<PresetResolutionResult, String>` — `Ok` 是带 `status` 判别字段的对象，五种状态的实际序列化样例：

| status | 实际 JSON | 含义 |
|---|---|---|
| `unchanged` | `{ "status": "unchanged" }` | 目标与当前模式一致（`@Hz` 缺省视为不关心），未做任何切换 |
| `applied` | `{ "status": "applied" }` | 切换成功；首次真正成功时已把切换前模式提交为还原点 |
| `restored` | `{ "status": "restored" }` | `resolution: null` 时成功还原了此前的还原点（分辨率 + 刷新率一起回去） |
| `skipped_unsupported` | `{ "status": "skipped_unsupported", "requested": "2560x1440", "reason": "Display mode 2560x1440 not supported" }` | 显示器不支持方案请求的模式；整次切换跳过，**不降级分辨率或刷新率**，当前模式与既有还原点未动 |
| `skipped_invalid` | `{ "status": "skipped_invalid", "requested": "abc" }` | 方案里的分辨率字段格式非法，已忽略本次分辨率调节 |

`requested` 是规范化描述：`"宽x高"` 或 `"宽x高@<Hz>Hz"`（如 `"2560x1440@280Hz"`）；`skipped_invalid` 里是原始字符串。

### `resolution: null` 的恢复语义

- 此前被本应用改过（还原点存在且属于该显示器）且当前模式 ≠ 还原点 → 自动还原分辨率与刷新率 → `{ status: "restored" }`。
- 无还原点，或当前模式已等于还原点 → `{ status: "unchanged" }`。
- 还原被驱动拒绝 → `invoke` 抛错（string），**还原点保留**，下一次再传 `null` 可重试。

### 错误行为：哪些是 Err，哪些是部分成功

**`Ok`（部分成功，不算失败）**：五个 status 全部是 Ok。`skipped_unsupported` / `skipped_invalid` 表示分辨率没动，但颜色参数应继续应用——前端不要把它伪装成全局失败，也不要伪装成完整成功。

**`Err`（invoke 抛出 string，新命令同样会抛的场景）**：

| 错误样例 | 场景 |
|---|---|
| `Monitor '\\.\DISPLAY9' not found` | `deviceId` 对不上任何活动显示器 |
| `No monitors found` | 无活动显示器 |
| `Cannot determine current display mode` | 无法读取当前模式 |
| `Display mode <W>x<H> not supported` / `Refresh rate <N> Hz not supported at <W>x<H>` | 仅旧命令 `apply_config_resolution` 返回（严格入口保留）；新命令同样场景返回 `skipped_unsupported` |
| `Display driver failed the specified graphics mode`、`ChangeDisplaySettingsExW failed: <code>` 等 | 驱动实际拒绝切换 / 系统级失败 |
| `Failed to load user32.dll: ...` | 枚举基础设施失败 |

### 相关文件

- 修改了：`src-tauri/src/icc.rs`（兼容判断、结构化结果、还原点事务、新命令）
- 修改了：`src-tauri/src/tray.rs`（后台路径改用兼容 helper，跳过时写 debug_recorder）
- 修改了：`src-tauri/src/lib.rs`（命令注册）
- 更新了：`.docs/api/icc.md`

**保留的旧命令**：`apply_config_resolution` 仍注册可用、语义严格（不支持的目标模式返回 Err）；手动分辨率卡片继续用 `set_display_mode`（裸切换、不登记还原点、严格报错），**不要**换成新命令。前端加载方案请从 `apply_config_resolution` 切到 `apply_preset_resolution`。

## 前端需要实现

### 1. 基本调用

在 `App.tsx::handleApply`（方案应用入口）中：

```typescript
import { invoke } from "@tauri-apps/api/core";

type PresetResolutionResult =
  | { status: "unchanged" }
  | { status: "applied" }
  | { status: "restored" }
  | { status: "skipped_unsupported"; requested: string; reason: string }
  | { status: "skipped_invalid"; requested: string };

// 顺序保持「先分辨率、再 ICC、再 gamma」
const resolutionResult = await invoke<PresetResolutionResult>("apply_preset_resolution", {
  deviceId: selectedDeviceId,   // 可为 null
  resolution: cfg.resolution,   // 方案字段，可为 null
});
```

### 2. 处理返回值

- `skipped_unsupported` → 暂存提示，待颜色参数全部成功后显示类似「颜色设置已应用；当前显示器不支持方案中的 {requested}，已保留当前分辨率」。使用方案区域的 `role="status"` 提示；颜色失败时显示实际错误并保留该说明。
- `skipped_invalid` → 一般不展示（配置数据损坏），可静默记录。
- `unchanged` / `applied` / `restored` → 无需特殊提示；`applied` / `restored` 后建议重拉 `get_display_modes` 刷新分辨率卡片，避免 UI 把未成功的目标模式当实际值或基线。
- 颜色参数（ICC / gamma / DVC）失败仍按现有方式向上报错，不因分辨率跳过而改变。

### 3. 错误处理

```typescript
try {
  const result = await invoke<PresetResolutionResult>("apply_preset_resolution", params);
  // 五种 status 都已"处理完毕"，继续后面的 ICC/gamma 流程
} catch (error) {
  // error 是 string：真实失败（设备不存在 / 枚举失败 / 驱动拒绝）。
  // 旧路径行为保持：分辨率失败不阻断颜色应用，错误留痕。
}
```

## 完整示例

```typescript
const handleApply = async (cfg: ColorConfig) => {
  let skipNote: string | null = null;
  try {
    const r = await invoke<PresetResolutionResult>("apply_preset_resolution", {
      deviceId: deviceId ?? null,
      resolution: cfg.resolution ?? null,
    });
    if (r.status === "skipped_unsupported") {
      skipNote = `当前显示器不支持方案中的 ${r.requested}，已保留当前分辨率`;
    }
    // ... 继续 ICC → gamma → DVC（顺序与现状一致）
  } catch (error) {
    // 分辨率真实失败：按现状继续颜色应用，最后整体报错
  }
};
```

## 测试场景

1. **正常流程**：1080p 屏应用 2560×1440 方案 → 返回 `{status:"skipped_unsupported", requested:"2560x1440"}`，模式保持不变，颜色生效，出现部分成功提示；重复点击不积累错误状态。
2. **边界情况**：方案带 `@280Hz` 但显示器只有 240Hz → `skipped_unsupported`（`requested` 含 `@280Hz`，不降级）；方案分辨率 = 当前模式 → `unchanged`；`resolution: null` 且此前被本应用切走过 → `restored`（分辨率和刷新率一起回去）。
3. **错误场景**：`deviceId` 乱填 → Err `Monitor '...' not found`；还原点存在但驱动拒绝还原 → Err，再次传 `null` 可重试成功。

## 注意事项

- 还原点是进程级单例，前端命令/托盘/快捷键/进程监听共用，前端不需要也无法管理其生命周期。
- 后端托盘路径对 `skipped_unsupported` 已写 debug_recorder（含目标显示器、请求模式、当前模式、原因），调试会话可直接核对。
- 新入口不改变方案 JSON 结构：`ColorConfig.resolution` 仍是 `Option<String>`，保存范围开关是前端 Task 3 的事。

## API 文档

详见 `.docs/api/icc.md#apply_preset_resolution`

[已完成] 2026-11-06


## v0.6.2 发布接入确认（2026-11-15）

本交接对应的实现已接入并随 v0.6.2 发布；配套 API/market 与管理后台预发及生产部署通过，客户端正式构建、签名、R2 和双域名更新检查通过。完整发布证据见 [v0.6.2 发布记录](../../releases/v0.6.2-release-plan.md)。
