mod config;
mod icc;
mod nvidia;
mod process_watcher;
mod shortcut;
mod tray;
mod updater;
mod announcements;
mod amd;
mod admin;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_global_shortcut::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![admin::SILENT_ARG]),
        ))
        .manage(updater::UpdaterState::default())
        .setup(|app| {
            let handle = app.handle().clone();
            let settings = config::get_app_settings().unwrap_or_default();

            // 需要管理员但当前未提权 → 以管理员重启（成功则本进程退出）。
            // 用户取消 UAC 时不循环、不退出，本次以非管理员继续运行。
            if settings.run_as_admin && !admin::is_elevated() {
                if let Err(e) = admin::relaunch_elevated() {
                    eprintln!("[admin] 提权重启已跳过: {e}");
                }
            }

            // 刚提权/首次开启管理员自启：确保计划任务存在，并互斥关掉注册表自启。
            // 条件短路保证非管理员用户启动时不会调用 schtasks。
            if settings.run_as_admin
                && settings.autostart
                && admin::is_elevated()
                && !admin::scheduled_task_exists()
            {
                let _ = admin::reconcile_autostart(&handle, true, true);
            }

            // 迁移旧版多文件配置到 profiles.json（幂等，旧文件不存在时无操作）
            let _ = config::migrate_legacy_files();

            // 初始化系统托盘
            tray::init_tray(&handle).expect("Failed to init tray");

            // 初始化全局快捷键（忽略错误，首次启动可能没有绑定）
            let _ = shortcut::init_shortcuts(&handle);

            // 初始化进程监听（后台线程，自动匹配规则切换方案）
            process_watcher::init_watcher(&handle);

            // 启动来源：带 --silent（开机自启）→ 保持隐藏仅进托盘；否则显示主窗口。
            // 主窗口在 tauri.conf.json 中初始 visible:false，避免非静默启动时的窗口闪现。
            let silent = std::env::args().any(|a| a == admin::SILENT_ARG);

            // 监听窗口关闭事件
            if let Some(window) = app.get_webview_window("main") {
                if !silent {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        // ⚠ 实际的关闭行为由前端 onCloseRequested 决定，不在这里。
                        // Tauri 只要检测到前端注册了 close-requested 监听器，就会自己
                        // 无条件 prevent_close（见 tauri 的 manager/window.rs），再把事件
                        // 抛给前端；前端不 preventDefault 时由 JS 包装层调 destroy()。
                        //
                        // 这里只保留一层兜底：前端尚未加载完（监听器还没注册）时，
                        // 若用户没选过关闭行为就先别让窗口关掉，否则他永远看不到询问弹窗。
                        let settings = config::get_app_settings().unwrap_or_default();
                        if !settings.close_prompted {
                            api.prevent_close();
                        }
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
            nvidia::set_nvidia_rgb_gain,
            nvidia::get_nvidia_settings,
            nvidia::get_dvc_default_ui_value,
            nvidia::get_dvc_capability,
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
            // Admin (run as administrator)
            admin::is_running_as_admin,
            admin::set_run_as_admin,
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
            // Announcements
            announcements::get_announcements,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                process_watcher::stop_watcher();
                amd::shutdown();
            }
        });
}

// === Autostart commands ===
// 自启机制由 run_as_admin 决定：非管理员用注册表 Run 项，管理员用计划任务。
// 两者互斥切换在 admin::reconcile_autostart 中处理。

#[tauri::command]
fn enable_autostart(app: tauri::AppHandle) -> Result<(), String> {
    let run_as_admin = config::get_app_settings()
        .map(|s| s.run_as_admin)
        .unwrap_or(false);
    admin::reconcile_autostart(&app, true, run_as_admin)
}

#[tauri::command]
fn disable_autostart(app: tauri::AppHandle) -> Result<(), String> {
    let run_as_admin = config::get_app_settings()
        .map(|s| s.run_as_admin)
        .unwrap_or(false);
    admin::reconcile_autostart(&app, false, run_as_admin)
}

#[tauri::command]
fn is_autostart_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    let via_registry = app.autolaunch().is_enabled().unwrap_or(false);
    Ok(via_registry || admin::scheduled_task_exists())
}
