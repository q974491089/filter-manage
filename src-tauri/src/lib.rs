mod config;
mod icc;
mod nvidia;
mod process_watcher;
mod shortcut;
mod tray;
mod updater;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_global_shortcut::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(updater::UpdaterState::default())
        .setup(|app| {
            let handle = app.handle().clone();

            // 迁移旧版多文件配置到 profiles.json（幂等，旧文件不存在时无操作）
            let _ = config::migrate_legacy_files();

            // 初始化系统托盘
            tray::init_tray(&handle).expect("Failed to init tray");

            // 初始化全局快捷键（忽略错误，首次启动可能没有绑定）
            let _ = shortcut::init_shortcuts(&handle);

            // 初始化进程监听（后台线程，自动匹配规则切换方案）
            process_watcher::init_watcher(&handle);

            // 监听窗口关闭事件
            if let Some(window) = app.get_webview_window("main") {
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        // 读取设置决定关闭行为
                        let settings = config::get_app_settings().unwrap_or_default();
                        if !settings.close_prompted {
                            // 用户尚未选择过关闭行为：阻止关闭并保持窗口可见，
                            // 由前端弹窗询问（缩小到托盘 / 直接退出），选择后写回设置
                            api.prevent_close();
                        }
                        // close_prompted == true 时：
                        // - close_to_tray == true → 不阻止，让前端调用 hide() 隐藏窗口
                        // - close_to_tray == false → 不阻止，正常退出
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // ICC
            icc::get_icc_profiles,
            icc::set_icc_profile,
            icc::get_current_icc_profile,
            icc::get_display_monitors,
            icc::restore_default_icc_profile,
            icc::import_icc_profile,
            icc::export_icc_profile,
            icc::search_icc_profiles,
            icc::set_preview_image,
            icc::open_icc_directory,
            icc::install_builtin_icc_profiles,
            // NVIDIA
            nvidia::set_nvidia_brightness,
            nvidia::set_nvidia_contrast,
            nvidia::set_nvidia_gamma,
            nvidia::set_nvidia_digital_vibrance,
            nvidia::get_nvidia_settings,
            nvidia::get_dvc_default_ui_value,
            nvidia::sync_dvc_from_driver,
            // Config
            config::save_config,
            config::load_config,
            config::list_configs,
            config::delete_config,
            config::rename_config,
            config::save_default_config,
            config::load_default_config,
            config::overwrite_default_config,
            config::get_app_settings,
            config::save_app_settings,
            // Tray
            tray::refresh_tray_menu,
            // Shortcuts
            shortcut::bind_shortcut,
            shortcut::unbind_shortcut,
            shortcut::list_shortcut_bindings,
            shortcut::pause_shortcuts,
            shortcut::resume_shortcuts,
            // Autostart
            enable_autostart,
            disable_autostart,
            is_autostart_enabled,
            // Process Watcher
            process_watcher::get_process_rules,
            process_watcher::add_process_rule,
            process_watcher::update_process_rule,
            process_watcher::delete_process_rule,
            process_watcher::get_running_processes,
            process_watcher::set_process_watcher_enabled,
            process_watcher::get_watcher_status,
            // Updater
            updater::check_update,
            updater::download_update,
            updater::cancel_update_download,
            updater::install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// === Autostart commands ===

#[tauri::command]
fn enable_autostart(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch()
        .enable()
        .map_err(|e| format!("Failed to enable autostart: {}", e))
}

#[tauri::command]
fn disable_autostart(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch()
        .disable()
        .map_err(|e| format!("Failed to disable autostart: {}", e))
}

#[tauri::command]
fn is_autostart_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch()
        .is_enabled()
        .map_err(|e| format!("Failed to check autostart: {}", e))
}
