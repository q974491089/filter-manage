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
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)
        .map_err(|e| e.to_string())?;

    // 获取方案列表，无方案时不显示快速方案子菜单
    let preset_names = get_tray_preset_names()?;

    let menu = if preset_names.is_empty() {
        Menu::with_items(app, &[&show, &restore, &quit])
            .map_err(|e| e.to_string())?
    } else {
        let submenu = Submenu::with_id(app, "presets", "快速方案", true)
            .map_err(|e| e.to_string())?;
        for name in &preset_names {
            let item_id = format!("preset_{}", name);
            let item = MenuItem::with_id(app, item_id, name, true, None::<&str>)
                .map_err(|e| e.to_string())?;
            submenu.append(&item).map_err(|e| e.to_string())?;
        }
        Menu::with_items(app, &[&show, &submenu, &restore, &quit])
            .map_err(|e| e.to_string())?
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
        apply_color_config_locked_with_source(source, &cfg)
    } else {
        let result = icc::restore_default_icc(None);
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
    apply_color_config_locked_with_source(source, cfg)
}

/// 已持闩的整组方案应用（apply_color_config_from / apply_default_config_from 在
/// 闩内复用；递增代号已由两个入口的门闩完成）
fn apply_color_config_locked_with_source(source: &str, cfg: &ColorConfig) -> Result<(), String> {
    // 门闩已在 apply_color_config_from / apply_default_config_from 入口递增代号

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
            crate::debug_recorder::log("config", format!("方案 '{}' 的分辨率应用失败：{e}", cfg.name));
            eprintln!("[apply] resolution skipped: {}", e);
        }
    }

    // 先处理 ICC（作为 gamma 基础 ramp），再叠加 NVIDIA 调节——与前端 handleApply 顺序一致。
    // ICC 失败（如引用的文件已删除）只跳过，不阻断后续 NVIDIA 应用。
    match &cfg.icc_profile {
        Some(icc_path) => {
            if let Err(e) = icc::apply_icc_profile(icc_path, None) {
                // 只跳过不阻断，但必须留痕——否则「方案应用了但 ICC 没生效」无从排查
                crate::debug_recorder::log("config", format!("方案 '{}' 的 ICC 应用失败：{e}", cfg.name));
            }
        }
        None => {
            // 无 ICC → 恢复系统默认 sRGB（清除残留 vcgt ramp），与前端一致
            let _ = icc::restore_default_icc(None);
        }
    }

    // 已持闩：走 *_locked helper，不得再经公开命令入口重取门闩（不可重入）
    nvidia::set_brightness_locked(&did, cfg.brightness)?;
    nvidia::set_contrast_locked(&did, cfg.contrast)?;
    nvidia::set_gamma_locked(&did, cfg.gamma)?;
    // 数字振动只有 NVIDIA 输出的显示器支持，失败不阻断其余调节 ——
    // AMD / Intel 机器上它必然失败，但 RGB 增益等仍然应该照常应用。
    if let Err(e) = nvidia::dvc_set_locked(&did, cfg.digital_vibrance.clamp(0, 100)) {
        eprintln!("[apply] digital vibrance skipped: {}", e);
    }
    nvidia::set_rgb_gain_locked(&did, cfg.rgb_r, cfg.rgb_g, cfg.rgb_b)?;

    Ok(())
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
