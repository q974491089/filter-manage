use filter_manage_platform::{
    enumerate_displays, read_gamma_ramp, restore_probe as restore_probe_values, snapshot,
    write_gamma_ramp, DisplayInfo, DisplaySnapshot, ProbeRamp,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

const EVENT_NAME: &str = "diagnostic://event";
const REPORT_FILENAME: &str = "diagnostic-report-v1.json";
const DEFAULT_TERMS: [&str; 4] = ["valorant", "riotclient", "vgc", "filter-manage"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticEvent {
    pub timestamp_ms: u128,
    pub kind: String,
    pub message: String,
    pub snapshot: Option<DisplaySnapshot>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MonitorState {
    pub running: bool,
    pub display_id: Option<String>,
    pub event_count: usize,
    pub probe_active: bool,
}

#[derive(Debug, Serialize)]
struct DiagnosticReport {
    schema_version: &'static str,
    generated_at_ms: u128,
    tool: &'static str,
    os: SystemSummary,
    displays: Vec<DisplayInfo>,
    display_id: Option<String>,
    process_terms: Vec<String>,
    events: Vec<DiagnosticEvent>,
    probe_active: bool,
}

#[derive(Debug, Serialize)]
struct SystemSummary {
    architecture: &'static str,
    windows_version: Option<String>,
}

struct InnerState {
    running: bool,
    stop_requested: bool,
    monitor_thread: Option<thread::JoinHandle<()>>,
    display_id: Option<String>,
    process_terms: Vec<String>,
    events: Vec<DiagnosticEvent>,
    previous: Option<DisplaySnapshot>,
    probe: Option<ProbeRamp>,
}

#[derive(Clone)]
pub struct DiagnosticState(Arc<Mutex<InnerState>>);

impl Default for DiagnosticState {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(InnerState {
            running: false,
            stop_requested: false,
            monitor_thread: None,
            display_id: None,
            process_terms: DEFAULT_TERMS
                .iter()
                .map(|term| (*term).to_string())
                .collect(),
            events: Vec::new(),
            previous: None,
            probe: None,
        })))
    }
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn system_summary() -> SystemSummary {
    let windows_version = if cfg!(windows) {
        std::process::Command::new("cmd")
            .args(["/C", "ver"])
            .output()
            .ok()
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .filter(|value| !value.is_empty())
    } else {
        None
    };
    SystemSummary {
        architecture: std::env::consts::ARCH,
        windows_version,
    }
}

fn record(
    state: &Arc<Mutex<InnerState>>,
    app: &AppHandle,
    kind: &str,
    message: String,
    snapshot: Option<DisplaySnapshot>,
) {
    let event = DiagnosticEvent {
        timestamp_ms: now_ms(),
        kind: kind.into(),
        message,
        snapshot,
    };
    if let Ok(mut inner) = state.lock() {
        inner.events.push(event.clone());
        if inner.events.len() > 2_000 {
            let remove = inner.events.len() - 2_000;
            inner.events.drain(0..remove);
        }
    }
    let _ = app.emit(EVENT_NAME, event);
}

fn snapshot_changes(
    previous: &DisplaySnapshot,
    current: &DisplaySnapshot,
) -> Vec<(String, String)> {
    let mut changes = Vec::new();
    if previous.mode != current.mode {
        changes.push(("DISPLAY_MODE_CHANGED".into(), "显示模式发生变化".into()));
    }
    if previous.gamma.signature != current.gamma.signature {
        changes.push(("GAMMA_RAMP_CHANGED".into(), "Gamma ramp 发生变化".into()));
    }
    let old_processes = previous
        .processes
        .iter()
        .map(|process| (process.name.clone(), process.pid))
        .collect::<std::collections::BTreeSet<_>>();
    let new_processes = current
        .processes
        .iter()
        .map(|process| (process.name.clone(), process.pid))
        .collect::<std::collections::BTreeSet<_>>();
    for process in new_processes.difference(&old_processes) {
        changes.push((
            "PROCESS_STARTED".into(),
            format!("进程启动: {} ({})", process.0, process.1),
        ));
    }
    for process in old_processes.difference(&new_processes) {
        changes.push((
            "PROCESS_STOPPED".into(),
            format!("进程退出: {} ({})", process.0, process.1),
        ));
    }
    changes
}

fn snapshot_changed(
    previous: &DisplaySnapshot,
    current: &DisplaySnapshot,
    state: &Arc<Mutex<InnerState>>,
    app: &AppHandle,
) {
    for (kind, message) in snapshot_changes(previous, current) {
        record(state, app, &kind, message, Some(current.clone()));
    }
}

fn request_monitor_stop(state: &Arc<Mutex<InnerState>>) -> bool {
    match state.lock() {
        Ok(mut inner) => {
            let was_running = inner.running;
            inner.stop_requested = true;
            was_running
        }
        Err(_) => false,
    }
}

fn wait_for_monitor_stop(state: &Arc<Mutex<InnerState>>) {
    let handle = state
        .lock()
        .ok()
        .and_then(|mut inner| inner.monitor_thread.take());
    if let Some(handle) = handle {
        let _ = handle.join();
        if let Ok(mut inner) = state.lock() {
            inner.running = false;
            inner.stop_requested = false;
        }
    }
    while state.lock().map(|inner| inner.running).unwrap_or(false) {
        thread::sleep(Duration::from_millis(25));
    }
}

fn restore_active_probe(state: &Arc<Mutex<InnerState>>) -> Result<bool, String> {
    let probe = state.lock().map_err(|_| "诊断状态锁定失败")?.probe.clone();
    let Some(probe) = probe else {
        return Ok(false);
    };
    restore_probe_values(&probe)?;
    clear_recovery_file();
    if let Ok(mut inner) = state.lock() {
        inner.probe = None;
    }
    Ok(true)
}

fn restore_probe_with_event(
    state: &Arc<Mutex<InnerState>>,
    app: &AppHandle,
) -> Result<bool, String> {
    match restore_active_probe(state) {
        Ok(true) => {
            record(
                state,
                app,
                "PROBE_RESTORE",
                "已恢复原始 Gamma ramp".into(),
                None,
            );
            Ok(true)
        }
        Ok(false) => Ok(false),
        Err(error) => {
            record(
                state,
                app,
                "PROBE_RESTORE_FAILED",
                format!("恢复原始 Gamma ramp 失败: {}", error),
                None,
            );
            Err(error)
        }
    }
}

fn recovery_file() -> Option<std::path::PathBuf> {
    dirs::data_local_dir().map(|directory| {
        directory
            .join("Filter-Manage")
            .join("diagnostic-probe.json")
    })
}

fn persist_recovery(probe: &ProbeRamp) -> Result<(), String> {
    let path = recovery_file().ok_or("找不到本地数据目录")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("创建恢复目录失败: {}", error))?;
    }
    let data = serde_json::to_vec(probe).map_err(|error| error.to_string())?;
    std::fs::write(path, data).map_err(|error| format!("保存 Gamma 恢复状态失败: {}", error))
}

