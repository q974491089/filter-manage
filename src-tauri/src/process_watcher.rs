use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex, OnceLock,
};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::config::{self, ProcessRule};
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
}

struct WatcherState {
    active_rule: Option<ProcessRule>,
    previous_config_name: Option<String>,
    subscribed_processes: Vec<String>,
}

// ─── 全局状态 ────────────────────────────────────────────────────────────────

static WATCHER_RUNNING: AtomicBool = AtomicBool::new(false);

fn state() -> &'static Arc<Mutex<WatcherState>> {
    static STATE: OnceLock<Arc<Mutex<WatcherState>>> = OnceLock::new();
    STATE.get_or_init(|| {
        Arc::new(Mutex::new(WatcherState {
            active_rule: None,
            previous_config_name: None,
            subscribed_processes: Vec::new(),
        }))
    })
}

fn cmd_tx() -> &'static Mutex<Option<mpsc::Sender<WatcherCommand>>> {
    static CMD_TX: OnceLock<Mutex<Option<mpsc::Sender<WatcherCommand>>>> = OnceLock::new();
    CMD_TX.get_or_init(|| Mutex::new(None))
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
        .map(|n| format!("TargetInstance.Name = '{}'", n))
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
                    eprintln!("WMI: {}", e);
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

fn handle_event(event: ProcessEvent, app: &AppHandle) {
    let settings = match config::get_app_settings() {
        Ok(s) => s,
        Err(_) => return,
    };

    if !settings.process_watcher_enabled {
        return;
    }

    let mut st = state().lock().unwrap();

    match event {
        ProcessEvent::Started(name) => {
            if let Some(rule) = settings
                .process_rules
                .iter()
                .find(|r| r.enabled && r.process_name.eq_ignore_ascii_case(&name))
            {
                if st.active_rule.as_ref().map(|r| &r.id) == Some(&rule.id) {
                    return;
                }

                if st.active_rule.is_none() {
                    st.previous_config_name = None;
                }

                let config_name = rule.config_name.clone();
                let notify = settings.process_notification;
                let rule = rule.clone();

                match config::load_config(config_name.clone()) {
                    Ok(cfg) => {
                        let _ = tray::apply_color_config(&cfg);
                        let _ = app.emit("config-applied", &config_name);
                        if notify {
                            show_toast(&format!("进程触发：已切换到「{}」", config_name));
                        }
                    }
                    Err(_) => {}
                }

                st.active_rule = Some(rule);
            }
        }
        ProcessEvent::Stopped(name) => {
            if let Some(active) = &st.active_rule {
                if active.process_name.eq_ignore_ascii_case(&name) {
                    let restore = active.restore_on_exit;
                    let notify = settings.process_notification;

                    if restore {
                        if let Some(ref prev_name) = st.previous_config_name {
                            if let Ok(cfg) = config::load_config(prev_name.clone()) {
                                let _ = tray::apply_color_config(&cfg);
                                let _ = app.emit("config-applied", prev_name);
                                if notify {
                                    show_toast(&format!("进程退出：已恢复到「{}」", prev_name));
                                }
                            }
                        } else {
                            let _ = tray::apply_default_config();
                            let _ = app.emit("config-applied", "__default__");
                            if notify {
                                show_toast("进程退出：已恢复默认方案");
                            }
                        }
                    }

                    st.active_rule = None;
                    st.previous_config_name = None;
                }
            }
        }
    }
}

// ─── 订阅后对账 ──────────────────────────────────────────────────────────────

/// WMI 的 `__InstanceOperationEvent` 只上报订阅之后发生的启动/退出事件，
/// 订阅前就已在运行的进程不会补发「启动」事件。这会导致：用户先开游戏、
/// 后开本应用（或游戏在应用启动前已运行）时，`active_rule` 一直是 None，
/// 游戏退出时的 Deletion 事件被 `handle_event` 的 `if let Some(active)` 挡掉，
/// 从而不会恢复方案。
///
/// 因此在（重新）订阅成功后，主动扫描一次当前进程，对第一个正在运行的受监听
/// 进程合成一次 `Started` 事件，复用 `handle_event` 完成登记 + 应用配色。
fn reconcile_running_processes(app: &AppHandle) {
    // active_rule 已存在则无需对账（避免覆盖 / 重复应用）
    // 显式作用域：确保锁守卫在调用 handle_event（内部会再次锁 state）前释放，
    // 否则同线程重入 std Mutex 会死锁。
    {
        if state().lock().unwrap().active_rule.is_some() {
            return;
        }
    }

    let settings = match config::get_app_settings() {
        Ok(s) => s,
        Err(_) => return,
    };
    if !settings.process_watcher_enabled {
        return;
    }

    let running = running_process_names();

    // 只对第一个命中的进程补发（handle_event 里 active_rule 一旦设上，
    // 后续 Started 会提前 return，这里也保持单次语义）
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
        handle_event(ProcessEvent::Started(name), app);
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

            // 初始订阅
            let settings = config::get_app_settings().unwrap_or_default();
            if settings.process_watcher_enabled {
                if let Some(wql) = build_wql(&settings.process_rules) {
                    let names = subscribed_names(&settings.process_rules);
                    state().lock().unwrap().subscribed_processes = names;

                    let (etx, erx) = mpsc::channel();
                    let (stx, srx) = mpsc::channel();
                    monitor_handle = spawn_wmi_monitor(etx, wql, srx);
                    event_rx = Some(erx);
                    stop_tx = Some(stx);

                    // 对账：补处理订阅前已在运行的进程（WMI 不补发历史启动事件）
                    reconcile_running_processes(&app_handle);
                }
            }

            loop {
                match cmd_receiver.try_recv() {
                    Ok(WatcherCommand::Resubscribe) => {
                        // 停止旧 WMI 线程
                        if let Some(tx) = stop_tx.take() {
                            let _ = tx.send(());
                        }
                        drop(monitor_handle.take());
                        drop(event_rx.take());

                        let settings = config::get_app_settings().unwrap_or_default();
                        if settings.process_watcher_enabled {
                            if let Some(wql) = build_wql(&settings.process_rules) {
                                let names = subscribed_names(&settings.process_rules);
                                state().lock().unwrap().subscribed_processes = names;

                                let (etx, erx) = mpsc::channel();
                                let (stx, srx) = mpsc::channel();
                                monitor_handle = spawn_wmi_monitor(etx, wql, srx);
                                event_rx = Some(erx);
                                stop_tx = Some(stx);

                                // 对账：补处理订阅前已在运行的进程
                                reconcile_running_processes(&app_handle);
                            }
                        } else {
                            state().lock().unwrap().subscribed_processes.clear();
                        }
                    }
                    Ok(WatcherCommand::Stop) => {
                        if let Some(tx) = stop_tx.take() {
                            let _ = tx.send(());
                        }
                        drop(monitor_handle.take());
                        drop(event_rx.take());
                        WATCHER_RUNNING.store(false, Ordering::Relaxed);
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        break;
                    }
                }

                if let Some(ref rx) = event_rx {
                    match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(event) => handle_event(event, &app_handle),
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            if let Some(tx) = stop_tx.take() {
                                let _ = tx.send(());
                            }
                            drop(monitor_handle.take());
                            drop(event_rx.take());
                        }
                    }
                } else {
                    thread::sleep(Duration::from_millis(500));
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
    })
}
