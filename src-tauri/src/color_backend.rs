//! Per-output color backend selection. All physical writes occur under nvidia's
//! color latch; status queries only inspect device/driver state.

use crate::{config, display_capability, icc, nvidia, nvidia_csc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Backend {
    #[default]
    Standard,
    Nvidia,
}

impl Backend {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "standard" => Ok(Self::Standard),
            "nvidia" => Ok(Self::Nvidia),
            _ => Err(format!("Unknown color backend: {value}")),
        }
    }
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Nvidia => "nvidia",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DisplayColorStatus {
    pub device_id: String,
    pub stable_key: Option<String>,
    pub monitor_state: String,
    pub gpu_vendor: String,
    pub hdr_enabled: Option<bool>,
    pub preferred_backend: String,
    pub active_backend: String,
    pub nvidia_support: String,
    pub reason: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ColorApplyResult {
    pub status: DisplayColorStatus,
    pub resolution: icc::PresetResolutionResult,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
struct NvidiaSession {
    original: nvidia_csc::OriginalState,
    stable_key: Option<String>,
    expected_ramp: [u16; 768],
    display_id: u32,
    suspended: bool,
}

#[derive(Default)]
struct Runtime {
    sessions: HashMap<String, NvidiaSession>,
    profiles: HashMap<String, String>,
    warnings: HashMap<String, Vec<String>>,
    errors: HashMap<String, String>,
}

fn runtime() -> &'static Mutex<Runtime> {
    static STATE: OnceLock<Mutex<Runtime>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(Runtime::default()))
}

