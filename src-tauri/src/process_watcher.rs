use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex, OnceLock,
};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::config::{self, AppSettings, ProcessRule};
use crate::tray;

#[cfg(windows)]
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};

// ─── 数据类型 ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct RunningProcess {
    pub name: String,
    pub pid: u32,
    pub icon: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct WatcherStatus {
    pub enabled: bool,
    pub active_rule: Option<ProcessRule>,
    pub active_config_name: Option<String>,
    pub subscribed_processes: Vec<String>,
    /// 当前是否持有存活的 WMI 事件通道
    pub wmi_connected: bool,
    /// 最近一次订阅/断开错误；成功订阅后清空
    pub last_error: Option<String>,
    /// 当前退避重连次数；Live 时为 0
    pub reconnect_attempt: u32,
}

struct WatcherState {
    active_rule: Option<ProcessRule>,
    subscribed_processes: Vec<String>,
    wmi_connected: bool,
    last_error: Option<String>,
    reconnect_attempt: u32,
}

// ─── 全局状态 ────────────────────────────────────────────────────────────────

static WATCHER_RUNNING: AtomicBool = AtomicBool::new(false);

fn state() -> &'static Arc<Mutex<WatcherState>> {
    static STATE: OnceLock<Arc<Mutex<WatcherState>>> = OnceLock::new();
    STATE.get_or_init(|| {
        Arc::new(Mutex::new(WatcherState {
            active_rule: None,
            subscribed_processes: Vec::new(),
            wmi_connected: false,
            last_error: None,
            reconnect_attempt: 0,
        }))
    })
}

fn cmd_tx() -> &'static Mutex<Option<mpsc::Sender<WatcherCommand>>> {
    static CMD_TX: OnceLock<Mutex<Option<mpsc::Sender<WatcherCommand>>>> = OnceLock::new();
    CMD_TX.get_or_init(|| Mutex::new(None))
}

fn pw_log(msg: impl AsRef<str>) {
    eprintln!("[process-watcher] {}", msg.as_ref());
}

fn set_health(connected: bool, error: Option<String>, attempt: u32) {
    let mut st = state().lock().unwrap();
    st.wmi_connected = connected;
    st.reconnect_attempt = attempt;
    if let Some(e) = error {
        st.last_error = Some(e);
    } else if connected {
        st.last_error = None;
    }
}

fn next_backoff_secs(prev: u64) -> u64 {
    if prev == 0 {
        1
    } else {
        prev.saturating_mul(2).min(30)
    }
}

enum WatcherCommand {
    Resubscribe,
    Stop,
}

#[derive(Debug, Clone)]
enum ProcessEvent {
    Started(String),
    Stopped(String),
}

// ─── WQL 构建 ────────────────────────────────────────────────────────────────

fn wql_escape(name: &str) -> String {
    name.replace('\'', "\\'")
}

fn build_wql(rules: &[ProcessRule]) -> Option<String> {
    let enabled: Vec<&str> = rules
        .iter()
        .filter(|r| r.enabled)
        .map(|r| r.process_name.as_str())
        .collect();

    if enabled.is_empty() {
        return None;
    }

    let name_filter = enabled
        .iter()
        .map(|n| format!("TargetInstance.Name = '{}'", wql_escape(n)))
        .collect::<Vec<_>>()
        .join(" OR ");

    Some(format!(
        "SELECT * FROM __InstanceOperationEvent WITHIN 1 \
         WHERE TargetInstance ISA 'Win32_Process' AND ({})",
        name_filter
    ))
}

fn subscribed_names(rules: &[ProcessRule]) -> Vec<String> {
    rules
        .iter()
        .filter(|r| r.enabled)
        .map(|r| r.process_name.clone())
        .collect()
}

/// `save_app_settings` 整包写入时，若进程监听相关字段变化则重订 WMI。
pub fn resubscribe_if_process_settings_changed(old: &AppSettings, new: &AppSettings) {
    if old.process_watcher_enabled != new.process_watcher_enabled
        || old.process_rules != new.process_rules
    {
        pw_log("process settings changed via save_app_settings → resubscribe");
        send_resubscribe();
    }
}

// ─── 进程图标提取 ─────────────────────────────────────────────────────────────

#[cfg(windows)]
fn get_process_exe_path(pid: u32) -> Option<String> {
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT::default(),
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut len,
        );
        let _ = windows::Win32::Foundation::CloseHandle(handle);
        if result.is_ok() {
            Some(String::from_utf16_lossy(&buf[..len as usize]))
        } else {
            None
        }
    }
}

