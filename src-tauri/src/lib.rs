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

/// 退出前的进程内清理：停进程监听线程、还原 AMD 状态、释放托盘/窗口资源。
///
/// `RunEvent::Exit` 会走到这里；安装更新后主动 `process::exit` 的那条路径
/// 也必须显式调用，否则这些清理全被跳过。
pub(crate) fn shutdown_runtime(app: &tauri::AppHandle) {
    process_watcher::stop_watcher();
    amd::shutdown();
    app.cleanup_before_exit();
}

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

            // requireAdministrator 清单保证 release 进程必已提权。自启一律用计划任务，
            // 这里按设置覆盖注册/删除（重复注册可自愈启动参数与 exe 路径变更，含旧版迁移）。
            // dev 构建不自动注册，否则计划任务会指向 target 下的调试 exe。
            if !cfg!(debug_assertions) && admin::is_elevated() {
                let _ = admin::reconcile_autostart(
                    &handle,
                    settings.autostart,
                    settings.autostart_silent,
                );
            }
            // 旧版注册表自启项残留：提权清单下开机必弹 UAC，无条件清理
            {
                use tauri_plugin_autostart::ManagerExt;
                let _ = handle.autolaunch().disable();
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
                // 原生标题栏标注管理员状态（任务栏/Alt+Tab 同步可见，参考 v2rayN 的「以管理员身份运行」后缀）
                if admin::is_elevated() {
                    let base = window.title().unwrap_or_else(|_| "Filter Manage".to_string());
                    let _ = window.set_title(&format!("{base} - 以管理员身份运行"));
                }
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
            // Admin
            admin::is_running_as_admin,
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
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                shutdown_runtime(app);
            }
        });
}

// === Autostart commands ===
// 自启一律使用计划任务（requireAdministrator 下注册表 Run 项会开机弹 UAC），
// 启动参数（静默托盘/显示窗口）由设置的 autostart_silent 决定；
// 旧版注册表项的清理在 admin::reconcile_autostart 中处理。

#[tauri::command]
fn enable_autostart(app: tauri::AppHandle) -> Result<(), String> {
    let silent = config::get_app_settings()
        .map(|s| s.autostart_silent)
        .unwrap_or(true);
    admin::reconcile_autostart(&app, true, silent)
}

#[tauri::command]
fn disable_autostart(app: tauri::AppHandle) -> Result<(), String> {
    admin::reconcile_autostart(&app, false, true)
}

#[tauri::command]
fn is_autostart_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    let via_registry = app.autolaunch().is_enabled().unwrap_or(false);
    Ok(via_registry || admin::scheduled_task_exists())
}