#[derive(Default, Serialize, Deserialize)]
struct Preferences {
    #[serde(default)]
    displays: HashMap<String, Backend>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecoveryMarker {
    device_id: String,
    stable_key: String,
    original: nvidia_csc::OriginalState,
    expected_ramp: Vec<u16>,
    #[serde(default)]
    pending_ramp: Option<Vec<u16>>,
}

fn settings_path(name: &str) -> Result<std::path::PathBuf, String> {
    Ok(config::get_config_dir()?.join(name))
}

fn read_preferences() -> Result<Preferences, String> {
    let path = settings_path("display-color-settings.json")?;
    if !path.exists() {
        return Ok(Preferences::default());
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Display color preferences are invalid: {e}"))
}

fn write_json(path: &std::path::Path, value: &impl Serialize) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let temp = path.with_extension("tmp");
    std::fs::write(
        &temp,
        serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("Cannot save color state: {e}"))?;
    std::fs::rename(&temp, path).map_err(|e| format!("Cannot commit color state: {e}"))
}

fn save_preference(key: Option<&str>, backend: Backend) -> Result<(), String> {
    if let Some(key) = key {
        let mut preferences = read_preferences()?;
        preferences.displays.insert(key.into(), backend);
        write_json(&settings_path("display-color-settings.json")?, &preferences)?;
    }
    Ok(())
}

fn read_markers() -> Result<Vec<RecoveryMarker>, String> {
    let path = settings_path("color-backend-recovery.json")?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Color recovery marker is invalid: {e}"))
}

fn record_marker(device_id: &str, session: &NvidiaSession) -> Result<(), String> {
    record_marker_pending(device_id, session, None)
}

fn record_marker_pending(
    device_id: &str,
    session: &NvidiaSession,
    pending: Option<&[u16; 768]>,
) -> Result<(), String> {
    let Some(key) = &session.stable_key else {
        return Ok(());
    };
    let mut markers = read_markers()?;
    markers.retain(|marker| marker.stable_key != *key);
    markers.push(RecoveryMarker {
        device_id: device_id.into(),
        stable_key: key.clone(),
        original: session.original.clone(),
        expected_ramp: session.expected_ramp.to_vec(),
        pending_ramp: pending.map(|ramp| ramp.to_vec()),
    });
    write_json(&settings_path("color-backend-recovery.json")?, &markers)
}

fn remove_marker(key: Option<&str>) -> Result<(), String> {
    let Some(key) = key else {
        return Ok(());
    };
    let mut markers = read_markers()?;
    markers.retain(|marker| marker.stable_key != key);
    write_json(&settings_path("color-backend-recovery.json")?, &markers)
}

pub(crate) fn active_backend(device_id: &str) -> Backend {
    if runtime().lock().unwrap().sessions.contains_key(device_id) {
        Backend::Nvidia
    } else {
        Backend::Standard
    }
}

pub(crate) fn note_profile(device_id: &str, path: &str) {
    runtime()
        .lock()
        .unwrap()
        .profiles
        .insert(device_id.into(), path.into());
}

pub(crate) fn clear_profile(device_id: &str) {
    runtime().lock().unwrap().profiles.remove(device_id);
}

pub(crate) fn profile_for_display(device_id: &str) -> Option<String> {
    runtime().lock().unwrap().profiles.get(device_id).cloned()
}

fn selected_profile(profile: Option<&str>) -> Option<&str> {
    profile
        .map(str::trim)
        .filter(|path| !matches!(*path, "" | "Default" | "Default.icc"))
}

pub(crate) fn note_warning(device_id: &str, message: String) {
    let mut state = runtime().lock().unwrap();
    let warnings = state.warnings.entry(device_id.into()).or_default();
    if !warnings.contains(&message) {
        warnings.push(message);
    }
}

fn note_error(device_id: &str, error: &str) {
    runtime()
        .lock()
        .unwrap()
        .errors
        .insert(device_id.into(), error.into());
    crate::debug_recorder::log("color-backend", format!("device={device_id} {error}"));
}

fn eligibility(cap: &display_capability::DisplayCapability) -> Result<(), String> {
    if !crate::features::nvidia_color_backend_enabled() {
        return Err("NVIDIA 专用模式尚未开放".into());
    }
    if cap.gpu_vendor != "nvidia" {
        return Err("当前目标显示器未由 NVIDIA 直接输出".into());
    }
    if cap.output_ambiguous {
        return Err("镜像或组合显示输出无法唯一对应目标屏幕".into());
    }
    if cap.hdr_enabled == Some(true) {
        return Err("NVIDIA 专用模式目前仅支持 SDR，请先关闭 HDR".into());
    }
    if cap.advanced_color_enabled != Some(false) {
        return Err(
            "目标显示器启用了高级色彩/ACM，或其状态未知；NVIDIA 专用模式首版仅支持传统 SDR".into(),
        );
    }
    match cap.hdr_enabled {
        Some(false) => Ok(()),
        Some(true) => Err("NVIDIA 专用模式目前仅支持 SDR，请先关闭 HDR".into()),
        None => Err("无法确认目标显示器的 HDR 状态，暂不能启用 NVIDIA 专用模式".into()),
    }
}

fn status_for(device_id: &str) -> Result<DisplayColorStatus, String> {
    let cap = match display_capability::inspect(device_id) {
        Ok(cap) => cap,
        Err(error) => {
            let active = active_backend(device_id).name().to_string();
            return Ok(DisplayColorStatus {
                device_id: device_id.into(),
                stable_key: None,
                monitor_state: "unknown".into(),
                gpu_vendor: "unknown".into(),
                hdr_enabled: None,
                preferred_backend: active.clone(),
                active_backend: active,
                nvidia_support: "unavailable".into(),
                reason: Some(format!(
                    "显示器诊断暂不可用，标准调色不受此查询错误阻断：{error}"
                )),
                warnings: Vec::new(),
            });
        }
    };
    let state = runtime().lock().unwrap();
    let active = state.sessions.contains_key(device_id);
    let mut warnings = state.warnings.get(device_id).cloned().unwrap_or_default();
    let last_error = state.errors.get(device_id).cloned();
    drop(state);
    let preference = read_preferences().map(|p| {
        cap.stable_key
            .as_ref()
            .and_then(|key| p.displays.get(key))
            .copied()
            .unwrap_or_default()
    });
    let preferred = match preference {
        Ok(value) => value,
        Err(error) => {
            warnings.push(error);
            if active {
                Backend::Nvidia
            } else {
                Backend::Standard
            }
        }
    };
    let capability_error = eligibility(&cap).err();
    let (support, probe_error) = if capability_error.is_some() {
        ("unavailable", None)
    } else {
        match nvidia_csc::probe(device_id) {
            Ok(probe) if probe.active_external_transform && !active => (
                "unavailable",
                Some("检测到其他 NVIDIA 颜色变换，请先关闭其他调色软件".into()),
            ),
            Ok(_) if active => {
                let session = runtime().lock().unwrap().sessions.get(device_id).cloned();
                let guard = session
                    .ok_or("NVIDIA session not found".into())
                    .and_then(|session| require_live_pipeline(device_id, &session));
                match guard {
                    Ok(()) => ("available", None),
                    Err(error) => ("unavailable", Some(error)),
                }
            }
            Ok(_) => ("unverified", None),
            Err(error) => ("unavailable", Some(error)),
        }
    };
    if cap.stable_key.is_none() {
        warnings.push("显示器身份未知：模式仅本次会话生效，自动恢复停用".into());
    }
    Ok(DisplayColorStatus {
        device_id: cap.device_id,
        stable_key: cap.stable_key,
        monitor_state: cap.monitor_state,
        gpu_vendor: cap.gpu_vendor,
        hdr_enabled: cap.hdr_enabled,
        preferred_backend: preferred.name().into(),
        active_backend: if active { "nvidia" } else { "standard" }.into(),
        nvidia_support: support.into(),
        reason: last_error.or(capability_error).or(probe_error),
        warnings,
    })
}

pub(crate) fn identity_ramp() -> [u16; 768] {
    std::array::from_fn(|i| ((i % 256) * 257) as u16)
}

/// Caller holds the color latch. The canonical curve is shared by both backends.
pub(crate) fn write_ramp_locked(device_id: &str, ramp: &[u16; 768]) -> Result<(), String> {
    let session = runtime().lock().unwrap().sessions.get(device_id).cloned();
    let result = (|| {
        if let Some(mut session) = session {
            let cap = display_capability::inspect(device_id)?;
            eligibility(&cap)?;
            if cap.stable_key != session.stable_key {
                return Err("目标显示器身份已变化，请重新选择调色模式".into());
            }
            require_live_pipeline(device_id, &session)?;
            let previous = session.clone();
            record_marker_pending(device_id, &previous, Some(ramp))?;
            session.expected_ramp = *ramp;
            let applied = run_switch_transaction(
                || nvidia_csc::write_ramp(device_id, ramp),
                || record_marker(device_id, &session),
                || {
                    combine_outcomes(
                        nvidia_csc::write_ramp(device_id, &previous.expected_ramp),
                        record_marker(device_id, &previous),
                    )
                },
            );
            if let Err(failure) = applied {
                return Err(failure.error);
            }
            {
                runtime()
                    .lock()
                    .unwrap()
                    .sessions
                    .insert(device_id.into(), session);
                crate::debug_recorder::log(
                    "color-backend",
                    format!("device={device_id} NVIDIA 曲线已写入（API 成功不等于画面验证）"),
                );
            }
            Ok(())
        } else {
            nvidia::write_gamma_ramp_raw(device_id, ramp)
        }
    })();
    match &result {
        Ok(()) => {
            runtime().lock().unwrap().errors.remove(device_id);
        }
        Err(error) => note_error(device_id, error),
    }
    result
}

fn switch_locked(device_id: &str, target: Backend) -> Result<(), String> {
    let cap = display_capability::inspect(device_id)?;
    let previous = runtime().lock().unwrap().sessions.get(device_id).cloned();
    let ramp = nvidia::build_gamma_ramp(device_id);
    if active_backend(device_id) == target {
        let old_ramp = match &previous {
            Some(session) => session.expected_ramp,
            None => nvidia::read_gamma_ramp(device_id)?,
        };
        if let Err(failure) = run_switch_transaction(
            || write_ramp_locked(device_id, &ramp),
            || save_preference(cap.stable_key.as_deref(), target),
            || write_ramp_locked(device_id, &old_ramp),
        ) {
            return Err(failure.error);
        }
        nvidia::cache_final_ramp(device_id, &ramp);
        return Ok(());
    }
    match target {
        Backend::Nvidia => {
            eligibility(&cap)?;
            let display_id = nvidia_csc::probe(device_id)?.display_id;
            let original = nvidia_csc::capture_idle(device_id)?;
            // A real backup is required before any neutralization or CSC write.
            let legacy = nvidia::read_gamma_ramp(device_id)?;
            let session = NvidiaSession {
                original,
                stable_key: cap.stable_key.clone(),
                expected_ramp: ramp,
                display_id,
                suspended: false,
            };
            record_marker(device_id, &session)?;
            let applied = run_switch_transaction(
                || {
                    nvidia::write_gamma_ramp_raw(device_id, &identity_ramp())?;
                    nvidia_csc::write_ramp(device_id, &ramp)
                },
                || save_preference(cap.stable_key.as_deref(), target),
                || {
                    combine_outcomes(
                        nvidia_csc::restore(device_id, &session.original),
                        nvidia::write_gamma_ramp_raw(device_id, &legacy),
                    )
                },
            );
            if let Err(failure) = applied {
                let cleanup = if failure.rolled_back {
                    remove_marker(cap.stable_key.as_deref())
                } else {
                    Ok(())
                };
                return Err(append_cleanup_error(failure.error, cleanup));
            }
            runtime()
                .lock()
                .unwrap()
                .sessions
                .insert(device_id.into(), session);
        }
        Backend::Standard => {
            let session = previous.ok_or("NVIDIA session not found")?;
            if cap.stable_key != session.stable_key {
                return Err("目标显示器身份已变化，不能向其他屏幕恢复".into());
            }
            if cap.output_ambiguous {
                return Err("无法唯一确认 NVIDIA 输出，未执行标准模式恢复".into());
            }
            require_pipeline(device_id, &session, true)?;
            let previous_base = nvidia::icc_base_for_display(device_id);
            let profile = runtime().lock().unwrap().profiles.get(device_id).cloned();
            let association_profile = prepare_standard_profile(profile.as_deref())?;
            let applied = run_switch_transaction(
                || {
                    nvidia_csc::restore(device_id, &session.original)?;
                    icc::apply_icc_profile_standard(&association_profile, Some(device_id.into()))?;
                    nvidia::write_gamma_ramp_raw(device_id, &ramp)
                },
                || save_preference(cap.stable_key.as_deref(), target),
                || {
                    nvidia::set_icc_base_ramp_for_display(device_id, previous_base);
                    restore_profile_note(device_id, profile.as_deref());
                    // A may have written a nonneutral LUT before preference save failed.
                    nvidia::write_gamma_ramp_raw(device_id, &identity_ramp())?;
                    nvidia_csc::write_ramp(device_id, &session.expected_ramp)
                },
            );
            if let Err(failure) = applied {
                return Err(failure.error);
            }
            runtime().lock().unwrap().sessions.remove(device_id);
            if let Err(error) = remove_marker(cap.stable_key.as_deref()) {
                note_warning(device_id, error);
            }
        }
    }
    nvidia::cache_final_ramp(device_id, &ramp);
    runtime().lock().unwrap().errors.remove(device_id);
    Ok(())
}

fn require_live_pipeline(device_id: &str, session: &NvidiaSession) -> Result<(), String> {
    require_pipeline(device_id, session, false)
}

fn require_pipeline(
    device_id: &str,
    session: &NvidiaSession,
    explicit_standard_return: bool,
) -> Result<(), String> {
    live_permission(
        session.suspended,
        explicit_standard_return,
        session.display_id,
        nvidia_csc::probe(device_id)?.display_id,
        nvidia_csc::matches_owned_pipeline(device_id)?,
    )
}

fn live_permission(
    suspended: bool,
    explicit_standard_return: bool,
    owned_id: u32,
    current_id: u32,
    matrix_matches: bool,
) -> Result<(), String> {
    if owned_id != current_id {
        return Err("NVIDIA 输出身份已变化，未向其他显示器写入".into());
    }
    if !matrix_matches {
        return Err("NVIDIA 颜色管线已被其他设置改变，未覆盖当前变换，请先关闭其他调色软件".into());
    }
    if suspended && !explicit_standard_return {
        return Err(
            "目标显示器身份未知且显示环境已变化，已停止自动写入；确认当前屏幕后可手动返回标准模式"
                .into(),
        );
    }
    Ok(())
}

fn restore_profile_note(device_id: &str, profile: Option<&str>) {
    match profile {
        Some(path) => note_profile(device_id, path),
        None => clear_profile(device_id),
    }
}

fn prepare_standard_profile(profile: Option<&str>) -> Result<String, String> {
    let path = match profile {
        Some(path) => path.to_string(),
        None => icc::find_system_default_profile()?
            .to_string_lossy()
            .to_string(),
    };
    icc::read_profile_curve(&path)?;
    Ok(path)
}

#[derive(Debug)]
struct SwitchFailure {
    error: String,
    rolled_back: bool,
}

/// Production transaction boundary, injected in tests without touching a screen.
fn run_switch_transaction(
    apply: impl FnOnce() -> Result<(), String>,
    persist: impl FnOnce() -> Result<(), String>,
    rollback: impl FnOnce() -> Result<(), String>,
) -> Result<(), SwitchFailure> {
    if let Err(error) = apply().and_then(|()| persist()) {
        let result = rollback();
        let rolled_back = result.is_ok();
        return Err(SwitchFailure {
            error: with_rollback(error, result),
            rolled_back,
        });
    }
    Ok(())
}

fn combine_outcomes(first: Result<(), String>, second: Result<(), String>) -> Result<(), String> {
    match (first, second) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(first), Err(second)) => Err(format!("{first}；{second}")),
    }
}

