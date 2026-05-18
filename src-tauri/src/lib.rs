mod config;
mod icc;
mod nvidia;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
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
            nvidia::set_nvidia_brightness,
            nvidia::set_nvidia_contrast,
            nvidia::set_nvidia_gamma,
            nvidia::set_nvidia_digital_vibrance,
            nvidia::get_nvidia_settings,
            nvidia::get_dvc_default_ui_value,
            nvidia::sync_dvc_from_driver,
            config::save_config,
            config::load_config,
            config::list_configs,
            config::delete_config,
            config::save_default_config,
            config::load_default_config,
            config::overwrite_default_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