#[cfg(windows)]
fn extract_icon_base64(exe_path: &str) -> Option<String> {
    use base64::Engine;
    use windows::core::HSTRING;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP,
    };
    use windows::Win32::UI::Shell::ExtractIconExW;
    use windows::Win32::UI::WindowsAndMessaging::{
        DestroyIcon, GetIconInfo, ICONINFO,
    };

    unsafe {
        let hpath = HSTRING::from(exe_path);
        let mut icon = std::mem::zeroed();
        let count = ExtractIconExW(&hpath, 0, Some(&mut icon), None, 1);
        if count == 0 || icon.is_invalid() {
            return None;
        }

        let mut ii = ICONINFO::default();
        if GetIconInfo(icon, &mut ii).is_err() {
            let _ = DestroyIcon(icon);
            return None;
        }

        let hdc = CreateCompatibleDC(None);
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                biPlanes: 1,
                ..Default::default()
            },
            ..Default::default()
        };

        let hbitmap = ii.hbmColor;
        let mut bmp = BITMAP::default();
        let bmp_size = std::mem::size_of::<BITMAP>() as i32;
        windows::Win32::Graphics::Gdi::GetObjectW(hbitmap, bmp_size, Some(&mut bmp as *mut _ as *mut _));

        let width = bmp.bmWidth;
        let height = bmp.bmHeight.abs();
        bmi.bmiHeader.biWidth = width;
        bmi.bmiHeader.biHeight = -height;

        let row_size = (width * 4) as usize;
        let mut pixels = vec![0u8; row_size * height as usize];
        GetDIBits(
            hdc,
            hbitmap,
            0,
            height as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        // Read AND mask for transparency
        let mask_hbitmap = ii.hbmMask;
        let mut mask_bmp = BITMAP::default();
        windows::Win32::Graphics::Gdi::GetObjectW(
            mask_hbitmap,
            bmp_size,
            Some(&mut mask_bmp as *mut _ as *mut _),
        );
        let mask_width = mask_bmp.bmWidth;
        let mask_height = mask_bmp.bmHeight.abs();
        let is_separate_mask = mask_height != height || ii.hbmColor.is_invalid();

        let mut mask_data = Vec::new();
        if is_separate_mask {
            let mask_row = ((mask_width + 31) / 32 * 4) as usize;
            mask_data = vec![0u8; mask_row * mask_height as usize];
            bmi.bmiHeader.biBitCount = 1;
            bmi.bmiHeader.biWidth = mask_width;
            bmi.bmiHeader.biHeight = -mask_height;
            GetDIBits(
                hdc,
                mask_hbitmap,
                0,
                mask_height as u32,
                Some(mask_data.as_mut_ptr() as *mut _),
                &mut bmi,
                DIB_RGB_COLORS,
            );
        }

        let _ = DeleteDC(hdc);
        let _ = DeleteObject(hbitmap);
        let _ = DeleteObject(mask_hbitmap);
        let _ = DestroyIcon(icon);

        // Convert BGRA → RGBA with transparency
        let mut rgba = vec![0u8; (width * height * 4) as usize];
        for y in 0..height as usize {
            for x in 0..width as usize {
                let si = y * row_size + x * 4;
                let di = (y * width as usize + x) * 4;
                rgba[di] = pixels[si + 2];     // R
                rgba[di + 1] = pixels[si + 1]; // G
                rgba[di + 2] = pixels[si];     // B

                if is_separate_mask && (y as i32) < mask_height && (x as i32) < mask_width {
                    let mask_row = ((mask_width + 31) / 32 * 4) as usize;
                    let byte_idx = y * mask_row + (x / 8);
                    let bit = 7 - (x % 8);
                    if byte_idx < mask_data.len() && (mask_data[byte_idx] >> bit) & 1 == 1 {
                        rgba[di + 3] = 0;
                    } else {
                        rgba[di + 3] = pixels[si + 3];
                    }
                } else {
                    rgba[di + 3] = pixels[si + 3];
                }
            }
        }

        // Encode PNG
        let mut buf = Vec::new();
        {
            let mut encoder =
                png::Encoder::new(&mut buf, width as u32, height as u32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().ok()?;
            writer.write_image_data(&rgba).ok()?;
        }

        Some(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&buf)
        ))
    }
}

// ─── 进程枚举（仅按需快照）────────────────────────────────────────────────────