fn append_cleanup_error(error: String, cleanup: Result<(), String>) -> String {
    match cleanup {
        Ok(()) => error,
        Err(cleanup) => format!("{error}；恢复记录保存失败：{cleanup}"),
    }
}

fn with_rollback(error: String, rollback: Result<(), String>) -> String {
    match rollback {
        Ok(()) => format!("{error}；已回退到切换前状态"),
        Err(rollback) => format!("{error}；回退失败：{rollback}，请手动恢复默认或重启"),
    }
}

pub(crate) fn apply_icc_locked(device_id: &str, profile_path: &str) -> Result<(), String> {
    let curve = icc::read_profile_curve(profile_path)?;
    let previous = nvidia::icc_base_for_display(device_id);
    nvidia::set_icc_base_ramp_for_display(device_id, curve);
    if let Err(error) = nvidia::apply_gamma_ramp_locked(device_id) {
        nvidia::set_icc_base_ramp_for_display(device_id, previous);
        return Err(error);
    }
    note_profile(device_id, profile_path);
    if curve.is_none() {
        note_warning(
            device_id,
            "该 ICC 无有效 VCGT，NVIDIA 模式仅应用滑块参数".into(),
        );
    }
    Ok(())
}

pub(crate) fn apply_config_locked(
    device_id: &str,
    cfg: &config::ColorConfig,
) -> Result<Vec<String>, String> {
    if !cfg.gamma.is_finite() {
        return Err("Gamma must be finite".into());
    }
    runtime().lock().unwrap().warnings.remove(device_id);
    let previous_settings = nvidia::get_or_default_settings(device_id);
    let previous_base = nvidia::icc_base_for_display(device_id);
    let previous_profile = profile_for_display(device_id);
    let result = with_logical_rollback(
        || {
            let base = match selected_profile(cfg.icc_profile.as_deref()) {
                Some(path) => {
                    let curve = icc::read_profile_curve(path)?;
                    if active_backend(device_id) == Backend::Standard {
                        icc::apply_icc_profile(path, Some(device_id.into()))?;
                    }
                    note_profile(device_id, path);
                    if curve.is_none() {
                        note_warning(device_id, "该 ICC 无有效 VCGT，仅应用滑块参数".into());
                    }
                    curve
                }
                None => {
                    clear_profile(device_id);
                    if active_backend(device_id) == Backend::Standard {
                        icc::restore_default_icc(Some(device_id.into()))?;
                    }
                    None
                }
            };
            nvidia::set_icc_base_ramp_for_display(device_id, base);
            nvidia::replace_settings(
                device_id,
                nvidia::NvidiaSettings {
                    brightness: cfg.brightness.clamp(-125, 125),
                    contrast: cfg.contrast.clamp(-82, 82),
                    gamma: cfg.gamma.clamp(0.4, 2.8),
                    digital_vibrance: cfg.digital_vibrance.clamp(0, 100),
                    rgb_r: cfg.rgb_r.clamp(-100, 100),
                    rgb_g: cfg.rgb_g.clamp(-100, 100),
                    rgb_b: cfg.rgb_b.clamp(-100, 100),
                },
            );
            if let Err(error) = nvidia::apply_gamma_ramp_locked(device_id) {
                return Err(error);
            }
            if let Err(error) =
                nvidia::dvc_set_locked(device_id, cfg.digital_vibrance.clamp(0, 100))
            {
                note_warning(device_id, format!("颜色曲线已写入，数字振动失败：{error}"));
            }
            Ok(runtime()
                .lock()
                .unwrap()
                .warnings
                .get(device_id)
                .cloned()
                .unwrap_or_default())
        },
        || {
            nvidia::replace_settings(device_id, previous_settings);
            nvidia::set_icc_base_ramp_for_display(device_id, previous_base);
            restore_profile_note(device_id, previous_profile.as_deref());
        },
    );
    if let Err(error) = &result {
        note_error(device_id, error);
    }
    result
}

