> v0.6.2 发布状态（2026-11-15）：代码与前端契约已上线，交接已归档；原计划的全部真机验收矩阵尚未逐项覆盖，保留为后续验收依据，不将未执行的检查标为完成。

# 方案分辨率兼容处理 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
> 执行时以本仓库 `.rules/subagent-dispatch.md` 为准；当前只编写计划，不修改应用代码。技能不可用时使用现有工具完成等价的分步执行与验证。

**Goal:** 加载带有不兼容分辨率的方案时保留当前显示模式、继续应用颜色，并让用户明确选择是否将分辨率保存到方案。

**Architecture:** 保留手动切换显示模式的严格校验；新增方案专用的兼容应用入口，返回结构化的部分成功结果。沿用 `ColorConfig.resolution: Option<String>`，不增加服务端字段，不改写已有方案。

**Tech Stack:** React 18 / TypeScript、Tauri 2、Rust、Win32 显示模式 API。

---

## 证据与边界

- 用户报告版本 v0.6.1；视频当前模式为 1920×1080@240Hz。
- 13:27:17.946 和 13:27:26.914，两次应用“三角洲降曝光”均收到 `Display mode 2560x1440 not supported`。
- `icc.rs::set_display_mode` 在驱动枚举结果中找不到可用的目标模式时返回该错误；不是分辨率列表加载失败。不能由此推断屏幕在所有连接方式下都不支持该模式。
- 错误后 ICC、gamma 和 DVC 仍执行，不能把整个方案描述为应用失败。
- `App.tsx::buildColorFields` 当前无条件保存 `currentResolution`；无法由报告确认该方案来自分享、市场还是本机创建。
- 当前 `apply_resolution_state` 在模式切换前登记还原点，即使切换失败也可能留下还原点；本次一起修正该状态事务。
- `resolution: null` 保持既有“没有目标模式，必要时恢复本应用之前的还原点”语义。

## 文件与职责

| 文件 | 改动 |
|---|---|
| `src-tauri/src/icc.rs` | 方案专用结果类型、兼容性判断、还原点事务、纯逻辑回归测试 |
| `src-tauri/src/lib.rs` | 注册新增 `apply_preset_resolution` 命令 |
| `src-tauri/src/tray.rs` | 后台方案使用相同兼容策略，记录跳过原因 |
| `src/App.tsx` | 应用结果反馈、保存范围开关、脏状态与基线 |
| `src/components/DisplayResolution.tsx` | 展示保存范围开关；手动切换仍使用严格入口 |
| `.docs/api/icc.md`、`.docs/api/config.md` | 同步新增入口、保存语义 |
| `.docs/README.md`、`SYNC_STATUS.md` | 实施完成后记录与同步信号 |
| `.docs/archive/handoff/preset-resolution-frontend.md` | 后端完成后写入实际契约与验证结果 |

跨端实施采用 Tier 1：后端 Agent 只修改 `src-tauri/` 与接口文档，前端 Agent 只修改 `src/`；先冻结契约再接入。主 Agent 负责集成验证。若执行时无子 Agent 工具，按 Tier 2 顺序执行并保留交接文档。

## Task 1：定义并验证兼容策略（P0）

- [ ] 在 `icc.rs` 新增以下纯判断函数与回归测试；先运行测试确认缺少实现时失败，再实现。

```rust
fn preset_mode_supported(
    list: &DisplayModeList,
    width: u32,
    height: u32,
    refresh_rate: Option<u32>,
) -> bool {
    let matches = |w, h, hz| {
        w == width && h == height && refresh_rate.is_none_or(|target| target == hz)
    };
    list.current.as_ref().is_some_and(|c| matches(c.width, c.height, c.refresh_rate))
        || list.modes.iter().any(|m| {
            m.width == width && m.height == height
                && m.refresh_rates.iter().any(|hz| *hz > 0 && matches(m.width, m.height, *hz))
        })
}

#[cfg(test)]
mod preset_resolution_tests {
    use super::*;

    #[test]
    fn unsupported_resolution_or_explicit_refresh_is_rejected() {
        let list = DisplayModeList {
            modes: vec![DisplayModeInfo {
                width: 1920, height: 1080,
                refresh_rates: vec![240, 144, 60], is_current: true,
            }],
            current: Some(CurrentDisplayMode {
                width: 1920, height: 1080, refresh_rate: 240,
            }),
        };
        assert!(!preset_mode_supported(&list, 2560, 1440, None));
        assert!(!preset_mode_supported(&list, 1920, 1080, Some(280)));
        assert!(preset_mode_supported(&list, 1920, 1080, Some(144)));
        assert!(preset_mode_supported(&list, 1920, 1080, None));
    }

    #[test]
    fn current_mode_is_valid_even_if_missing_from_enumeration() {
        let list = DisplayModeList {
            modes: vec![],
            current: Some(CurrentDisplayMode {
                width: 1920, height: 1080, refresh_rate: 240,
            }),
        };
        assert!(preset_mode_supported(&list, 1920, 1080, Some(240)));
        assert!(!preset_mode_supported(&list, 1920, 1080, Some(144)));
    }
}
```

- [ ] 运行 `cargo test --manifest-path src-tauri/Cargo.toml --lib preset_resolution_tests`；预期纯测试通过、不调用模式切换 API。
- [ ] 方案明确指定刷新率但当前设备不支持时跳过整次模式切换，保留当前模式；不擅自选择其他分辨率或更低刷新率。无 `@Hz` 时沿用现有择优策略。