#[cfg(windows)]
fn list_running_processes() -> Vec<RunningProcess> {
    let mut processes = Vec::new();

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    let Ok(snapshot) = snapshot else {
        return processes;
    };

    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };

    let mut icon_cache: HashMap<String, Option<String>> = HashMap::new();

    unsafe {
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                let pid = entry.th32ProcessID;

                let icon = icon_cache
                    .entry(name.clone())
                    .or_insert_with(|| {
                        get_process_exe_path(pid)
                            .and_then(|path| extract_icon_base64(&path))
                    })
                    .clone();

                processes.push(RunningProcess { name, pid, icon });
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = windows::Win32::Foundation::CloseHandle(snapshot);
    }

    processes
}

#[cfg(not(windows))]
fn list_running_processes() -> Vec<RunningProcess> {
    Vec::new()
}

// ─── 轻量进程名枚举（对账用，不提取图标）──────────────────────────────────────

#[cfg(windows)]
fn running_process_names() -> Vec<String> {
    let mut names = Vec::new();

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    let Ok(snapshot) = snapshot else {
        return names;
    };

    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };

    unsafe {
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                names.push(String::from_utf16_lossy(&entry.szExeFile[..len]));
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = windows::Win32::Foundation::CloseHandle(snapshot);
    }

    names
}

#[cfg(not(windows))]
fn running_process_names() -> Vec<String> {
    Vec::new()
}

// ─── Toast 通知 ──────────────────────────────────────────────────────────────

#[cfg(windows)]
fn show_toast(body: &str) {
    use std::sync::atomic::AtomicU32;
    use windows::{
        core::HSTRING,
        Data::Xml::Dom::XmlDocument,
        UI::Notifications::{ToastNotification, ToastNotificationManager},
    };

    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let idx = COUNTER.fetch_add(1, Ordering::Relaxed);
    let new_tag = format!("fm-proc-{}", idx);
    let app_id = HSTRING::from("com.filter-manage.app");

    if idx > 0 {
        let old_tag = HSTRING::from(format!("fm-proc-{}", idx - 1));
        if let Ok(history) = ToastNotificationManager::History() {
            let _ = history.Remove(&old_tag);
        }
    }

    let xml = XmlDocument::new().unwrap();
    xml.LoadXml(&HSTRING::from(format!(
        r#"<toast><visual><binding template="ToastGeneric"><text>Filter Manage</text><text>{}</text></binding></visual></toast>"#,
        body
    )))
    .unwrap();

    if let Ok(toast) = ToastNotification::CreateToastNotification(&xml) {
        let _ = toast.SetTag(&HSTRING::from(new_tag));
        if let Ok(notifier) = ToastNotificationManager::CreateToastNotifierWithId(&app_id) {
            let _ = notifier.Show(&toast);
        }
    }
}

#[cfg(not(windows))]
fn show_toast(_body: &str) {}

// ─── WMI 监听线程 ────────────────────────────────────────────────────────────

#[cfg(windows)]
mod wmi_impl {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;

    use windows::core::*;
    use windows::Win32::System::Com::*;
    use windows::Win32::System::Wmi::*;

    use super::ProcessEvent;

