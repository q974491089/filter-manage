use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// ─── 公共数据类型 ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ColorConfig {
    pub name: String,
    pub icon: Option<String>,
    pub brightness: i32,
    pub contrast: i32,
    pub gamma: f64,
    pub digital_vibrance: i32,
    pub icc_profile: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShortcutBinding {
    pub shortcut: String,
    pub config_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProcessRule {
    pub id: String,
    pub process_name: String,
    pub config_name: String,
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub restore_on_exit: bool,
}

// ─── AppStore — 统一单文件结构 ───────────────────────────────────────────────
//
// 存储路径：%APPDATA%\filter-manage\app.json
//
// 云端同步说明：
//   - 同步全量 app.json 即可恢复所有预设和设置
//   - 不希望跨设备同步的字段（如 autostart）由前端在上传前排除
//   - 后端不感知云同步，保持单一职责

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    #[serde(default)]
    pub close_to_tray: Option<bool>,
    #[serde(default)]
    pub close_prompted: bool,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default)]
    pub tray_presets: Vec<String>,
    #[serde(default)]
    pub shortcuts: Vec<ShortcutBinding>,
    #[serde(default = "default_true")]
    pub shortcut_notification: bool,
    #[serde(default = "default_true")]
    pub process_watcher_enabled: bool,
    #[serde(default = "default_true")]
    pub process_notification: bool,
    #[serde(default)]
    pub process_rules: Vec<ProcessRule>,
}

fn default_true() -> bool { true }

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            close_to_tray: None,
            close_prompted: false,
            autostart: false,
            tray_presets: Vec::new(),
            shortcuts: Vec::new(),
            shortcut_notification: true,
            process_watcher_enabled: true,
            process_notification: true,
            process_rules: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AppStore {
    /// 格式版本，当前为 1
    version: u32,
    /// 默认预设名称
    #[serde(default)]
    default_preset: Option<String>,
    /// 所有颜色预设
    #[serde(default)]
    presets: Vec<ColorConfig>,
    /// 应用设置
    #[serde(default)]
    settings: AppSettings,
}

impl Default for AppStore {
    fn default() -> Self {
        Self {
            version: 1,
            default_preset: None,
            presets: Vec::new(),
            settings: AppSettings::default(),
        }
    }
}

const APP_FILE: &str = "app.json";
const DEFAULT_CONFIG_NAME: &str = "__default__";

// ─── AppStore 读写 ───────────────────────────────────────────────────────────

fn read_store() -> Result<AppStore, String> {
    let path = app_path()?;
    if !path.exists() {
        return Ok(AppStore::default());
    }
    let json = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read app.json: {}", e))?;

    // 主文件解析失败时自动降级读备份
    match serde_json::from_str::<AppStore>(&json) {
        Ok(store) => Ok(store),
        Err(_) => {
            let bak = path.with_extension("json.bak");
            if bak.exists() {
                if let Ok(bak_json) = fs::read_to_string(&bak) {
                    if let Ok(store) = serde_json::from_str::<AppStore>(&bak_json) {
                        return Ok(store);
                    }
                }
            }
            // 备份也读不了，返回默认值（总比崩溃好）
            Ok(AppStore::default())
        }
    }
}

fn write_store(store: &AppStore) -> Result<(), String> {
    let path = app_path()?;
    let json = serde_json::to_string_pretty(store)
        .map_err(|e| format!("Failed to serialize app store: {}", e))?;

    // 写前备份（app.json.bak），损坏时可手动恢复
    if path.exists() {
        let _ = fs::copy(&path, path.with_extension("json.bak"));
    }

    // 原子写：先写临时文件，再 rename，避免写入中断导致文件损坏
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)
        .map_err(|e| format!("Failed to write app.json.tmp: {}", e))?;
    fs::rename(&tmp, &path)
        .map_err(|e| format!("Failed to rename app.json.tmp: {}", e))
}

fn app_path() -> Result<PathBuf, String> {
    Ok(get_config_dir()?.join(APP_FILE))
}

// ─── 旧格式迁移 ──────────────────────────────────────────────────────────────
//
// 兼容两代旧格式：
//   1. 旧多文件格式：每个预设一个 *.json + __settings__.json
//   2. 过渡格式：profiles.json + __settings__.json
//
// 迁移是幂等的，旧文件迁移成功后删除。