fn clear_recovery_file() {
    if let Some(path) = recovery_file() {
        let _ = std::fs::remove_file(path);
    }
}

fn recover_previous_probe() -> Result<bool, String> {
    let Some(path) = recovery_file() else {
        return Ok(false);
    };
    if !path.exists() {
        return Ok(false);
    }
    let data =
        std::fs::read(&path).map_err(|error| format!("读取 Gamma 恢复状态失败: {}", error))?;
    let probe: ProbeRamp = serde_json::from_slice(&data)
        .map_err(|error| format!("解析 Gamma 恢复状态失败: {}", error))?;
    restore_probe_values(&probe)?;
    clear_recovery_file();
    Ok(true)
}

#[tauri::command]
fn get_displays() -> Result<Vec<DisplayInfo>, String> {
    enumerate_displays()
}

#[tauri::command]
fn get_default_process_terms() -> Vec<String> {
    DEFAULT_TERMS
        .iter()
        .map(|term| (*term).to_string())
        .collect()
}

#[tauri::command]
fn get_monitor_state(state: State<'_, DiagnosticState>) -> MonitorState {
    let inner = state.0.lock().unwrap();
    MonitorState {
        running: inner.running,
        display_id: inner.display_id.clone(),
        event_count: inner.events.len(),
        probe_active: inner.probe.is_some(),
    }
}