fn with_logical_rollback<T>(
    apply: impl FnOnce() -> Result<T, String>,
    rollback: impl FnOnce(),
) -> Result<T, String> {
    let result = apply();
    if result.is_err() {
        rollback();
    }
    result
}

pub(crate) fn restore_default_locked(device_id: &str) -> Result<(), String> {
    if let Some(cfg) = config::load_default_config()? {
        apply_config_locked(device_id, &cfg)?;
    } else {
        let previous_settings = nvidia::get_or_default_settings(device_id);
        let previous_base = nvidia::icc_base_for_display(device_id);
        let previous_profile = profile_for_display(device_id);
        let result = with_logical_rollback(
            || {
                clear_profile(device_id);
                nvidia::replace_settings(device_id, nvidia::NvidiaSettings::default());
                nvidia::set_icc_base_ramp_for_display(device_id, None);
                nvidia::apply_gamma_ramp_locked(device_id)?;
                if let Err(error) = nvidia::dvc_set_locked(device_id, 50) {
                    note_warning(device_id, error);
                }
                Ok(())
            },
            || {
                nvidia::replace_settings(device_id, previous_settings);
                nvidia::set_icc_base_ramp_for_display(device_id, previous_base);
                restore_profile_note(device_id, previous_profile.as_deref());
            },
        );
        if let Err(error) = &result {
            note_error(device_id, error);
        }
        result?;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_display_color_status(
    device_id: Option<String>,
) -> Result<DisplayColorStatus, String> {
    tauri::async_runtime::spawn_blocking(move || status_for(&nvidia::resolve_display_id(device_id)))
        .await
        .map_err(|e| format!("Color status task failed: {e}"))?
}

#[tauri::command]
pub async fn set_display_color_backend(
    device_id: String,
    backend: String,
) -> Result<DisplayColorStatus, String> {
    let target = Backend::parse(&backend)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _latch = nvidia::begin_external_write();
        if let Err(error) = switch_locked(&device_id, target) {
            note_error(&device_id, &error);
            return Err(error);
        }
        status_for(&device_id)
    })
    .await
    .map_err(|e| format!("Backend switch task failed: {e}"))?
}