    // CoInitializeSecurity is in Win32::System::Ole in windows 0.58,
    // declare it via raw FFI to avoid adding another feature
    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeSecurity(
            psd: *const std::ffi::c_void,
            cauthsvcs: i32,
            asauthsvcs: *const std::ffi::c_void,
            preserved: *const std::ffi::c_void,
            dwauthnlevel: u32,
            dwimplevel: u32,
            pauthlist: *const std::ffi::c_void,
            dwcapabilities: u32,
            reserved: *const std::ffi::c_void,
        ) -> HRESULT;
    }

    #[implement(IWbemObjectSink)]
    pub struct EventSink {
        tx: mpsc::Sender<ProcessEvent>,
        stop: AtomicBool,
    }

    impl EventSink {
        pub fn new(tx: mpsc::Sender<ProcessEvent>) -> Self {
            Self {
                tx,
                stop: AtomicBool::new(false),
            }
        }

        pub fn stop(&self) {
            self.stop.store(true, Ordering::Relaxed);
        }
    }

    impl IWbemObjectSink_Impl for EventSink_Impl {
        fn Indicate(
            &self,
            lObjectCount: i32,
            apObjArray: *const Option<IWbemClassObject>,
        ) -> windows::core::Result<()> {
            if self.stop.load(Ordering::Relaxed) {
                return Err(Error::from(windows::Win32::Foundation::E_FAIL));
            }

            let count = lObjectCount as usize;
            for i in 0..count {
                let obj = unsafe { &*apObjArray.add(i) };
                let Some(class_obj) = obj else { continue };

                // Get __Class property via VARIANT output parameter
                let mut class_var = VARIANT::default();
                let class_name = unsafe {
                    class_obj
                        .Get(w!("__Class"), 0, &mut class_var, None, None)
                        .and_then(|_| BSTR::try_from(&class_var))
                        .map(|b| b.to_string())
                        .unwrap_or_default()
                };

                // Get TargetInstance.Name
                let mut target_var = VARIANT::default();
                let proc_name = unsafe {
                    class_obj
                        .Get(w!("TargetInstance"), 0, &mut target_var, None, None)
                        .ok()
                        .and_then(|_| {
                            // TargetInstance is VT_UNKNOWN → extract IUnknown via TryFrom
                            let iunk: std::result::Result<IUnknown, _> =
                                IUnknown::try_from(&target_var);
                            iunk.ok().and_then(|iunk| {
                                iunk.cast::<IWbemClassObject>().ok().and_then(|inner_obj| {
                                    let mut name_var = VARIANT::default();
                                    inner_obj
                                        .Get(w!("Name"), 0, &mut name_var, None, None)
                                        .ok()
                                        .and_then(|_| BSTR::try_from(&name_var).ok())
                                        .map(|b| b.to_string())
                                })
                            })
                        })
                };

                let Some(name) = proc_name else { continue };

                let evt = if class_name.contains("Creation") {
                    ProcessEvent::Started(name)
                } else if class_name.contains("Deletion") {
                    ProcessEvent::Stopped(name)
                } else {
                    continue;
                };

                if self.tx.send(evt).is_err() {
                    self.stop.store(true, Ordering::Relaxed);
                    return Err(Error::from(windows::Win32::Foundation::E_FAIL));
                }
            }

            Ok(())
        }

        fn SetStatus(
            &self,
            _lFlags: i32,
            _hResult: HRESULT,
            _strParam: &BSTR,
            _pObjParam: Option<&IWbemClassObject>,
        ) -> windows::core::Result<()> {
            Ok(())
        }
    }

    pub struct WmiSubscription {
        svc: IWbemServices,
        sink_interface: IWbemObjectSink,
    }

    impl WmiSubscription {
        pub fn new(wql: &str, event_tx: mpsc::Sender<ProcessEvent>) -> std::result::Result<Self, String> {
            unsafe {
                CoInitializeEx(None, COINIT_MULTITHREADED)
                    .ok()
                    .map_err(|e| format!("CoInitializeEx: {:?}", e))?;

                // CoInitializeSecurity may fail if already initialized — ignore
                let _ = CoInitializeSecurity(
                    std::ptr::null(),
                    -1,
                    std::ptr::null(),
                    std::ptr::null(),
                    1,  // RPC_C_AUTHN_LEVEL_CONNECT
                    3,  // RPC_C_IMP_LEVEL_IMPERSONATE
                    std::ptr::null(),
                    0,  // EOAC_NONE
                    std::ptr::null(),
                );

                let loc: IWbemLocator =
                    CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)
                        .map_err(|e| format!("CoCreateInstance: {:?}", e))?;

                let svc = loc
                    .ConnectServer(
                        &BSTR::from("ROOT\\CIMV2"),
                        None,
                        None,
                        None,
                        0,
                        None,
                        None,
                    )
                    .map_err(|e| format!("ConnectServer: {:?}", e))?;

                let _ = CoSetProxyBlanket(
                    &svc,
                    10,  // RPC_C_AUTHN_WINNT
                    0,   // RPC_C_AUTHZ_NONE
                    None,
                    RPC_C_AUTHN_LEVEL(3),   // RPC_C_AUTHN_LEVEL_CALL
                    RPC_C_IMP_LEVEL(3),     // RPC_C_IMP_LEVEL_IMPERSONATE
                    None,
                    EOAC_NONE,
                );

                let sink = EventSink::new(event_tx);
                let sink_interface: IWbemObjectSink = sink.into();

                svc.ExecNotificationQueryAsync(
                    &BSTR::from("WQL"),
                    &BSTR::from(wql),
                    WBEM_FLAG_SEND_STATUS,
                    None,
                    &sink_interface,
                )
                .map_err(|e| format!("ExecNotificationQueryAsync: {:?}", e))?;

                Ok(Self {
                    svc,
                    sink_interface,
                })
            }
        }

        pub fn cancel(&self) {
            let _ = unsafe { self.svc.CancelAsyncCall(&self.sink_interface) };
        }
    }

    impl Drop for WmiSubscription {
        fn drop(&mut self) {
            self.cancel();
        }
    }
}

