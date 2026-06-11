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
                    let _ = apply_default_config();
                    let _ = app.emit("config-applied", "__default__");
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {
                    // 处理方案点击：id 格式为 "preset_<name>"
                    if let Some(name) = id.strip_prefix("preset_") {
                        let _ = apply_config_by_name(name);
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

/// 根据方案名应用配置
fn apply_config_by_name(name: &str) -> Result<(), String> {
    let cfg = config::load_config(name.to_string())?;
    apply_color_config(&cfg)
}

/// 恢复默认：应用已保存的默认配置；若无则恢复系统 sRGB + DVC 50%
pub(crate) fn apply_default_config() -> Result<(), String> {
    if let Some(cfg) = config::load_default_config()? {
        apply_color_config(&cfg)
    } else {
        icc::restore_default_icc(None)
    }
}

/// 应用 ColorConfig 的所有设置
pub fn apply_color_config(cfg: &ColorConfig) -> Result<(), String> {
    // 先处理 ICC（作为 gamma 基础 ramp），再叠加 NVIDIA 调节——与前端 handleApply 顺序一致。
    // ICC 失败（如引用的文件已删除）只跳过，不阻断后续 NVIDIA 应用。
    match &cfg.icc_profile {
        Some(icc_path) => {
            let _ = icc::apply_icc_profile(icc_path, None);
        }
        None => {
            // 无 ICC → 恢复系统默认 sRGB（清除残留 vcgt ramp），与前端一致
            let _ = icc::restore_default_icc(None);
        }
    }

    nvidia::set_nvidia_brightness(None, cfg.brightness)?;
    nvidia::set_nvidia_contrast(None, cfg.contrast)?;
    nvidia::set_nvidia_gamma(None, cfg.gamma)?;
    nvidia::set_nvidia_digital_vibrance(None, cfg.digital_vibrance)?;

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