#[tauri::command]
pub async fn apply_display_color_config(
    device_id: Option<String>,
    config: config::ColorConfig,
) -> Result<ColorApplyResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let did = nvidia::resolve_display_id(device_id);
        let _latch = nvidia::begin_external_write();
        let mut warnings = Vec::new();
        let resolution = match icc::apply_preset_resolution_state(
            Some(did.clone()),
            config.resolution.as_deref(),
        ) {
            Ok(result) => result,
            Err(error) => {
                warnings.push(format!("分辨率应用失败：{error}"));
                icc::PresetResolutionResult::Unchanged
            }
        };
        warnings.extend(apply_config_locked(&did, &config)?);
        let status = status_for(&did)?;
        Ok(ColorApplyResult {
            status,
            resolution,
            warnings,
        })
    })
    .await
    .map_err(|e| format!("Color apply task failed: {e}"))?
}

#[tauri::command]
pub fn open_display_device_manager() -> Result<(), String> {
    std::process::Command::new("mmc.exe")
        .arg("devmgmt.msc")
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Cannot open Device Manager: {e}"))
}

/// Called outside the feature-state mutex. Persisted preferences never choose a
/// different physical output; current live color parameters are reapplied.
pub(crate) fn on_feature_flag_changed(enabled: bool, expected_generation: u64) {
    if crate::features::nvidia_color_backend_enabled() != enabled {
        return;
    }
    if enabled && crate::process_watcher::apply_generation() != expected_generation {
        return;
    }
    let targets: Vec<String> = if enabled {
        let preferences = read_preferences().unwrap_or_default();
        icc::enumerate_display_monitors()
            .into_iter()
            .filter_map(|monitor| {
                let cap = display_capability::inspect(&monitor.device_id).ok()?;
                let preferred = cap
                    .stable_key
                    .as_ref()
                    .and_then(|key| preferences.displays.get(key));
                (preferred == Some(&Backend::Nvidia)
                    && active_backend(&monitor.device_id) == Backend::Standard
                    && eligibility(&cap).is_ok())
                .then_some(monitor.device_id)
            })
            .collect()
    } else {
        runtime().lock().unwrap().sessions.keys().cloned().collect()
    };
    // Most users have no B transition at all. A feature fetch must not bump the
    // A generation or invalidate its game recovery simply because B is disabled.
    with_transition_targets(&targets, || {
        let _latch = nvidia::begin_external_write();
        if crate::features::nvidia_color_backend_enabled() != enabled {
            return;
        }
        if enabled
            && crate::process_watcher::apply_generation() != expected_generation.wrapping_add(1)
        {
            return;
        }
        for device in targets.iter() {
            if enabled {
                let Ok(cap) = display_capability::inspect(device) else {
                    continue;
                };
                let preferred = read_preferences().ok().and_then(|p| {
                    cap.stable_key
                        .as_ref()
                        .and_then(|key| p.displays.get(key))
                        .copied()
                });
                if preferred != Some(Backend::Nvidia) || active_backend(device) == Backend::Nvidia {
                    continue;
                }
                if let Err(error) = switch_locked(device, Backend::Nvidia) {
                    note_error(device, &error);
                }
            } else if active_backend(device) == Backend::Nvidia {
                if let Err(error) = teardown_default_locked(device) {
                    note_error(device, &error);
                }
            }
        }
    });
}