#[cfg(windows)]
fn spawn_wmi_monitor(
    event_tx: mpsc::Sender<ProcessEvent>,
    wql: String,
    stop_rx: mpsc::Receiver<()>,
) -> Option<thread::JoinHandle<()>> {
    thread::Builder::new()
        .name("wmi-monitor".into())
        .spawn(move || {
            let sub = match wmi_impl::WmiSubscription::new(&wql, event_tx) {
                Ok(s) => s,
                Err(e) => {
                    pw_log(format!("WMI subscribe failed: {e}"));
                    return;
                }
            };

            // Block until stop signal arrives (subscription stays alive)
            let _ = stop_rx.recv();
            drop(sub);
        })
        .ok()
}

#[cfg(not(windows))]
fn spawn_wmi_monitor(
    _event_tx: mpsc::Sender<ProcessEvent>,
    _wql: String,
    _stop_rx: mpsc::Receiver<()>,
) -> Option<thread::JoinHandle<()>> {
    None
}

// ─── 事件处理 ────────────────────────────────────────────────────────────────

/// 清除 active_rule；若 restore 为 true 则恢复默认方案。调用前不得持有 state 锁。
fn clear_active_rule(app: &AppHandle, restore: bool, notify: bool, reason: &str) {
    {
        let mut st = state().lock().unwrap();
        if st.active_rule.is_none() {
            return;
        }
        st.active_rule = None;
    }
    pw_log(format!("clear active_rule ({reason}) restore={restore}"));
    if restore {
        if let Err(e) = tray::apply_default_config() {
            pw_log(format!("apply_default_config failed: {e}"));
        } else {
            let _ = app.emit("config-applied", "__default__");
            if notify {
                show_toast("进程退出：已恢复默认方案");
            }
        }
    }
}

fn handle_event(event: ProcessEvent, app: &AppHandle) {
    let settings = match config::get_app_settings() {
        Ok(s) => s,
        Err(e) => {
            pw_log(format!("get_app_settings failed: {e}"));
            return;
        }
    };

    if !settings.process_watcher_enabled {
        return;
    }

    match event {
        ProcessEvent::Started(name) => {
            pw_log(format!("event Started name={name}"));
            let rule = {
                let st = state().lock().unwrap();
                settings
                    .process_rules
                    .iter()
                    .find(|r| r.enabled && r.process_name.eq_ignore_ascii_case(&name))
                    .cloned()
                    .filter(|rule| st.active_rule.as_ref().map(|r| &r.id) != Some(&rule.id))
            };
            let Some(rule) = rule else {
                return;
            };

            let config_name = rule.config_name.clone();
            let notify = settings.process_notification;

            match config::load_config(config_name.clone()) {
                Ok(cfg) => match tray::apply_color_config(&cfg) {
                    Ok(()) => {
                        // 先登记 active_rule，再 emit，避免前端 get_watcher_status 与事件竞态
                        state().lock().unwrap().active_rule = Some(rule);
                        pw_log(format!("active_rule set config={config_name}"));
                        let _ = app.emit("config-applied", &config_name);
                        if notify {
                            show_toast(&format!("进程触发：已切换到「{}」", config_name));
                        }
                    }
                    Err(e) => {
                        pw_log(format!("apply_color_config failed: {e}"));
                    }
                },
                Err(e) => {
                    pw_log(format!("load_config failed for '{config_name}': {e}"));
                }
            }
        }
        ProcessEvent::Stopped(name) => {
            pw_log(format!("event Stopped name={name}"));

            // 同名多实例：仍有进程在跑则忽略本次退出
            let still = running_process_names()
                .iter()
                .any(|p| p.eq_ignore_ascii_case(&name));
            if still {
                pw_log(format!(
                    "Stopped ignored, instance still running: {name}"
                ));
                return;
            }

            let (should_clear, restore, notify) = {
                let st = state().lock().unwrap();
                match &st.active_rule {
                    Some(active) if active.process_name.eq_ignore_ascii_case(&name) => {
                        (true, active.restore_on_exit, settings.process_notification)
                    }
                    _ => (false, false, false),
                }
            };

            if should_clear {
                clear_active_rule(app, restore, notify, "process stopped");
            }
        }
    }
}

// ─── 订阅后对账 ──────────────────────────────────────────────────────────────