#[tauri::command]
fn get_events(state: State<'_, DiagnosticState>) -> Vec<DiagnosticEvent> {
    state
        .0
        .lock()
        .map(|inner| inner.events.clone())
        .unwrap_or_default()
}

#[tauri::command]
fn start_monitor(
    app: AppHandle,
    state: State<'_, DiagnosticState>,
    display_id: String,
    process_terms: Vec<String>,
) -> Result<(), String> {
    if display_id.trim().is_empty() {
        return Err("请先选择显示器".into());
    }
    let shared = state.0.clone();
    {
        let mut inner = shared.lock().map_err(|_| "诊断状态锁定失败")?;
        if inner.running {
            return Ok(());
        }
        inner.running = true;
        inner.stop_requested = false;
        inner.display_id = Some(display_id.clone());
        inner.process_terms = if process_terms.is_empty() {
            DEFAULT_TERMS
                .iter()
                .map(|term| (*term).to_string())
                .collect()
        } else {
            process_terms
        };
        inner.previous = None;
    }
    record(
        &shared,
        &app,
        "MONITOR_STARTED",
        format!("开始监视 {}", display_id),
        None,
    );
    let monitor_thread = thread::spawn(move || {
        loop {
            let (stop, terms) = match shared.lock() {
                Ok(inner) => (inner.stop_requested, inner.process_terms.clone()),
                Err(_) => (true, Vec::new()),
            };
            if stop {
                break;
            }
            let current = match snapshot(&display_id, &terms) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    record(&shared, &app, "SNAPSHOT_ERROR", error, None);
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }
            };
            let previous = shared.lock().ok().and_then(|inner| inner.previous.clone());
            if let Some(previous) = previous {
                snapshot_changed(&previous, &current, &shared, &app);
            } else {
                record(
                    &shared,
                    &app,
                    "SNAPSHOT_INITIAL",
                    "记录监视开始状态".into(),
                    Some(current.clone()),
                );
            }
            if let Ok(mut inner) = shared.lock() {
                inner.previous = Some(current);
            }
            thread::sleep(Duration::from_millis(500));
        }
        if let Ok(mut inner) = shared.lock() {
            inner.running = false;
            inner.stop_requested = false;
        }
        record(&shared, &app, "MONITOR_STOPPED", "监视已停止".into(), None);
    });
    if let Ok(mut inner) = state.0.lock() {
        inner.monitor_thread = Some(monitor_thread);
    } else {
        let _ = monitor_thread.join();
        return Err("诊断状态锁定失败".into());
    }
    Ok(())
}

#[tauri::command]
fn stop_monitor(app: AppHandle, state: State<'_, DiagnosticState>) -> Result<(), String> {
    let shared = state.0.clone();
    if request_monitor_stop(&shared) {
        record(
            &shared,
            &app,
            "MONITOR_STOP_REQUESTED",
            "正在停止监视".into(),
            None,
        );
        wait_for_monitor_stop(&shared);
    }
    restore_probe_with_event(&shared, &app).map(|_| ())
}

#[tauri::command]
fn mark_event(
    app: AppHandle,
    state: State<'_, DiagnosticState>,
    label: String,
) -> Result<(), String> {
    let shared = state.0.clone();
    let (display_id, terms) = {
        let inner = shared.lock().map_err(|_| "诊断状态锁定失败")?;
        (inner.display_id.clone(), inner.process_terms.clone())
    };
    let current = display_id
        .as_deref()
        .and_then(|display| snapshot(display, &terms).ok());
    record(&shared, &app, "USER_MARK", label, current);
    Ok(())
}