## Task 2：接入结构化结果和还原点事务（P0）

- [ ] 在 `icc.rs` 定义并序列化以下契约，命令参数为 `device_id: Option<String>, resolution: Option<String>`，返回 `Result<PresetResolutionResult, String>`。

```rust
#[derive(serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PresetResolutionResult {
    Unchanged,
    Applied,
    Restored,
    SkippedUnsupported { requested: String, reason: String },
    SkippedInvalid { requested: String },
}
```

- [ ] 新命令 `apply_preset_resolution` 与后台共享 `apply_preset_resolution_state`。按顺序执行：解析目标 → 读取当前模式及可用模式 → 判断相同/不支持 → 必要时调用既有严格切换函数 → 提交还原点状态。格式错误使用 `SkippedInvalid`；设备不存在、枚举失败、驱动实际拒绝切换仍返回真实错误。
- [ ] 首次真正切换成功后才提交原模式为还原点；失败时恢复原有还原点状态。曾成功切换后再应用不支持的方案，不清除既有还原点。恢复失败时保留还原点以便重试。
- [ ] 原 `apply_config_resolution` / `set_display_mode` 继续服务手动控制并严格报错。两种入口使用同一份还原点事务实现，避免复制两套状态逻辑。
- [ ] `tray.rs::apply_color_config` 使用新兼容 helper；`SkippedUnsupported` 写入目标显示器、请求模式、当前模式与原因，继续 ICC / gamma / DVC。后续 gamma 失败仍向上返回失败，不伪装为完整成功。
- [ ] 增加还原点事务测试，使用注入的假切换函数而非真实屏幕：首次失败不建点、既有还原点失败不丢失、成功后能恢复、恢复失败仍可重试。运行 `cargo test --manifest-path src-tauri/Cargo.toml --lib resolution`。
- [ ] 注册命令并更新 `.docs/api/icc.md`；写交接文档，再进行前端接入。

## Task 3：前端反馈与保存范围（P0 / P1）

- [ ] P0：`handleApply` 改用 `apply_preset_resolution`，包含 `resolution: null` 的恢复路径。保留“先分辨率、再 ICC、再 gamma”的顺序。
- [ ] 对 `SkippedUnsupported` 暂存提示，待颜色参数全部成功后显示“颜色设置已应用；当前显示器不支持方案中的 2560×1440，已保留当前分辨率”。使用方案区域的 `role="status"` 提示，不把兼容跳过伪装成全局成功或全局失败。颜色失败时显示实际错误，并保留分辨率跳过说明。
- [ ] 分辨率卡片最后重拉真实模式，避免 UI 将未成功的目标模式当作实际值或基线。
- [ ] P1：分辨率卡片增加“将分辨率和刷新率保存到方案”开关，受既有分辨率功能开关约束；新方案默认关闭，加载已有含 `resolution` 的方案时开启。关闭时说明“不指定目标模式；必要时恢复本应用此前改过的分辨率”。
- [ ] 功能入口被远程开关隐藏时，更新已有方案仍保留原有 `resolution`，不因为不可见控件的默认值清空它；新建方案仍默认不携带分辨率。已有方案只有在用户可见地关闭保存范围后才清除目标模式。
- [ ] `buildColorFields` 的分辨率值改为以下表达式；关闭开关不自动改动现有 JSON，只有用户保存/更新时写入。

```typescript
resolution: includeResolution ? currentResolution : null,
```

- [ ] 保存范围单独进入 baseline；开关变化参与脏判断。仅在 `includeResolution` 开启时，将实际模式变化计为方案颜色编辑以外的分辨率变更。应用/切屏/后台同步均显式更新保存范围，避免只改 UI、不改基线。
- [ ] 新增控件遵循 `DESIGN.md` 与项目组件属性规范。前端不新装测试框架；通过 TypeScript 编译和下方手动矩阵验证。

## Task 4：交付验证与文档

- [ ] Windows 执行 `cargo check --manifest-path src-tauri/Cargo.toml`，预期无 error、无新增 warning。
- [ ] 执行 `pnpm exec tsc --noEmit`、`pnpm run build`，预期成功。
- [ ] 在 1080p 屏应用 1440p 方案：模式保持不变，颜色生效，出现部分成功提示；重复点击不积累错误状态。
- [ ] 验证当前支持分辨率但不支持方案刷新率、合法 4:3 模式切换、切回无分辨率方案恢复原模式及刷新率。
- [ ] 分别走手动卡片、方案点击、托盘、快捷键、进程监听；兼容策略一致，手动请求不支持模式仍报错。
- [ ] 新建纯颜色方案 JSON 为 `resolution: null`；旧含分辨率方案加载/分享/导入保留其字段，用户明确关闭后更新才清除。
- [ ] 更新 `.docs/api/config.md`、`.docs/README.md`、`SYNC_STATUS.md`；只暂存本任务文件。建议拆分后端兼容修复与保存范围 UI 两个提交，不调整版本号、不自动发布。

**验收：** 原用户的 2560×1440 方案在当前 1080p 模式下能应用颜色并得到准确提示；有效分辨率方案继续可用；失败切换不污染还原点。