/// WMI 的 `__InstanceOperationEvent` 只上报订阅之后发生的启动/退出事件，
/// 订阅前就已在运行的进程不会补发「启动」事件。
///
/// 在（重新）订阅成功后调用一次（非周期轮询）：
/// - 校正脏 active_rule（规则失效 / 进程已退出）
/// - 对仍在运行的首个匹配规则补 Started
fn reconcile_running_processes(app: &AppHandle) {
    let settings = match config::get_app_settings() {
        Ok(s) => s,
        Err(e) => {
            pw_log(format!("reconcile: get_app_settings failed: {e}"));
            return;
        }
    };
    if !settings.process_watcher_enabled {
        pw_log("reconcile skip: watcher disabled");
        return;
    }

    let running = running_process_names();

    // A. 校正已有 active_rule（释放锁后再调 handle_event / clear）
    let active_snapshot = state().lock().unwrap().active_rule.clone();
    if let Some(active) = active_snapshot {
        let rule_still_enabled = settings
            .process_rules
            .iter()
            .any(|r| r.enabled && r.id == active.id);
        if !rule_still_enabled {
            clear_active_rule(
                app,
                active.restore_on_exit,
                settings.process_notification,
                "rule disabled or removed",
            );
        } else {
            let proc_running = running
                .iter()
                .any(|p| p.eq_ignore_ascii_case(&active.process_name));
            if !proc_running {
                pw_log(format!(
                    "reconcile: active process gone → Stopped {}",
                    active.process_name
                ));
                handle_event(ProcessEvent::Stopped(active.process_name.clone()), app);
            } else {
                pw_log("reconcile skip: active_rule still valid");
                return;
            }
        }
    }

    // B. active 为空时补第一个正在运行的规则进程
    if state().lock().unwrap().active_rule.is_some() {
        return;
    }

    if let Some(name) = settings
        .process_rules
        .iter()
        .filter(|r| r.enabled)
        .find_map(|r| {
            running
                .iter()
                .find(|p| p.eq_ignore_ascii_case(&r.process_name))
                .cloned()
        })
    {
        pw_log(format!("reconcile Started name={name}"));
        handle_event(ProcessEvent::Started(name), app);
    } else {
        pw_log("reconcile skip: no matching running process");
    }
}

// ─── 订阅辅助 ────────────────────────────────────────────────────────────────

type SubscribeParts = (
    Option<thread::JoinHandle<()>>,
    mpsc::Receiver<ProcessEvent>,
    mpsc::Sender<()>,
);

fn stop_subscription(
    monitor_handle: &mut Option<thread::JoinHandle<()>>,
    event_rx: &mut Option<mpsc::Receiver<ProcessEvent>>,
    stop_tx: &mut Option<mpsc::Sender<()>>,
) {
    if let Some(tx) = stop_tx.take() {
        let _ = tx.send(());
    }
    drop(event_rx.take());
    if let Some(handle) = monitor_handle.take() {
        // 短暂等待线程退出，避免与新订阅重叠过久
        let _ = handle.join();
    }
    set_health(false, None, state().lock().unwrap().reconnect_attempt);
}

fn try_start_subscription(settings: &AppSettings) -> Option<SubscribeParts> {
    if !settings.process_watcher_enabled {
        return None;
    }
    let wql = build_wql(&settings.process_rules)?;
    let names = subscribed_names(&settings.process_rules);
    state().lock().unwrap().subscribed_processes = names.clone();

    let (etx, erx) = mpsc::channel();
    let (stx, srx) = mpsc::channel();
    let handle = spawn_wmi_monitor(etx, wql, srx);
    if handle.is_none() {
        pw_log("WMI subscribe failed: spawn wmi-monitor thread failed");
        set_health(
            false,
            Some("failed to spawn wmi-monitor thread".into()),
            state().lock().unwrap().reconnect_attempt,
        );
        return None;
    }

    pw_log(format!("WMI subscribed: names={names:?}"));
    set_health(true, None, 0);
    Some((handle, erx, stx))
}

/// 关监听或无规则时：停 WMI、清订阅名、按需 restore active。
fn teardown_subscription(
    app: &AppHandle,
    monitor_handle: &mut Option<thread::JoinHandle<()>>,
    event_rx: &mut Option<mpsc::Receiver<ProcessEvent>>,
    stop_tx: &mut Option<mpsc::Sender<()>>,
    restore_active: bool,
) {
    stop_subscription(monitor_handle, event_rx, stop_tx);
    state().lock().unwrap().subscribed_processes.clear();
    set_health(false, None, 0);

    if restore_active {
        let (restore, notify) = {
            let st = state().lock().unwrap();
            match &st.active_rule {
                Some(a) => (a.restore_on_exit, true),
                None => (false, false),
            }
        };
        // notify 用 settings
        let notify = config::get_app_settings()
            .map(|s| s.process_notification)
            .unwrap_or(true)
            && notify;
        if state().lock().unwrap().active_rule.is_some() {
            clear_active_rule(app, restore, notify, "watcher disabled or no rules");
        }
    }
}

// ─── 主 watcher 线程 ─────────────────────────────────────────────────────────