#[tauri::command]
fn start_probe(app: AppHandle, state: State<'_, DiagnosticState>) -> Result<(), String> {
    let shared = state.0.clone();
    let display_id = {
        let inner = shared.lock().map_err(|_| "诊断状态锁定失败")?;
        if inner.probe.is_some() {
            return Err("已有 Gamma 探针处于激活状态".into());
        }
        inner.display_id.clone().ok_or("请先选择显示器")?
    };
    let (original, error) = read_gamma_ramp(&display_id);
    let original =
        original.ok_or_else(|| error.unwrap_or_else(|| "无法读取原始 Gamma ramp".into()))?;
    let probe = ProbeRamp {
        display_id: display_id.clone(),
        values: original,
    };
    persist_recovery(&probe)?;
    if let Err(error) = write_gamma_ramp(&display_id, &filter_manage_platform::probe_ramp()) {
        clear_recovery_file();
        return Err(error);
    }
    if let Ok(mut inner) = shared.lock() {
        inner.probe = Some(probe);
    }
    record(
        &shared,
        &app,
        "PROBE_WRITE",
        "已写入临时 Gamma 2.4，请复现问题".into(),
        None,
    );
    Ok(())
}

#[tauri::command]
fn restore_probe(app: AppHandle, state: State<'_, DiagnosticState>) -> Result<(), String> {
    let shared = state.0.clone();
    restore_probe_with_event(&shared, &app).map(|_| ())
}

#[tauri::command]
fn export_report(state: State<'_, DiagnosticState>) -> Result<String, String> {
    let inner = state.0.lock().map_err(|_| "诊断状态锁定失败")?;
    let report = DiagnosticReport {
        schema_version: "diagnostic-report-v1",
        generated_at_ms: now_ms(),
        tool: "Filter-Manage Rust Diagnostic",
        os: system_summary(),
        displays: enumerate_displays().unwrap_or_default(),
        display_id: inner.display_id.clone(),
        process_terms: inner.process_terms.clone(),
        events: inner.events.clone(),
        probe_active: inner.probe.is_some(),
    };
    let desktop = dirs::desktop_dir().ok_or("找不到桌面目录")?;
    let path = desktop.join(REPORT_FILENAME);
    let json = serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?;
    std::fs::write(&path, json).map_err(|error| format!("写入报告失败: {}", error))?;
    Ok(path.to_string_lossy().into_owned())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(DiagnosticState::default())
        .invoke_handler(tauri::generate_handler![
            get_displays,
            get_default_process_terms,
            get_monitor_state,
            get_events,
            start_monitor,
            stop_monitor,
            mark_event,
            start_probe,
            restore_probe,
            export_report,
        ])
        .setup(|app| {
            let state = app.state::<DiagnosticState>().inner().clone();
            let app_handle = app.handle().clone();
            match recover_previous_probe() {
                Ok(true) => record(
                    &state.0,
                    &app_handle,
                    "PROBE_RECOVERED",
                    "已从上一次异常退出恢复原始 Gamma ramp".into(),
                    None,
                ),
                Ok(false) => {}
                Err(error) => record(
                    &state.0,
                    &app_handle,
                    "PROBE_RECOVERY_FAILED",
                    format!("上一次 Gamma 恢复失败: {}", error),
                    None,
                ),
            }
            if let Some(window) = app.get_webview_window("diagnostic") {
                window.on_window_event(move |event| {
                    if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                        request_monitor_stop(&state.0);
                        wait_for_monitor_stop(&state.0);
                        let _ = restore_probe_with_event(&state.0, &app_handle);
                    }
                });
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building diagnostic application")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                let state = app.state::<DiagnosticState>().inner().clone();
                let app_handle = app.app_handle().clone();
                request_monitor_stop(&state.0);
                wait_for_monitor_stop(&state.0);
                let _ = restore_probe_with_event(&state.0, &app_handle);
            }
        });
}

