use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::config::{self, ShortcutBinding};
use crate::tray;

/// 发送 Windows Toast 通知，每次弹出且不堆叠
#[cfg(windows)]
fn show_toast(body: &str) {
    use std::sync::atomic::{AtomicU32, Ordering};
    use windows::{
        Data::Xml::Dom::XmlDocument,
        UI::Notifications::{ToastNotification, ToastNotificationManager},
        core::HSTRING,
    };

    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let idx = COUNTER.fetch_add(1, Ordering::Relaxed);
    let new_tag = format!("fm-switch-{}", idx);
    let app_id = HSTRING::from("com.filter-manage.app");

    // 移除上一条通知（避免堆叠）
    if idx > 0 {
        let old_tag = HSTRING::from(format!("fm-switch-{}", idx - 1));
        if let Ok(history) = ToastNotificationManager::History() {
            let _ = history.Remove(&old_tag);
        }
    }

    let xml = XmlDocument::new().unwrap();
    xml.LoadXml(&HSTRING::from(format!(
        r#"<toast><visual><binding template="ToastGeneric"><text>Filter Manage</text><text>{}</text></binding></visual></toast>"#,
        body
    ))).unwrap();

    if let Ok(toast) = ToastNotification::CreateToastNotification(&xml) {
        let _ = toast.SetTag(&HSTRING::from(new_tag));
        if let Ok(notifier) = ToastNotificationManager::CreateToastNotifierWithId(&app_id) {
            let _ = notifier.Show(&toast);
        }
    }
}

#[cfg(not(windows))]
fn show_toast(_body: &str) {}

/// 应用启动时注册所有已保存的快捷键
pub fn init_shortcuts(app: &AppHandle) -> Result<(), String> {
    let settings = config::get_app_settings()?;
    let existing_configs = config::list_configs().unwrap_or_default();
    let existing_names: Vec<&str> = existing_configs.iter().map(|c| c.name.as_str()).collect();
    for binding in &settings.shortcuts {
        // 跳过方案已被删除的孤儿绑定（__default__ 是特殊的恢复默认，始终保留）
        if binding.config_name != "__default__" && !existing_names.contains(&binding.config_name.as_str()) {
            continue;
        }
        register_shortcut(app, binding)?;
    }
    Ok(())
}

/// 注册单个快捷键
fn register_shortcut(app: &AppHandle, binding: &ShortcutBinding) -> Result<(), String> {
    let shortcut_str = binding.shortcut.clone();
    let config_name = binding.config_name.clone();

    let shortcut: Shortcut = shortcut_str
        .parse()
        .map_err(|e| format!("Invalid shortcut '{}': {:?}", shortcut_str, e))?;

    app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
        if event.state() == ShortcutState::Pressed {
            let notify = config::get_app_settings()
                .map(|s| s.shortcut_notification)
                .unwrap_or(true);
            if config_name == "__default__" {
                let _ = tray::apply_default_config();
                let _ = _app.emit("config-applied", "__default__");
                if notify { show_toast("已切换到默认方案"); }
            } else if let Ok(cfg) = config::load_config(config_name.clone()) {
                let _ = tray::apply_color_config(&cfg);
                let _ = _app.emit("config-applied", &config_name);
                if notify { show_toast(&format!("已切换到方案：{}", config_name)); }
            }
        }
    }).map_err(|e| format!("Failed to register shortcut '{}': {}", shortcut_str, e))?;

    Ok(())
}

/// 注销单个快捷键（失败时忽略——可能从未注册过，如启动时被跳过的孤儿绑定）
fn unregister_shortcut(app: &AppHandle, shortcut_str: &str) {
    if let Ok(shortcut) = shortcut_str.parse::<Shortcut>() {
        let _ = app.global_shortcut().unregister(shortcut);
    }
}

/// Tauri 命令：绑定快捷键到方案
#[tauri::command]
pub fn bind_shortcut(app: AppHandle, shortcut: String, config_name: String) -> Result<(), String> {
    // 检查方案是否存在
    config::load_config(config_name.clone())?;

    let mut settings = config::get_app_settings()?;

    // 清理孤儿绑定：方案已被删除但 settings 里还残留的快捷键记录
    // 注意：__default__ 是特殊的"恢复默认"绑定，不在 list_configs 里但必须保留
    let existing_configs = config::list_configs().unwrap_or_default();
    let before_len = settings.shortcuts.len();
    settings.shortcuts.retain(|b| b.config_name == "__default__" || existing_configs.iter().any(|c| c.name == b.config_name));
    let cleaned = before_len != settings.shortcuts.len();

    // 检查冲突：同一快捷键不能绑定多个方案
    if let Some(existing) = settings.shortcuts.iter().find(|b| b.shortcut == shortcut).cloned() {
        if existing.config_name != config_name {
            if cleaned { let _ = config::save_app_settings(settings); }
            return Err(format!(
                "快捷键 '{}' 已绑定到方案 '{}'",
                shortcut, existing.config_name
            ));
        }
        if cleaned { let _ = config::save_app_settings(settings); }
        return Ok(());
    }

    // 如果该方案已有绑定，先解绑旧的
    if let Some(pos) = settings.shortcuts.iter().position(|b| b.config_name == config_name) {
        let old = settings.shortcuts.remove(pos);
        unregister_shortcut(&app, &old.shortcut);
    }

    // 注册新快捷键
    let binding = ShortcutBinding {
        shortcut: shortcut.clone(),
        config_name,
    };
    register_shortcut(&app, &binding)?;

    // 保存设置（含清理孤儿 + 新绑定）
    settings.shortcuts.push(binding);
    config::save_app_settings(settings)?;

    Ok(())
}

/// Tauri 命令：解绑快捷键
#[tauri::command]
pub fn unbind_shortcut(app: AppHandle, config_name: String) -> Result<(), String> {
    let mut settings = config::get_app_settings()?;

    if let Some(pos) = settings.shortcuts.iter().position(|b| b.config_name == config_name) {
        let binding = settings.shortcuts.remove(pos);
        unregister_shortcut(&app, &binding.shortcut);
        config::save_app_settings(settings)?;
    }

    Ok(())
}

/// Tauri 命令：获取所有快捷键绑定
#[tauri::command]
pub fn list_shortcut_bindings() -> Result<Vec<ShortcutBinding>, String> {
    let settings = config::get_app_settings()?;
    Ok(settings.shortcuts)
}

/// Tauri 命令：暂停所有全局快捷键（前端录制新快捷键时调用，
/// 避免待录制的组合键被已注册的全局快捷键拦截，导致输入框收不到按键）
#[tauri::command]
pub fn pause_shortcuts(app: AppHandle) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| format!("Failed to pause shortcuts: {}", e))
}

/// Tauri 命令：恢复所有全局快捷键（录制结束或取消后调用，从设置重新注册）
#[tauri::command]
pub fn resume_shortcuts(app: AppHandle) -> Result<(), String> {
    let _ = app.global_shortcut().unregister_all();
    init_shortcuts(&app)
}