pub fn init_watcher(app: &AppHandle) {
    if WATCHER_RUNNING.load(Ordering::Relaxed) {
        return;
    }
    WATCHER_RUNNING.store(true, Ordering::Relaxed);

    let (cmd_sender, cmd_receiver) = mpsc::channel::<WatcherCommand>();
    cmd_tx().lock().unwrap().replace(cmd_sender);

    let app_handle = app.clone();

    thread::Builder::new()
        .name("process-watcher".into())
        .spawn(move || {
            let mut monitor_handle: Option<thread::JoinHandle<()>> = None;
            let mut event_rx: Option<mpsc::Receiver<ProcessEvent>> = None;
            let mut stop_tx: Option<mpsc::Sender<()>> = None;
            let mut backoff_secs: u64 = 0;
            let mut next_retry_at: Option<Instant> = None;

            // 初始订阅
            let settings = config::get_app_settings().unwrap_or_default();
            if let Some((h, erx, stx)) = try_start_subscription(&settings) {
                monitor_handle = h;
                event_rx = Some(erx);
                stop_tx = Some(stx);
                reconcile_running_processes(&app_handle);
            } else if settings.process_watcher_enabled
                && build_wql(&settings.process_rules).is_some()
            {
                backoff_secs = next_backoff_secs(0);
                next_retry_at = Some(Instant::now() + Duration::from_secs(backoff_secs));
                set_health(
                    false,
                    Some("initial subscribe failed".into()),
                    1,
                );
                pw_log(format!("reconnect in {backoff_secs}s"));
            } else {
                pw_log("idle: watcher disabled or no enabled rules");
            }

            loop {
                match cmd_receiver.try_recv() {
                    Ok(WatcherCommand::Resubscribe) => {
                        pw_log("command Resubscribe");
                        stop_subscription(
                            &mut monitor_handle,
                            &mut event_rx,
                            &mut stop_tx,
                        );
                        backoff_secs = 0;
                        next_retry_at = None;

                        let settings = config::get_app_settings().unwrap_or_default();
                        if settings.process_watcher_enabled
                            && build_wql(&settings.process_rules).is_some()
                        {
                            if let Some((h, erx, stx)) = try_start_subscription(&settings) {
                                monitor_handle = h;
                                event_rx = Some(erx);
                                stop_tx = Some(stx);
                                reconcile_running_processes(&app_handle);
                            } else {
                                backoff_secs = next_backoff_secs(0);
                                next_retry_at =
                                    Some(Instant::now() + Duration::from_secs(backoff_secs));
                                set_health(
                                    false,
                                    Some("resubscribe failed".into()),
                                    1,
                                );
                                pw_log(format!("reconnect in {backoff_secs}s"));
                            }
                        } else {
                            // 关监听或无规则：清 active
                            teardown_subscription(
                                &app_handle,
                                &mut monitor_handle,
                                &mut event_rx,
                                &mut stop_tx,
                                true,
                            );
                            pw_log("idle after resubscribe: disabled or no rules");
                        }
                    }
                    Ok(WatcherCommand::Stop) => {
                        pw_log("command Stop");
                        stop_subscription(
                            &mut monitor_handle,
                            &mut event_rx,
                            &mut stop_tx,
                        );
                        WATCHER_RUNNING.store(false, Ordering::Relaxed);
                        set_health(false, None, 0);
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        pw_log("cmd channel disconnected, exiting watcher");
                        break;
                    }
                }

                if let Some(ref rx) = event_rx {
                    match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(event) => handle_event(event, &app_handle),
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            pw_log("WMI event channel disconnected, will reconnect");
                            stop_subscription(
                                &mut monitor_handle,
                                &mut event_rx,
                                &mut stop_tx,
                            );
                            let settings = config::get_app_settings().unwrap_or_default();
                            if settings.process_watcher_enabled
                                && build_wql(&settings.process_rules).is_some()
                            {
                                backoff_secs = next_backoff_secs(backoff_secs);
                                next_retry_at =
                                    Some(Instant::now() + Duration::from_secs(backoff_secs));
                                let attempt = {
                                    let n = state().lock().unwrap().reconnect_attempt.saturating_add(1);
                                    n.max(1)
                                };
                                set_health(
                                    false,
                                    Some("event channel disconnected".into()),
                                    attempt,
                                );
                                pw_log(format!("reconnect in {backoff_secs}s"));
                            } else {
                                set_health(false, None, 0);
                                backoff_secs = 0;
                                next_retry_at = None;
                            }
                        }
                    }
                } else {
                    // 无活跃订阅：若应监听则按退避重订
                    let settings = config::get_app_settings().unwrap_or_default();
                    let should_subscribe = settings.process_watcher_enabled
                        && build_wql(&settings.process_rules).is_some();

                    if should_subscribe {
                        let due = next_retry_at
                            .map(|t| Instant::now() >= t)
                            .unwrap_or(true);
                        if due {
                            if let Some((h, erx, stx)) = try_start_subscription(&settings) {
                                monitor_handle = h;
                                event_rx = Some(erx);
                                stop_tx = Some(stx);
                                backoff_secs = 0;
                                next_retry_at = None;
                                reconcile_running_processes(&app_handle);
                            } else {
                                backoff_secs = next_backoff_secs(backoff_secs);
                                next_retry_at =
                                    Some(Instant::now() + Duration::from_secs(backoff_secs));
                                let attempt =
                                    state().lock().unwrap().reconnect_attempt.saturating_add(1);
                                set_health(
                                    false,
                                    Some("subscribe failed".into()),
                                    attempt.max(1),
                                );
                                pw_log(format!("reconnect in {backoff_secs}s"));
                                thread::sleep(Duration::from_millis(200));
                            }
                        } else {
                            thread::sleep(Duration::from_millis(200));
                        }
                    } else {
                        thread::sleep(Duration::from_millis(500));
                    }
                }
            }
        })
        .expect("Failed to spawn process watcher thread");
}

