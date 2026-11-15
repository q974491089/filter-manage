> 📦 已归档 2026-11-15 · 仅供历史追溯，非当前开发依据

# 颜色恢复所有权快照 — 后端交接

**日期**：2026-11-07
**状态**：主 Agent 已接入 Started、fallback、rebind 与 pending-restore 四条规则应用路径；旧事后快照捕获已移除。
**范围**：仅 `src-tauri/src/nvidia.rs`、`src-tauri/src/tray.rs`、`.docs/api/nvidia.md` 与本文件；不提交。

## 新契约

```rust
pub(crate) fn tray::apply_color_config_with_recovery(
    rule_id: &str,
    cfg: &ColorConfig,
) -> Result<Option<crate::color_recovery::RecoveryOwnership>, String>
```

- `Ok(Some(owner))`：初次应用成功，在同一个 `begin_external_write` guard 内复制本次 generation、最终成功写入的 LUT、实际 device_id 与匹配的 PnP 身份，并分配 activation_seq。
- `Ok(None)`：初次应用成功，但没有可用 LUT/实际目标身份；不启动恢复任务，不当作方案应用失败。
- `Err(e)`：应用失败；不捕获、不借用历史 LUT 建立此次所有权。
- 目标 device_id 是 `apply_color_config_locked_with_source` 实际解析并传给 NVIDIA/LUT helper 的设备。身份匹配不得重新选主屏或回退到其它显示器；设备缺失/PnP 为空返回 None。
- 所有权只绑定本次提交；返回后其它外部写入会先递增 generation，即使 LUT 尚未写完，该 owner 也已无效。仍需由 tick / `recovery_write_ramp` 复核代号及 PnP 等当前所有权条件。
- 不得持 watcher 状态锁调用此入口。capture 只读显示器列表/最终 LUT、分配原子 activation_seq；不取状态锁、不重入写门闩。

## 主 Agent 接入

1. 将首次激活、fallback、rebind、pending-restore 的规则方案应用统一改成新入口；将返回 owner 原样传入 ActiveRecovery。
2. 删除 `capture_recovery_ownership` 及 apply 返回后再读 `apply_generation` / `final_ramp_for_display` 的组合；**不要**覆盖返回 owner 的 generation。
3. `Ok(None)` 仍按应用成功更新 applied/status，但 recovery 为 None；`Err` 延续原有失败/重试语义。
4. 默认方案/Keep 分支维持无规则恢复所有权；非 watcher 入口签名与行为不变。
5. tick/window 修复由主 Agent 负责。本补丁未改 `process_watcher.rs`、`color_recovery.rs`、`SYNC_STATUS.md` 或 `.docs/README.md`。

## 实现边界

- `nvidia::with_external_write_snapshot` 与局部 mutex/假写测试共用事务 helper，确保快照构造先于 guard 释放。
- 已持闩 tray 应用 helper 从 `Result<(), String>` 改为内部 `Result<String, String>`；原有外部入口映射回 `()`，不改变公开 API。
- 不修改 ICC 应用/失败兼容策略、颜色算法、DVC 策略或显示模式逻辑。
- 写门闩不阻止 OS/驱动更改显示拓扑；主 Agent 的后续身份检查仍必需。

## 验证

- RED 已复现：将事务 helper 保持旧的“apply → drop guard → capture”顺序，确定性测试失败于 `owner.generation` 得到 2（手动写）而应为 1（本次方案）。
- GREEN：`cargo test --manifest-path src-tauri/Cargo.toml --lib recovery_snapshot_tests -- --test-threads=1`，5 项通过，全部仅使用局部 mutex/假 LUT/合成显示器列表，无真实屏幕写入或桌面修改。
- 覆盖：手动写在快照边界被阻塞；手动写先加代号、未更新 LUT 时旧 owner 已失效；失败不捕获历史 LUT；成功无快照为 None；主屏变化/枚举顺序变化仍绑定实际设备；实际设备或 PnP 缺失不回退。
- `cargo check --manifest-path src-tauri/Cargo.toml --lib` 已通过编译；尚未接入时检查有 4 条、测试构建有 2 条 dead_code 警告，均来自未接线的新调用链，未使用 allow 屏蔽。主 Agent 接入后需复跑检查确认无新增警告。
- 指定 Rust 文件的 `git diff --check` 通过。当前 `.gitignore` 第 83 行实际忽略整个 `.docs/`；两份文档已落盘，但普通 git status 不会显示。未改忽略规则、未暂存、未提交。

## 主 Agent 集成复验（2026-11-07）

- 新入口已接通，`cargo test --lib` 40 项全部通过；`cargo check --all-targets` 无编译警告，接线前 dead_code 警告已消失。
- 同一状态锁中先校验恢复所有权再取任务，真实 I/O 均在锁外；120 秒到期只休眠，事件再开窗保留累计预算/冷却。
- TypeScript 检查、前端构建、7 组导入语义回归断言、4 个本次 Rust 文件格式检查与 `git diff --check` 通过。
- 额外严格 Clippy 的 12 项报警代码已比对 HEAD，均为既有 lint（包括 rustfmt 拆行的原有代码），不属本轮新增；未屏蔽或顺手重构。真实 Intel 屏/游戏验收尚未执行，未提交、未发布。


## v0.6.2 发布接入确认（2026-11-15）

本交接对应的实现已接入并随 v0.6.2 发布；配套 API/market 与管理后台预发及生产部署通过，客户端正式构建、签名、R2 和双域名更新检查通过。完整发布证据见 [v0.6.2 发布记录](../../releases/v0.6.2-release-plan.md)。