fn with_transition_targets(targets: &[String], transition: impl FnOnce()) {
    if !targets.is_empty() {
        transition();
    }
}

pub(crate) fn shutdown() {
    let devices: Vec<String> = runtime().lock().unwrap().sessions.keys().cloned().collect();
    let _latch = nvidia::begin_external_write();
    for device in devices {
        if let Err(error) = teardown_default_locked(&device) {
            note_error(&device, &error);
        }
    }
}

/// A normal exit cannot leave an unowned custom B curve on a driver whose V2
/// readback is unavailable. Teardown uses the standard software default; failures
/// keep the journal and report manual recovery instead of declaring success.
fn teardown_default_locked(device_id: &str) -> Result<(), String> {
    let session = runtime()
        .lock()
        .unwrap()
        .sessions
        .get(device_id)
        .cloned()
        .ok_or("NVIDIA session not found")?;
    let cap = display_capability::inspect(device_id)?;
    if cap.stable_key != session.stable_key {
        return Err("显示器身份已变化，未执行退出恢复".into());
    }
    require_live_pipeline(device_id, &session)?;
    // Prepare every disk/profile-dependent part before the first physical write.
    let default = config::load_default_config()?;
    if default.as_ref().is_some_and(|cfg| !cfg.gamma.is_finite()) {
        return Err("Default gamma must be finite".into());
    }
    let settings = match &default {
        Some(cfg) => nvidia::NvidiaSettings {
            brightness: cfg.brightness.clamp(-125, 125),
            contrast: cfg.contrast.clamp(-82, 82),
            gamma: cfg.gamma.clamp(0.4, 2.8),
            digital_vibrance: cfg.digital_vibrance.clamp(0, 100),
            rgb_r: cfg.rgb_r.clamp(-100, 100),
            rgb_g: cfg.rgb_g.clamp(-100, 100),
            rgb_b: cfg.rgb_b.clamp(-100, 100),
        },
        None => nvidia::NvidiaSettings::default(),
    };
    let profile = selected_profile(default.as_ref().and_then(|cfg| cfg.icc_profile.as_deref()));
    let association_profile = match profile {
        Some(path) => path.to_string(),
        None => icc::find_system_default_profile()?
            .to_string_lossy()
            .to_string(),
    };
    let base = icc::read_profile_curve(&association_profile)?;
    let ramp = nvidia::compose_gamma_ramp(&settings, base.as_ref());
    let previous_settings = nvidia::get_or_default_settings(device_id);
    let previous_base = nvidia::icc_base_for_display(device_id);
    let previous_profile = profile_for_display(device_id);
    let outcome = run_switch_transaction(
        || {
            nvidia_csc::restore(device_id, &session.original)?;
            icc::apply_icc_profile_standard(&association_profile, Some(device_id.into()))?;
            nvidia::write_gamma_ramp_raw(device_id, &ramp)
        },
        || remove_marker(session.stable_key.as_deref()),
        || {
            nvidia::replace_settings(device_id, previous_settings);
            nvidia::set_icc_base_ramp_for_display(device_id, previous_base);
            restore_profile_note(device_id, previous_profile.as_deref());
            nvidia::write_gamma_ramp_raw(device_id, &identity_ramp())?;
            combine_outcomes(
                nvidia_csc::write_ramp(device_id, &session.expected_ramp),
                record_marker(device_id, &session),
            )
        },
    );
    if let Err(failure) = outcome {
        return Err(failure.error);
    }
    nvidia::replace_settings(device_id, settings.clone());
    nvidia::set_icc_base_ramp_for_display(device_id, base);
    restore_profile_note(device_id, profile);
    if let Err(error) = nvidia::dvc_set_locked(device_id, settings.digital_vibrance) {
        note_warning(device_id, error);
    }
    nvidia::cache_final_ramp(device_id, &ramp);
    runtime().lock().unwrap().sessions.remove(device_id);
    Ok(())
}