pub fn stop_watcher() {
    if let Some(tx) = cmd_tx().lock().unwrap().as_ref() {
        let _ = tx.send(WatcherCommand::Stop);
    }
    WATCHER_RUNNING.store(false, Ordering::Relaxed);
}

fn send_resubscribe() {
    if let Some(tx) = cmd_tx().lock().unwrap().as_ref() {
        let _ = tx.send(WatcherCommand::Resubscribe);
    }
}

// ─── Tauri 命令 ──────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_process_rules() -> Result<Vec<ProcessRule>, String> {
    let settings = config::get_app_settings()?;
    Ok(settings.process_rules)
}

#[tauri::command]
pub fn add_process_rule(rule: ProcessRule) -> Result<(), String> {
    if rule.process_name.trim().is_empty() {
        return Err("进程名不能为空".into());
    }
    config::load_config(rule.config_name.clone())?;

    let mut settings = config::get_app_settings()?;

    let lower = rule.process_name.to_lowercase();
    if settings
        .process_rules
        .iter()
        .any(|r| r.process_name.to_lowercase() == lower)
    {
        return Err(format!("进程 '{}' 已存在规则", rule.process_name));
    }

    settings.process_rules.push(rule);
    config::save_app_settings(settings)?;
    send_resubscribe();
    Ok(())
}

#[tauri::command]
pub fn update_process_rule(rule: ProcessRule) -> Result<(), String> {
    if rule.process_name.trim().is_empty() {
        return Err("进程名不能为空".into());
    }

    let mut settings = config::get_app_settings()?;
    if let Some(existing) = settings.process_rules.iter_mut().find(|r| r.id == rule.id) {
        *existing = rule;
        config::save_app_settings(settings)?;
        send_resubscribe();
        Ok(())
    } else {
        Err(format!("规则 '{}' 不存在", rule.id))
    }
}

#[tauri::command]
pub fn delete_process_rule(id: String) -> Result<(), String> {
    let mut settings = config::get_app_settings()?;
    let before = settings.process_rules.len();
    settings.process_rules.retain(|r| r.id != id);
    if settings.process_rules.len() == before {
        return Err(format!("规则 '{}' 不存在", id));
    }
    config::save_app_settings(settings)?;
    send_resubscribe();
    Ok(())
}

#[tauri::command]
pub fn get_running_processes() -> Result<Vec<RunningProcess>, String> {
    Ok(list_running_processes())
}

#[tauri::command]
pub fn set_process_watcher_enabled(enabled: bool) -> Result<(), String> {
    let mut settings = config::get_app_settings()?;
    settings.process_watcher_enabled = enabled;
    config::save_app_settings(settings)?;
    send_resubscribe();
    Ok(())
}

#[tauri::command]
pub fn get_watcher_status() -> Result<WatcherStatus, String> {
    let settings = config::get_app_settings()?;
    let st = state().lock().unwrap();
    Ok(WatcherStatus {
        enabled: settings.process_watcher_enabled,
        active_rule: st.active_rule.clone(),
        active_config_name: st.active_rule.as_ref().map(|r| r.config_name.clone()),
        subscribed_processes: st.subscribed_processes.clone(),
        wmi_connected: st.wmi_connected,
        last_error: st.last_error.clone(),
        reconnect_attempt: st.reconnect_attempt,
    })
}
