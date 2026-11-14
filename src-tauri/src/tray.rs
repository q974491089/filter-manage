use tauri::{
    menu::{Menu, MenuItem, Submenu},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

use crate::config::{self, ColorConfig};
use crate::icc;
use crate::nvidia;

const MAX_TRAY_PRESETS: usize = 5;

/// 初始化系统托盘
pub fn init_tray(app: &AppHandle) -> Result<(), String> {
    let menu = build_tray_menu(app)?;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Filter Manage")
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();
            match id {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "restore_default" => {
                    let _ = apply_default_config_from("tray");
                    let _ = app.emit("config-applied", "__default__");
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {
                    // 处理方案点击：id 格式为 "preset_<name>"
                    if let Some(name) = id.strip_prefix("preset_") {
                        let _ = apply_config_by_name_from("tray", name);
                        let _ = app.emit("config-applied", name);
                    }
                }
            }
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => {
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            _ => {}
        })
        .build(app)
        .map_err(|e| format!("Failed to build tray: {}", e))?;

    Ok(())
}

/// 构建托盘菜单
fn build_tray_menu(app: &AppHandle) -> Result<Menu<tauri::Wry>, String> {
    let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let restore = MenuItem::with_id(app, "restore_default", "恢复默认", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let quit =
        MenuItem::with_id(app, "quit", "退出", true, None::<&str>).map_err(|e| e.to_string())?;

    // 获取方案列表，无方案时不显示快速方案子菜单
    let preset_names = get_tray_preset_names()?;

    let menu = if preset_names.is_empty() {
        Menu::with_items(app, &[&show, &restore, &quit]).map_err(|e| e.to_string())?
    } else {
        let submenu =
            Submenu::with_id(app, "presets", "快速方案", true).map_err(|e| e.to_string())?;
        for name in &preset_names {
            let item_id = format!("preset_{}", name);
            let item = MenuItem::with_id(app, item_id, name, true, None::<&str>)
                .map_err(|e| e.to_string())?;
            submenu.append(&item).map_err(|e| e.to_string())?;
        }
        Menu::with_items(app, &[&show, &submenu, &restore, &quit]).map_err(|e| e.to_string())?
    };

    Ok(menu)
}

/// 获取托盘应展示的方案名列表
fn get_tray_preset_names() -> Result<Vec<String>, String> {
    let settings = config::get_app_settings()?;
    let all_configs = config::list_configs()?;
    let all_names: Vec<String> = all_configs.iter().map(|c| c.name.clone()).collect();

    if settings.tray_presets.is_empty() {
        // 默认展示前5个
        Ok(all_names.into_iter().take(MAX_TRAY_PRESETS).collect())
    } else {
        // 只展示用户选择的（且仍然存在的）
        Ok(settings
            .tray_presets
            .into_iter()
            .filter(|name| all_names.contains(name))
            .take(MAX_TRAY_PRESETS)
            .collect())
    }
}

/// 根据方案名应用配置（外部写入来源 `source`：tray / shortcut / process-watcher）
fn apply_config_by_name_from(source: &str, name: &str) -> Result<(), String> {
    let cfg = config::load_config(name.to_string())?;
    apply_color_config_from(source, &cfg)
}

/// 恢复默认：应用已保存的默认配置；若无则恢复系统 sRGB + DVC 50%。
/// 外部写入入口：持颜色写门闩并递增代号（旧恢复所有权随之失效）。
pub(crate) fn apply_default_config_from(source: &str) -> Result<(), String> {
    let _latch = nvidia::begin_external_write();
    crate::debug_recorder::log("config", format!("恢复默认方案（来源={source}）"));
    if let Some(cfg) = config::load_default_config()? {
        apply_color_config_locked_with_source(source, &cfg).map(|_| ())
    } else {
        let device = nvidia::resolve_display_id(None);
        let result = if crate::color_backend::active_backend(&device) == crate::color_backend::Backend::Nvidia {
            crate::color_backend::restore_default_locked(&device)
        } else {
            icc::restore_default_icc(Some(device.clone())).and_then(|()| {
                nvidia::replace_settings(&device, nvidia::NvidiaSettings::default());
                nvidia::apply_gamma_ramp_locked(&device)
            })
        };
        // 无默认方案时也要还原本应用此前改过的分辨率（如进程监听切走的 4:3）
        if let Err(e) = icc::apply_preset_resolution_state(None, None) {
            eprintln!("[apply] resolution restore on default failed: {e}");
        }
        result
    }
}

/// 应用 ColorConfig 的所有设置。外部写入入口：持颜色写门闩并递增代号，
/// 整组应用期间保持持闩（内部全部走已持闩 helper，不得重取公开命令入口）。
/// `source` 用于诊断日志区分托盘 / 快捷键 / 进程监听来源。
pub fn apply_color_config_from(source: &str, cfg: &ColorConfig) -> Result<(), String> {
    let _latch = nvidia::begin_external_write();
    apply_color_config_locked_with_source(source, cfg).map(|_| ())
}

/// watcher 专用：应用成功后，在同一颜色写门闩内捕获本次恢复所有权。
/// Err 表示初次 apply 失败；Ok(None) 表示 apply 成功但最终 LUT 或该设备的 PnP
/// 身份不可用。返回后发生的手动写会递增 generation，使该快照失效；调用方
/// 不得另行采样 generation/LUT，也不得持 watcher 状态锁调用本入口。
pub(crate) fn apply_color_config_with_recovery(
    rule_id: &str,
    cfg: &ColorConfig,
) -> Result<Option<crate::color_recovery::RecoveryOwnership>, String> {
    let ownership = nvidia::with_external_write_snapshot(
        || apply_color_config_locked_with_source("process-watcher", cfg),
        |device_id, generation| {
            let expected_lut = nvidia::final_ramp_for_display(device_id)?;
            let monitors = icc::get_display_monitors().ok()?;
            if crate::color_backend::active_backend(device_id) == crate::color_backend::Backend::Nvidia {
                let cap = crate::display_capability::inspect(device_id).ok()?;
                let key = cap.stable_key?;
                Some(crate::color_recovery::RecoveryOwnership {
                    rule_id: rule_id.into(), activation_seq: crate::color_recovery::next_activation_seq(),
                    device_id: device_id.into(), pnp_id: key, generation, expected_lut,
                    backend: crate::color_backend::Backend::Nvidia,
                })
            } else {
                recovery_ownership_for_display(rule_id, device_id, generation, expected_lut, &monitors)
            }
        },
    )?;
    if ownership.is_none() {
        let message = format!(
            "color recovery not armed: rule={rule_id}, final LUT or applied display identity unavailable"
        );
        eprintln!("[process-watcher] {message}");
        crate::debug_recorder::log("process-watcher", message);
    }
    Ok(ownership)
}

// 只匹配 apply 实际返回的设备；主屏变化或枚举顺序变化时不回退到另一块屏幕。
fn recovery_ownership_for_display(
    rule_id: &str,
    device_id: &str,
    generation: u64,
    expected_lut: [u16; 768],
    monitors: &[icc::DisplayMonitor],
) -> Option<crate::color_recovery::RecoveryOwnership> {
    let monitor = monitors
        .iter()
        .find(|m| m.device_id == device_id && !m.pnp_id.is_empty())?;
    Some(crate::color_recovery::RecoveryOwnership {
        rule_id: rule_id.to_string(),
        activation_seq: crate::color_recovery::next_activation_seq(),
        device_id: monitor.device_id.clone(),
        pnp_id: monitor.pnp_id.clone(),
        generation,
        expected_lut,
        backend: crate::color_backend::Backend::Standard,
    })
}

/// 已持闩的整组方案应用；返回所有 NVIDIA/LUT helper 实际写入的 device_id，
/// 供 watcher 在闩内捕获，避免事后重新 resolve_display_id(None) 选错屏幕。
fn apply_color_config_locked_with_source(
    source: &str,
    cfg: &ColorConfig,
) -> Result<String, String> {
    // 代号由外部入口递增；本函数不得重新获取颜色写门闩。

    // 实际目标显示器：全部 None 参数最终都落到主显示器，写进日志供诊断
    let did = nvidia::resolve_display_id(None);
    crate::debug_recorder::log(
        "config",
        format!(
            "应用方案 '{}' 来源={source} 目标={did} icc={:?} 亮度={} 对比度={} 伽马={} DVC={} RGB=({},{},{}) resolution={:?}（后台路径：托盘/快捷键/进程监听/自动恢复）",
            cfg.name, cfg.icc_profile, cfg.brightness, cfg.contrast, cfg.gamma,
            cfg.digital_vibrance, cfg.rgb_r, cfg.rgb_g, cfg.rgb_b, cfg.resolution
        ),
    );

    // 分辨率先行：模式切换可能重置 gamma/ICC ramp，先落分辨率再上色。
    // 兼容策略：显示器不支持的目标模式跳过切换并留痕（保留当前模式与既有
    // 还原点），颜色参数继续应用——这不算整体失败；真实错误（设备不存在/
    // 枚举失败/驱动拒绝）同样只留痕不阻断（与 DVC 同策略）。后续 gamma 等
    // 失败仍向上返回 Err，不伪装为完整成功。
    match icc::apply_preset_resolution_state(None, cfg.resolution.as_deref()) {
        Ok(icc::PresetResolutionResult::SkippedUnsupported { requested, reason }) => {
            let monitors = icc::enumerate_display_monitors();
            let target_desc = monitors
                .iter()
                .find(|m| m.is_primary)
                .or_else(|| monitors.first())
                .map(|m| format!("{} ({})", m.name, m.device_id))
                .unwrap_or_else(|| "未知".to_string());
            let current_desc = icc::get_display_modes(None)
                .ok()
                .and_then(|m| m.current)
                .map(|c| format!("{}x{}@{}Hz", c.width, c.height, c.refresh_rate))
                .unwrap_or_else(|| "未知".to_string());
            crate::debug_recorder::log(
                "config",
                format!(
                    "方案 '{}' 分辨率跳过：目标显示器 '{}'，请求 '{}'，当前 '{}'，原因：{}（颜色参数继续应用）",
                    cfg.name, target_desc, requested, current_desc, reason
                ),
            );
            eprintln!(
                "[apply] resolution skipped (unsupported): target={target_desc} requested={requested} current={current_desc} reason={reason}"
            );
        }
        Ok(_) => {}
        Err(e) => {
            crate::debug_recorder::log(
                "config",
                format!("方案 '{}' 的分辨率应用失败：{e}", cfg.name),
            );
            eprintln!("[apply] resolution skipped: {}", e);
        }
    }

    // A/B 共用一次最终曲线合成；后台和前端整组应用使用同一个协调器。
    for warning in crate::color_backend::apply_config_locked(&did, cfg)? {
        crate::debug_recorder::log("config", format!("方案 '{}' 部分成功：{warning}", cfg.name));
        eprintln!("[apply] {warning}");
    }

    Ok(did)
}

/// Tauri 命令：刷新托盘菜单（方案变化时前端调用）
#[tauri::command]
pub fn refresh_tray_menu(app: AppHandle) -> Result<(), String> {
    // Tauri 2 中需要重建菜单
    let menu = build_tray_menu(&app)?;
    // 获取托盘并设置新菜单
    if let Some(tray) = app.tray_by_id("main") {
        tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod recovery_snapshot_tests {
    use super::recovery_ownership_for_display;
    use crate::icc::DisplayMonitor;

    fn monitor(device_id: &str, pnp_id: &str, is_primary: bool) -> DisplayMonitor {
        DisplayMonitor {
            name: device_id.into(),
            device_id: device_id.into(),
            pnp_id: pnp_id.into(),
            is_primary,
        }
    }

    #[test]
    fn ownership_keeps_applied_device_when_primary_or_enumeration_order_changes() {
        let monitors = [
            monitor("new-primary", "new-primary-pnp", true),
            monitor("applied-device", "applied-device-pnp", false),
        ];
        let lut = [32123; 768];
        let owner =
            recovery_ownership_for_display("rule", "applied-device", 17, lut, &monitors).unwrap();
        assert_eq!(owner.rule_id, "rule");
        assert_eq!(owner.device_id, "applied-device");
        assert_eq!(owner.pnp_id, "applied-device-pnp");
        assert_eq!(owner.generation, 17);
        assert_eq!(owner.expected_lut, lut);
        let next =
            recovery_ownership_for_display("rule", "applied-device", 17, lut, &monitors).unwrap();
        assert!(next.activation_seq > owner.activation_seq);
    }

    #[test]
    fn missing_applied_device_or_pnp_never_falls_back_to_another_monitor() {
        let other = [monitor("other-primary", "other-pnp", true)];
        assert!(recovery_ownership_for_display("rule", "missing", 17, [1; 768], &other).is_none());
        let unidentified = [monitor("applied-device", "", true)];
        assert!(recovery_ownership_for_display(
            "rule",
            "applied-device",
            17,
            [1; 768],
            &unidentified,
        )
        .is_none());
    }
}