pub fn migrate_legacy_files() -> Result<(), String> {
    let config_dir = get_config_dir()?;
    let mut store = read_store()?;
    let existing: std::collections::HashSet<String> =
        store.presets.iter().map(|p| p.name.clone()).collect();
    let mut dirty = false;

    // 迁移 __settings__.json
    let settings_path = config_dir.join("__settings__.json");
    if settings_path.exists() {
        if let Ok(json) = fs::read_to_string(&settings_path) {
            if let Ok(settings) = serde_json::from_str::<AppSettings>(&json) {
                store.settings = settings;
                dirty = true;
            }
        }
        let _ = fs::remove_file(&settings_path);
    }

    // 迁移 profiles.json（过渡格式）
    let profiles_path = config_dir.join("profiles.json");
    if profiles_path.exists() {
        #[derive(Deserialize)]
        struct ProfileStore {
            #[serde(default)]
            default_preset: Option<String>,
            #[serde(default)]
            presets: Vec<ColorConfig>,
        }
        if let Ok(json) = fs::read_to_string(&profiles_path) {
            if let Ok(ps) = serde_json::from_str::<ProfileStore>(&json) {
                if store.default_preset.is_none() {
                    store.default_preset = ps.default_preset;
                }
                for preset in ps.presets {
                    if !existing.contains(&preset.name) {
                        store.presets.push(preset);
                        dirty = true;
                    }
                }
            }
        }
        let _ = fs::remove_file(&profiles_path);
    }

    // 迁移散落的单个 *.json 预设文件
    if let Ok(entries) = fs::read_dir(&config_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let file_name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            if file_name == APP_FILE {
                continue;
            }
            let stem = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            if let Ok(json) = fs::read_to_string(&path) {
                if let Ok(mut config) = serde_json::from_str::<ColorConfig>(&json) {
                    if !existing.contains(&stem) {
                        if stem == DEFAULT_CONFIG_NAME {
                            config.name = DEFAULT_CONFIG_NAME.to_string();
                            if store.default_preset.is_none() {
                                store.default_preset = Some(DEFAULT_CONFIG_NAME.to_string());
                            }
                        }
                        store.presets.push(config);
                        dirty = true;
                    }
                }
            }
            let _ = fs::remove_file(&path);
        }
    }

    if dirty {
        write_store(&store)?;
    }

    Ok(())
}

// ─── 颜色预设命令（签名与旧版完全兼容）──────────────────────────────────────

#[tauri::command]
pub fn save_config(config: ColorConfig) -> Result<(), String> {
    let mut store = read_store()?;
    if let Some(existing) = store.presets.iter_mut().find(|p| p.name == config.name) {
        *existing = config;
    } else {
        store.presets.push(config);
    }
    write_store(&store)
}

#[tauri::command]
pub fn load_config(name: String) -> Result<ColorConfig, String> {
    let store = read_store()?;
    store.presets.into_iter().find(|p| p.name == name)
        .ok_or_else(|| format!("Config '{}' not found", name))
}

#[tauri::command]
pub fn list_configs() -> Result<Vec<ColorConfig>, String> {
    let store = read_store()?;
    let mut configs: Vec<ColorConfig> = store.presets.iter()
        .filter(|p| p.name != DEFAULT_CONFIG_NAME)
        .cloned()
        .collect();
    configs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(configs)
}

#[tauri::command]
pub fn delete_config(name: String) -> Result<(), String> {
    let mut store = read_store()?;
    let before = store.presets.len();
    store.presets.retain(|p| p.name != name);
    if store.presets.len() == before {
        return Err(format!("Config '{}' not found", name));
    }
    if store.default_preset.as_deref() == Some(name.as_str()) {
        store.default_preset = None;
    }
    write_store(&store)
}

#[tauri::command]
pub fn rename_config(old_name: String, new_name: String) -> Result<(), String> {
    if old_name == new_name {
        return Ok(());
    }
    let mut store = read_store()?;
    if store.presets.iter().any(|p| p.name == new_name) {
        return Err(format!("Config '{}' already exists", new_name));
    }
    let preset = store.presets.iter_mut()
        .find(|p| p.name == old_name)
        .ok_or_else(|| format!("Config '{}' not found", old_name))?;
    preset.name = new_name.clone();
    if store.default_preset.as_deref() == Some(old_name.as_str()) {
        store.default_preset = Some(new_name);
    }
    write_store(&store)
}

#[tauri::command]
pub fn save_default_config(config: ColorConfig) -> Result<(), String> {
    let mut store = read_store()?;
    if store.presets.iter().any(|p| p.name == DEFAULT_CONFIG_NAME) {
        return Ok(());
    }
    let mut default = config;
    default.name = DEFAULT_CONFIG_NAME.to_string();
    store.presets.push(default);
    store.default_preset = Some(DEFAULT_CONFIG_NAME.to_string());
    write_store(&store)
}

#[tauri::command]
pub fn overwrite_default_config(config: ColorConfig) -> Result<(), String> {
    let mut store = read_store()?;
    let mut default = config;
    default.name = DEFAULT_CONFIG_NAME.to_string();
    if let Some(existing) = store.presets.iter_mut().find(|p| p.name == DEFAULT_CONFIG_NAME) {
        *existing = default;
    } else {
        store.presets.push(default);
    }
    store.default_preset = Some(DEFAULT_CONFIG_NAME.to_string());
    write_store(&store)
}

#[tauri::command]
pub fn load_default_config() -> Result<Option<ColorConfig>, String> {
    let store = read_store()?;
    Ok(store.presets.into_iter().find(|p| p.name == DEFAULT_CONFIG_NAME))
}

// ─── 应用设置命令（签名与旧版完全兼容）──────────────────────────────────────

#[tauri::command]
pub fn get_app_settings() -> Result<AppSettings, String> {
    Ok(read_store()?.settings)
}

#[tauri::command]
pub fn save_app_settings(settings: AppSettings) -> Result<(), String> {
    let mut store = read_store()?;
    store.settings = settings;
    write_store(&store)
}

// ─── 工具函数 ────────────────────────────────────────────────────────────────

fn get_config_dir() -> Result<PathBuf, String> {
    let app_dir = dirs::config_dir()
        .ok_or_else(|| "Failed to get config directory".to_string())?
        .join("filter-manage");
    if !app_dir.exists() {
        fs::create_dir_all(&app_dir)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }
    Ok(app_dir)
}