#[cfg(test)]
mod tests {
    use filter_manage_platform::{DisplayMode, ProcessInfo};

    use super::*;

    fn test_snapshot(
        mode: DisplayMode,
        ramp: Vec<u16>,
        processes: Vec<ProcessInfo>,
    ) -> DisplaySnapshot {
        DisplaySnapshot {
            timestamp_ms: 1,
            display_id: "\\\\.\\DISPLAY1".into(),
            mode: Some(mode),
            gamma: filter_manage_platform::gamma_state(Some(&ramp), None),
            processes,
            errors: Vec::new(),
        }
    }

    #[test]
    fn snapshot_changes_detects_mode_gamma_and_process_transitions() {
        let previous = test_snapshot(
            DisplayMode {
                width: 2560,
                height: 1440,
                refresh_hz: 280,
                bits_per_pixel: 32,
            },
            filter_manage_platform::identity_ramp(),
            vec![ProcessInfo {
                name: "old-game.exe".into(),
                pid: 10,
            }],
        );
        let current = test_snapshot(
            DisplayMode {
                width: 1920,
                height: 1080,
                refresh_hz: 144,
                bits_per_pixel: 32,
            },
            filter_manage_platform::probe_ramp(),
            vec![ProcessInfo {
                name: "new-game.exe".into(),
                pid: 20,
            }],
        );

        let changes = snapshot_changes(&previous, &current);
        let kinds = changes
            .iter()
            .map(|(kind, _)| kind.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert!(kinds.contains("DISPLAY_MODE_CHANGED"));
        assert!(kinds.contains("GAMMA_RAMP_CHANGED"));
        assert!(kinds.contains("PROCESS_STARTED"));
        assert!(kinds.contains("PROCESS_STOPPED"));
    }

    #[test]
    fn report_serializes_the_v1_contract() {
        let snapshot = test_snapshot(
            DisplayMode {
                width: 2560,
                height: 1440,
                refresh_hz: 280,
                bits_per_pixel: 32,
            },
            filter_manage_platform::identity_ramp(),
            vec![ProcessInfo {
                name: "game.exe".into(),
                pid: 42,
            }],
        );
        let report = DiagnosticReport {
            schema_version: "diagnostic-report-v1",
            generated_at_ms: 1,
            tool: "Filter-Manage Rust Diagnostic",
            os: SystemSummary {
                architecture: "x86_64",
                windows_version: Some("Windows test".into()),
            },
            displays: vec![DisplayInfo {
                id: "\\\\.\\DISPLAY1".into(),
                adapter: "Test adapter".into(),
                pnp_id: "TEST\\MONITOR".into(),
                primary: true,
                mode: snapshot.mode.clone(),
            }],
            display_id: Some(snapshot.display_id.clone()),
            process_terms: vec!["game".into()],
            events: vec![DiagnosticEvent {
                timestamp_ms: 2,
                kind: "SNAPSHOT_INITIAL".into(),
                message: "initial".into(),
                snapshot: Some(snapshot),
            }],
            probe_active: false,
        };

        let value = serde_json::to_value(report).expect("report should serialize");
        assert_eq!(value["schema_version"], "diagnostic-report-v1");
        for field in [
            "generated_at_ms",
            "tool",
            "os",
            "displays",
            "display_id",
            "process_terms",
            "events",
            "probe_active",
        ] {
            assert!(value.get(field).is_some(), "missing report field: {field}");
        }
        assert_eq!(value["events"][0]["kind"], "SNAPSHOT_INITIAL");
        assert_eq!(
            value["events"][0]["snapshot"]["display_id"],
            "\\\\.\\DISPLAY1"
        );
        assert_eq!(value["probe_active"], false);
    }
}