/// Unknown physical identities are session-only and cannot survive a topology or
/// mode event. This notification suspends writes; it does not change any colors.
pub(crate) fn on_display_environment_changed() {
    let mut state = runtime().lock().unwrap();
    for session in state.sessions.values_mut() {
        if session.stable_key.is_none() {
            session.suspended = true;
        }
    }
}

/// Startup cleanup only accepts a full matching private LUT readback. A marker
/// alone never proves that the current transform still belongs to this app.
pub(crate) fn initialize() {
    let markers = match read_markers() {
        Ok(markers) => markers,
        Err(error) => {
            eprintln!("[color-backend] {error}");
            return;
        }
    };
    for marker in markers {
        let Ok(cap) = display_capability::inspect(&marker.device_id) else {
            continue;
        };
        if cap.stable_key.as_deref() != Some(&marker.stable_key) {
            continue;
        }
        let Ok(expected) = <[u16; 768]>::try_from(marker.expected_ramp.as_slice()) else {
            note_error(&marker.device_id, "恢复记录曲线长度无效，未修改显示输出");
            continue;
        };
        let matches =
            nvidia_csc::is_current_ours(&marker.device_id, &expected).and_then(|matches| {
                if matches {
                    return Ok(true);
                }
                let Some(pending) = &marker.pending_ramp else {
                    return Ok(false);
                };
                let pending = <[u16; 768]>::try_from(pending.as_slice())
                    .map_err(|_| "待提交曲线长度无效".to_string())?;
                nvidia_csc::is_current_ours(&marker.device_id, &pending)
            });
        match matches {
            Ok(true) => {
                let _latch = nvidia::begin_external_write();
                let outcome = nvidia_csc::restore(&marker.device_id, &marker.original)
                    .and_then(|()| restore_default_locked(&marker.device_id))
                    .and_then(|()| remove_marker(Some(&marker.stable_key)));
                if let Err(error) = outcome { note_error(&marker.device_id, &error); }
            }
            Ok(false) => note_error(&marker.device_id, "当前 NVIDIA 变换与本软件恢复记录不同，未覆盖其他调色状态"),
            Err(error) => note_error(&marker.device_id, &format!("存在待清理的 NVIDIA 曲线，但无法完整确认所有权；未自动修改，请手动恢复默认：{error}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_backend_and_names_are_compatible() {
        assert_eq!(Backend::default(), Backend::Standard);
        assert_eq!(Backend::parse("standard").unwrap(), Backend::Standard);
        assert_eq!(Backend::parse("nvidia").unwrap(), Backend::Nvidia);
        assert!(Backend::parse("automatic").is_err());
    }

    #[test]
    fn neutral_curve_keeps_every_channel_and_endpoint() {
        let ramp = identity_ramp();
        for channel in 0..3 {
            assert_eq!(ramp[channel * 256], 0);
            assert_eq!(ramp[channel * 256 + 255], 65535);
            assert_eq!(ramp[channel * 256 + 128], 32896);
        }
    }

    #[test]
    fn rollback_failure_is_never_reported_as_restored() {
        assert!(with_rollback("write failed".into(), Ok(())).contains("已回退"));
        let error = with_rollback("write failed".into(), Err("driver refused".into()));
        assert!(error.contains("回退失败"));
        assert!(!error.contains("已回退"));
    }

    #[test]
    fn failed_neutralization_never_runs_csc_or_commits_preference() {
        use std::cell::RefCell;
        let events = RefCell::new(Vec::new());
        let outcome = run_switch_transaction(
            || {
                events.borrow_mut().push("neutralize");
                let neutralized: Result<(), String> = Err("GDI rejected".into());
                neutralized?;
                events.borrow_mut().push("set-csc");
                Ok(())
            },
            || {
                events.borrow_mut().push("save-preference");
                Ok(())
            },
            || {
                events.borrow_mut().push("restore-idle");
                events.borrow_mut().push("restore-gdi");
                Ok(())
            },
        )
        .unwrap_err();
        assert!(outcome.rolled_back);
        assert_eq!(
            *events.borrow(),
            ["neutralize", "restore-idle", "restore-gdi"]
        );
    }

    #[test]
    fn preference_failure_neutralizes_a_before_restoring_b() {
        use std::cell::RefCell;
        let events = RefCell::new(Vec::new());
        let outcome = run_switch_transaction(
            || {
                events
                    .borrow_mut()
                    .extend(["restore-idle", "associate-icc", "write-a"]);
                Ok(())
            },
            || {
                events.borrow_mut().push("save-preference");
                Err("disk full".into())
            },
            || {
                events.borrow_mut().extend(["neutralize-gdi", "restore-b"]);
                Ok(())
            },
        )
        .unwrap_err();
        assert!(outcome.rolled_back);
        assert_eq!(
            *events.borrow(),
            [
                "restore-idle",
                "associate-icc",
                "write-a",
                "save-preference",
                "neutralize-gdi",
                "restore-b"
            ]
        );
    }

    #[test]
    fn native_write_failure_restores_committed_marker_and_skips_commit() {
        use std::cell::Cell;
        let current = Cell::new(10);
        let committed = Cell::new(10);
        let pending = Cell::new(Some(20));
        let commits = Cell::new(0);
        let outcome = run_switch_transaction(
            || {
                current.set(20);
                Err("partial driver write".into())
            },
            || {
                commits.set(commits.get() + 1);
                committed.set(20);
                pending.set(None);
                Ok(())
            },
            || {
                current.set(committed.get());
                pending.set(None);
                Ok(())
            },
        )
        .unwrap_err();
        assert!(outcome.rolled_back);
        assert_eq!(current.get(), 10);
        assert_eq!(committed.get(), 10);
        assert_eq!(pending.get(), None);
        assert_eq!(commits.get(), 0);
    }

    #[test]
    fn journal_commit_failure_restores_previous_curve_and_marker() {
        use std::cell::Cell;
        let current = Cell::new(10);
        let committed = Cell::new(10);
        let pending = Cell::new(Some(20));
        let outcome = run_switch_transaction(
            || {
                current.set(20);
                Ok(())
            },
            || Err("journal rename failed".into()),
            || {
                current.set(committed.get());
                pending.set(None);
                Ok(())
            },
        )
        .unwrap_err();
        assert!(outcome.rolled_back);
        assert_eq!(current.get(), 10);
        assert_eq!(committed.get(), 10);
        assert_eq!(pending.get(), None);
    }

    #[test]
    fn both_rollback_errors_survive_in_failure_report() {
        let error = combine_outcomes(
            Err("CSC rollback failed".into()),
            Err("GDI rollback failed".into()),
        )
        .unwrap_err();
        assert!(error.contains("CSC rollback failed"));
        assert!(error.contains("GDI rollback failed"));
        let error = append_cleanup_error(
            "driver rejected; rollback failed".into(),
            Err("journal disk full".into()),
        );
        assert!(error.contains("driver rejected"));
        assert!(error.contains("journal disk full"));
    }

    #[test]
    fn disabled_feature_without_b_sessions_does_not_touch_a_generation() {
        use std::cell::Cell;
        let generation = Cell::new(7);
        let writes = Cell::new(0);
        with_transition_targets(&[], || {
            generation.set(8);
            writes.set(1);
        });
        assert_eq!(generation.get(), 7);
        assert_eq!(writes.get(), 0);
    }

    #[test]
    fn enabled_feature_without_b_preferences_does_not_touch_a_generation() {
        use std::cell::Cell;
        let preferences = Preferences::default();
        let targets: Vec<String> = preferences
            .displays
            .into_iter()
            .filter_map(|(key, backend)| (backend == Backend::Nvidia).then_some(key))
            .collect();
        let generation = Cell::new(10);
        with_transition_targets(&targets, || generation.set(11));
        assert_eq!(generation.get(), 10);
    }

    #[test]
    fn legacy_default_sentinels_do_not_hide_explicit_profile_paths() {
        assert_eq!(selected_profile(Some("Default")), None);
        assert_eq!(selected_profile(Some("Default.icc")), None);
        assert_eq!(selected_profile(Some("  ")), None);
        assert_eq!(
            selected_profile(Some("C:\\profiles\\Default.icc")),
            Some("C:\\profiles\\Default.icc")
        );
        assert_eq!(selected_profile(Some("real.icc")), Some("real.icc"));
    }

    #[test]
    fn failed_profile_apply_and_clear_keep_previous_selection() {
        use std::cell::RefCell;
        let profile = RefCell::new(Some("a.icc"));
        let failure = with_logical_rollback(
            || {
                *profile.borrow_mut() = Some("b.icc");
                Err::<(), _>("driver rejected".into())
            },
            || *profile.borrow_mut() = Some("a.icc"),
        );
        assert!(failure.is_err());
        assert_eq!(*profile.borrow(), Some("a.icc"));
        let failure = with_logical_rollback(
            || {
                *profile.borrow_mut() = None;
                Err::<(), _>("default write rejected".into())
            },
            || *profile.borrow_mut() = Some("a.icc"),
        );
        assert!(failure.is_err());
        assert_eq!(*profile.borrow(), Some("a.icc"));
    }

    #[test]
    fn suspended_session_allows_only_explicit_matching_standard_return() {
        assert!(live_permission(true, false, 7, 7, true).is_err());
        assert!(live_permission(true, true, 7, 7, true).is_ok());
        assert!(live_permission(true, true, 7, 8, true).is_err());
        assert!(live_permission(true, true, 7, 7, false).is_err());
    }
}
