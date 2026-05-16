use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ColorConfig {
    pub name: String,
    pub brightness: i32,
    pub contrast: i32,
    pub gamma: f64,
    pub digital_vibrance: i32,
    pub icc_profile: Option<String>,
}

const DEFAULT_CONFIG_NAME: &str = "__default__";

#[tauri::command]
pub fn save_config(config: ColorConfig) -> Result<(), String> {
    let config_dir = get_config_dir()?;
    let config_file = config_dir.join(format!("{}.json", config.name));

    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    fs::write(&config_file, json).map_err(|e| format!("Failed to write config file: {}", e))?;

    Ok(())
}

/// 保存启动时的默认值（只在不存在时保存，避免覆盖用户设置的默认值）
#[tauri::command]
pub fn save_default_config(config: ColorConfig) -> Result<(), String> {
    let config_dir = get_config_dir()?;
    let default_file = config_dir.join(format!("{}.json", DEFAULT_CONFIG_NAME));

    // 只在第一次启动时保存（文件不存在时）
    if !default_file.exists() {
        let mut default = config;
        default.name = DEFAULT_CONFIG_NAME.to_string();
        let json = serde_json::to_string_pretty(&default)
            .map_err(|e| format!("Failed to serialize: {}", e))?;
        fs::write(&default_file, json)
            .map_err(|e| format!("Failed to write default config: {}", e))?;
    }

    Ok(())
}

/// 强制覆盖保存默认值（用户手动设置默认时调用）
#[tauri::command]
pub fn overwrite_default_config(config: ColorConfig) -> Result<(), String> {
    let config_dir = get_config_dir()?;
    let default_file = config_dir.join(format!("{}.json", DEFAULT_CONFIG_NAME));
    let mut default = config;
    default.name = DEFAULT_CONFIG_NAME.to_string();
    let json = serde_json::to_string_pretty(&default)
        .map_err(|e| format!("Failed to serialize: {}", e))?;
    fs::write(&default_file, json)
        .map_err(|e| format!("Failed to write default config: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn load_default_config() -> Result<Option<ColorConfig>, String> {
    let config_dir = get_config_dir()?;
    let default_file = config_dir.join(format!("{}.json", DEFAULT_CONFIG_NAME));

    if !default_file.exists() {
        return Ok(None);
    }

    let json = fs::read_to_string(&default_file)
        .map_err(|e| format!("Failed to read default config: {}", e))?;
    let config: ColorConfig = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to parse default config: {}", e))?;
    Ok(Some(config))
}

#[tauri::command]
pub fn load_config(name: String) -> Result<ColorConfig, String> {
    let config_dir = get_config_dir()?;
    let config_file = config_dir.join(format!("{}.json", name));

    if !config_file.exists() {
        return Err(format!("Config '{}' not found", name));
    }

    let json = fs::read_to_string(&config_file)
        .map_err(|e| format!("Failed to read config file: {}", e))?;

    let config: ColorConfig =
        serde_json::from_str(&json).map_err(|e| format!("Failed to parse config: {}", e))?;

    Ok(config)
}

#[tauri::command]
pub fn list_configs() -> Result<Vec<String>, String> {
    let config_dir = get_config_dir()?;

    if !config_dir.exists() {
        return Ok(Vec::new());
    }

    let mut configs = Vec::new();

    if let Ok(entries) = fs::read_dir(config_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                    // 不显示内部默认配置
                    if name != DEFAULT_CONFIG_NAME {
                        configs.push(name.to_string());
                    }
                }
            }
        }
    }

    configs.sort();
    Ok(configs)
}

#[tauri::command]
pub fn delete_config(name: String) -> Result<(), String> {
    let config_dir = get_config_dir()?;
    let config_file = config_dir.join(format!("{}.json", name));

    if !config_file.exists() {
        return Err(format!("Config '{}' not found", name));
    }

    fs::remove_file(&config_file).map_err(|e| format!("Failed to delete config file: {}", e))?;

    Ok(())
}

fn get_config_dir() -> Result<PathBuf, String> {
    // %APPDATA%\filter-manage — 标准 Windows 应用数据目录，重装系统后仍保留
    let app_dir = dirs::config_dir()
        .ok_or_else(|| "Failed to get config directory".to_string())?
        .join("filter-manage");

    if !app_dir.exists() {
        fs::create_dir_all(&app_dir)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    Ok(app_dir)
}
