use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
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
    /// 最近激活的那条规则（= active_rules 的最后一条）；仅为兼容旧前端保留，
    /// 「屏幕上实际是什么方案」看 active_config_name
    pub active_rule: Option<ProcessRule>,
    /// 监听器当前贴在屏幕上的方案名；restore_on_exit=false 的规则退出后仍保留
    pub active_config_name: Option<String>,
    /// 当前处于激活状态的规则，按激活顺序排列（多个被监听的进程同时运行时会有多条）
    pub active_rules: Vec<ProcessRule>,
    pub subscribed_processes: Vec<String>,
    /// 当前是否持有存活的 WMI 事件通道
    pub wmi_connected: bool,
    /// 最近一次订阅/断开错误；成功订阅后清空
    pub last_error: Option<String>,
    /// 当前退避重连次数；Live 时为 0
    pub reconnect_attempt: u32,
}

/// 监听器当前「贴在」屏幕上的那条规则。`restore_on_exit=false` 的规则在进程退出后
/// 屏幕不会还原，这个记录会保留，保证 status 与屏幕一致。
#[derive(Clone)]
struct AppliedConfig {
    rule_id: String,
    config_name: String,
}

/// 进程退出后要把屏幕切到的目标
#[derive(Clone)]
enum SwitchTarget {
    /// 回退到这条（仍在运行的）规则对应的方案
    Rule {
        rule_id: String,
        config_name: String,
    },
    /// 恢复默认方案
    Default,
    /// 屏幕保持不动（进程退出但 restore_on_exit=false）
    Keep,
}

/// 切方案失败后的待重试项
struct PendingRestore {
    target: SwitchTarget,
    /// 切换成功后要从 active_rules 里移除的规则
    rule_ids: Vec<String>,
    /// 这些规则监听的进程名：重试前要确认它们确实还没回来，
    /// 否则进程重启后这次恢复会把刚生效的配色又冲掉
    process_names: Vec<String>,
    /// 发起时的 APPLY_GENERATION；被别的应用改写后说明用户接管了屏幕，放弃重试
    generation: u64,
    attempt: u32,
    notify: bool,
    due_at: Instant,
}

/// 当前恢复所有权（含休眠窗口）：只跟随 `applied` 的当前方案。
/// `ownership` 回答「保护谁」；`window` 回答「何时检查、何时重写、何时停手」。
struct ActiveRecovery {
    ownership: crate::color_recovery::RecoveryOwnership,
    window: crate::color_recovery::RecoveryWindow,
}

struct WatcherState {
    /// 已激活的规则，按激活顺序排列（最早激活的在前）
    active_rules: Vec<ProcessRule>,
    /// 当前贴屏幕的规则
    applied: Option<AppliedConfig>,
    /// 恢复默认/回退失败后的重试队列（同一时刻最多一条）
    pending_restore: Option<PendingRestore>,
    /// 当前恢复任务：方案激活成功时建立，规则退出/改绑/外部改写时失效
    recovery: Option<ActiveRecovery>,
    subscribed_processes: Vec<String>,
    wmi_connected: bool,
    last_error: Option<String>,
    reconnect_attempt: u32,
}

// ─── 全局状态 ────────────────────────────────────────────────────────────────

static WATCHER_RUNNING: AtomicBool = AtomicBool::new(false);

/// 每次「屏幕上的配色被改写」时自增（滑块 / DVC / ICC / 恢复默认 / 托盘 / 快捷键 /
/// 进程监听 / 显示模式命令都算；统一在 nvidia.rs 的颜色写门闩内递增）。
///
/// 用途：待重试的恢复动作与恢复所有权记下发起时的代号，行动前先比对——代号变了
/// 说明期间有人手动改过配色，这时再恢复默认就会把用户刚选的设置冲掉，直接放弃。
static APPLY_GENERATION: AtomicU64 = AtomicU64::new(0);

/// 颜色写门闩（nvidia::begin_external_write）的闩内递增入口
pub(crate) fn note_color_applied() {
    APPLY_GENERATION.fetch_add(1, Ordering::SeqCst);
}

/// 当前代号查询。原本仅模块内可见；恢复协调器（nvidia::recovery_write_ramp 的
/// 闩内复核）需要按计划收紧地开放它。
pub(crate) fn apply_generation() -> u64 {
    APPLY_GENERATION.load(Ordering::SeqCst)
}

/// 切方案失败后的重试次数上限；超过后保留状态、交给下次重订时的对账继续兜底
const RESTORE_RETRY_MAX: u32 = 3;

fn state() -> &'static Arc<Mutex<WatcherState>> {
    static STATE: OnceLock<Arc<Mutex<WatcherState>>> = OnceLock::new();
    STATE.get_or_init(|| {
        Arc::new(Mutex::new(WatcherState {
            active_rules: Vec::new(),
            applied: None,
            pending_restore: None,
            recovery: None,
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

/// watcher 日志：stderr 常驻 + 调试记录会话落盘（"process-watcher" 分类）。
/// 调试未开启时 debug_recorder::log 内部直接返回，正常路径零落盘开销；
/// 语义记录（Started / 规则 id / 方案名 / 目标显示器 / 激活与退出）因此
/// 无需第二套日志管线。
fn pw_log(msg: impl AsRef<str>) {
    let msg = msg.as_ref();
    eprintln!("[process-watcher] {msg}");
    crate::debug_recorder::log("process-watcher", msg);
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
    /// 颜色环境变化信号（显示模式 / 前台窗口）：只影响恢复调度，
    /// 不当进程创建/退出事件处理
    ColorEnvChanged(ColorEnvEvent),
}

/// 颜色环境变化信号（Task 4）。只携带定位信息，不携带窗口标题等原始数据。
#[derive(Debug, Clone)]
pub(crate) enum ColorEnvEvent {
    /// 显示模式变化（WM_DISPLAYCHANGE，display_watch 独立信号，不经拓扑列表比较）
    DisplayModeChanged,
    /// 某窗口成为前台（EVENT_SYSTEM_FOREGROUND，foreground_watch）
    Foreground { pid: u32, hwnd: isize },
}

/// 颜色环境信号入口（display_watch / foreground_watch 调用）：投递给 watcher 线程。
/// watcher 未运行或已停止时静默丢弃——监听都没开，恢复任务也不可能存在。
pub(crate) fn notify_color_env_changed(event: ColorEnvEvent) {
    if let Some(tx) = cmd_tx().lock().unwrap().as_ref() {
        let _ = tx.send(WatcherCommand::ColorEnvChanged(event));
    }
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
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use windows::Win32::UI::Shell::ExtractIconExW;
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

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
        windows::Win32::Graphics::Gdi::GetObjectW(
            hbitmap,
            bmp_size,
            Some(&mut bmp as *mut _ as *mut _),
        );

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
                rgba[di] = pixels[si + 2]; // R
                rgba[di + 1] = pixels[si + 1]; // G
                rgba[di + 2] = pixels[si]; // B

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
            let mut encoder = png::Encoder::new(&mut buf, width as u32, height as u32);
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
                        get_process_exe_path(pid).and_then(|path| extract_icon_base64(&path))
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
pub(crate) fn running_process_names() -> Vec<String> {
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
pub(crate) fn running_process_names() -> Vec<String> {
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

    // 方案名是用户自由输入，含 & < > 等字符会破坏 XML；转义后再拼，避免 LoadXml 失败导致监听线程 panic 退出
    let safe_body = body
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let xml = XmlDocument::new().unwrap();
    if xml
        .LoadXml(&HSTRING::from(format!(
            r#"<toast><visual><binding template="ToastGeneric"><text>Filter Manage</text><text>{}</text></binding></visual></toast>"#,
            safe_body
        )))
        .is_err()
    {
        return;
    }

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
    struct EventSink {
        tx: mpsc::Sender<ProcessEvent>,
        stop: AtomicBool,
    }

    impl EventSink {
        fn new(tx: mpsc::Sender<ProcessEvent>) -> Self {
            Self {
                tx,
                stop: AtomicBool::new(false),
            }
        }
    }

    // COM 接口参数名必须与 Windows IDL 一致（PascalCase），不能改成 snake_case
    #[allow(non_snake_case)]
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
        pub fn new(
            wql: &str,
            event_tx: mpsc::Sender<ProcessEvent>,
        ) -> std::result::Result<Self, String> {
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
                    1, // RPC_C_AUTHN_LEVEL_CONNECT
                    3, // RPC_C_IMP_LEVEL_IMPERSONATE
                    std::ptr::null(),
                    0, // EOAC_NONE
                    std::ptr::null(),
                );

                let loc: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)
                    .map_err(|e| format!("CoCreateInstance: {:?}", e))?;

                let svc = loc
                    .ConnectServer(&BSTR::from("ROOT\\CIMV2"), None, None, None, 0, None, None)
                    .map_err(|e| format!("ConnectServer: {:?}", e))?;

                let _ = CoSetProxyBlanket(
                    &svc,
                    10, // RPC_C_AUTHN_WINNT
                    0,  // RPC_C_AUTHZ_NONE
                    None,
                    RPC_C_AUTHN_LEVEL(3), // RPC_C_AUTHN_LEVEL_CALL
                    RPC_C_IMP_LEVEL(3),   // RPC_C_IMP_LEVEL_IMPERSONATE
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

/// 订阅结果（成功/失败原因）通过 `ready_rx` 回传给调用方。
///
/// 之前这里只回传 JoinHandle，调用方 spawn 成功就当订阅成功上报健康状态——
/// 而 `WmiSubscription::new` 是在线程里跑的，失败时线程只是 return，没人知道，
/// 前端会一直显示「已连接」但一条事件都收不到。
type WmiMonitorParts = (thread::JoinHandle<()>, mpsc::Receiver<Result<(), String>>);

#[cfg(windows)]
fn spawn_wmi_monitor(
    event_tx: mpsc::Sender<ProcessEvent>,
    wql: String,
    stop_rx: mpsc::Receiver<()>,
) -> Option<WmiMonitorParts> {
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();

    let handle = thread::Builder::new()
        .name("wmi-monitor".into())
        .spawn(move || {
            let sub = match wmi_impl::WmiSubscription::new(&wql, event_tx) {
                Ok(s) => s,
                Err(e) => {
                    pw_log(format!("WMI subscribe failed: {e}"));
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };

            let _ = ready_tx.send(Ok(()));
            // Block until stop signal arrives (subscription stays alive)
            let _ = stop_rx.recv();
            drop(sub);
        })
        .ok()?;

    Some((handle, ready_rx))
}

#[cfg(not(windows))]
fn spawn_wmi_monitor(
    _event_tx: mpsc::Sender<ProcessEvent>,
    _wql: String,
    _stop_rx: mpsc::Receiver<()>,
) -> Option<WmiMonitorParts> {
    None
}

// ─── 事件处理 ────────────────────────────────────────────────────────────────

/// 从 active_rules 移除这些规则，并清掉指向它们的待重试项（单次加锁）。不动 `applied`。
fn forget_rules(ids: &[String]) {
    let mut st = state().lock().unwrap();
    st.active_rules.retain(|r| !ids.contains(&r.id));
    let emptied = match st.pending_restore.as_mut() {
        Some(p) => {
            p.rule_ids.retain(|id| !ids.contains(id));
            p.rule_ids.is_empty()
        }
        None => false,
    };
    if emptied {
        st.pending_restore = None;
    }
}

/// 把屏幕切到目标方案（来源标记进诊断日志，区分托盘 / 快捷键 / 进程监听）
fn apply_target(
    target: &SwitchTarget,
) -> Result<Option<crate::color_recovery::RecoveryOwnership>, String> {
    match target {
        SwitchTarget::Rule {
            rule_id,
            config_name,
        } => {
            let cfg = config::load_config(config_name.clone())?;
            tray::apply_color_config_with_recovery(rule_id, &cfg)
        }
        SwitchTarget::Default => tray::apply_default_config_from("process-watcher").map(|_| None),
        SwitchTarget::Keep => Ok(None),
    }
}

/// 切换成功后通知前端 + 弹提示
fn announce_target(app: &AppHandle, target: &SwitchTarget, notify: bool) {
    match target {
        SwitchTarget::Rule { config_name, .. } => {
            let _ = app.emit("config-applied", config_name);
            if notify {
                show_toast(&format!("进程退出：已回退到「{}」", config_name));
            }
        }
        SwitchTarget::Default => {
            let _ = app.emit("config-applied", "__default__");
            if notify {
                show_toast("进程退出：已恢复默认方案");
            }
        }
        SwitchTarget::Keep => {}
    }
}

/// 同名多实例：仍有同名进程在跑则忽略本次退出（退出 ≠ 最后一个实例退出）。
/// 提取为纯函数供状态测试与真实事件路径共用。
fn stop_should_be_ignored(name: &str, running: &[String]) -> bool {
    running.iter().any(|p| p.eq_ignore_ascii_case(name))
}

/// 待重试项是否已被别处的应用作废：代号变化 = 用户/别的路径接管了屏幕，
/// 此时旧重试必须取消，否则会把用户刚选的配色覆盖掉。纯函数，测试共用。
fn restore_retry_stale(pending_generation: u64, current_generation: u64) -> bool {
    pending_generation != current_generation
}

/// 丢弃待重试项，并把它名下那批（进程已退出的）规则一并移出 active_rules。
/// 屏幕已经落到新目标上时用它收尾。
fn take_pending_restore(st: &mut WatcherState) {
    if let Some(p) = st.pending_restore.take() {
        st.active_rules.retain(|r| !p.rule_ids.contains(&r.id));
    }
}

/// 登记一条待重试的切方案动作
fn schedule_restore_retry(target: SwitchTarget, leaving: &[ProcessRule], notify: bool) {
    let mut st = state().lock().unwrap();
    let attempt = st.pending_restore.as_ref().map(|p| p.attempt).unwrap_or(0) + 1;
    st.pending_restore = Some(PendingRestore {
        target,
        rule_ids: leaving.iter().map(|r| r.id.clone()).collect(),
        process_names: leaving.iter().map(|r| r.process_name.clone()).collect(),
        generation: apply_generation(),
        attempt,
        notify,
        due_at: Instant::now() + Duration::from_secs(1u64 << attempt.min(3)),
    });
}

/// 把一批规则移出 active_rules（进程退出 / 规则被删除或停用 / 监听关闭）。
///
/// 关键顺序：**先让屏幕切到正确的方案，成功之后才改内存状态**。这样切换失败时
/// `get_watcher_status` 报的仍是屏幕上实际生效的方案，不会出现「状态说没有方案、
/// 屏幕还停在触发方案」的错位；失败的动作会进重试队列。
///
/// 调用前不得持有 state 锁。
fn deactivate_rules(app: &AppHandle, leaving: &[ProcessRule], reason: &str, notify: bool) {
    if leaving.is_empty() {
        return;
    }
    let leaving_ids: Vec<String> = leaving.iter().map(|r| r.id.clone()).collect();

    let (owns_screen, restore_on_exit, survivor) = {
        let st = state().lock().unwrap();
        let owns_screen = st
            .applied
            .as_ref()
            .is_some_and(|a| leaving_ids.contains(&a.rule_id));
        if !owns_screen {
            (false, false, None)
        } else {
            let restore = st
                .applied
                .as_ref()
                .and_then(|a| st.active_rules.iter().find(|r| r.id == a.rule_id))
                .map(|r| r.restore_on_exit)
                .unwrap_or(true);
            // 待重试队列里的规则进程也已经退出了，不能拿它当接管目标，
            // 否则会把屏幕切到一条早就没了进程的方案上
            let retired: &[String] = st
                .pending_restore
                .as_ref()
                .map(|p| p.rule_ids.as_slice())
                .unwrap_or(&[]);
            let survivor = st
                .active_rules
                .iter()
                .rev()
                .find(|r| !leaving_ids.contains(&r.id) && !retired.contains(&r.id))
                .cloned();
            (true, restore, survivor)
        }
    };

    // 屏幕上的方案不由这批规则提供 → 直接清状态，不用动屏幕
    if !owns_screen {
        pw_log(format!(
            "deactivate ({reason}) rules={leaving_ids:?} without switching: {:?}",
            leaving
                .iter()
                .map(|r| r.process_name.as_str())
                .collect::<Vec<_>>()
        ));
        forget_rules(&leaving_ids);
        return;
    }

    // 屏幕由这批规则提供 → 本次激活的恢复所有权随规则退出而失效
    // （切回默认 / 回退幸存规则 / Keep 都一样；回退成功时会在下面按新激活重建）
    state().lock().unwrap().recovery = None;

    let target = match (&survivor, restore_on_exit) {
        // 还有别的被监听进程在跑 → 回退到它那条更新一点的规则
        (Some(r), _) => SwitchTarget::Rule {
            rule_id: r.id.clone(),
            config_name: r.config_name.clone(),
        },
        (None, true) => SwitchTarget::Default,
        // restore_on_exit=false：进程退出后刻意保留当前配色，屏幕不动
        (None, false) => SwitchTarget::Keep,
    };

    pw_log(format!(
        "deactivate ({reason}) rules={leaving_ids:?} → {}",
        describe_target(&target)
    ));

    if matches!(target, SwitchTarget::Keep) {
        forget_rules(&leaving_ids);
        return;
    }

    match apply_target(&target) {
        Ok(ownership) => {
            let recovery = recovery_from_ownership(ownership);
            {
                let mut st = state().lock().unwrap();
                st.active_rules.retain(|r| !leaving_ids.contains(&r.id));
                // 屏幕已经落到新目标上，之前排队的恢复动作连同它的规则一起作废
                take_pending_restore(&mut st);
                st.applied = match &target {
                    SwitchTarget::Rule {
                        rule_id,
                        config_name,
                    } => Some(AppliedConfig {
                        rule_id: rule_id.clone(),
                        config_name: config_name.clone(),
                    }),
                    _ => None,
                };
                st.recovery = recovery;
            }
            // 新所有权已与 applied 一起登记，来自整组应用在门闩内的快照。

            pw_log(format!(
                "deactivate ({reason}) applied {}",
                describe_target(&target)
            ));
            announce_target(app, &target, notify);
        }
        Err(e) => {
            // 屏幕没切换成功，状态就不能提前改：规则留在 active_rules 里，
            // status 与屏幕保持一致，并排队重试（恢复所有权已在上方清除）
            pw_log(format!(
                "deactivate ({reason}) failed: {e}; queued for retry"
            ));
            schedule_restore_retry(target, leaving, notify);
        }
    }
}

fn describe_target(target: &SwitchTarget) -> String {
    match target {
        SwitchTarget::Rule { config_name, .. } => format!("config '{config_name}'"),
        SwitchTarget::Default => "default".to_string(),
        SwitchTarget::Keep => "keep".to_string(),
    }
}

/// 规则在监听期间被改了绑定方案。若它正是当前生效的那条，按新方案重新应用
/// （不用先回默认再切一次，避免闪一下）；否则只更新记录。
fn reactivate_rule(app: &AppHandle, rule: &ProcessRule) {
    let was_applied = state()
        .lock()
        .unwrap()
        .applied
        .as_ref()
        .is_some_and(|a| a.rule_id == rule.id);

    let recovery = if was_applied {
        let target = SwitchTarget::Rule {
            rule_id: rule.id.clone(),
            config_name: rule.config_name.clone(),
        };
        match apply_target(&target) {
            Ok(ownership) => recovery_from_ownership(ownership),
            Err(e) => {
                // 新方案失败：状态仍是旧方案；本次外部写代号会让旧恢复任务失效。
                pw_log(format!("reactivate {} failed: {e}", rule.id));
                return;
            }
        }
    } else {
        None
    };

    {
        let mut st = state().lock().unwrap();
        if let Some(slot) = st.active_rules.iter_mut().find(|r| r.id == rule.id) {
            *slot = rule.clone();
        }
        if was_applied {
            st.applied = Some(AppliedConfig {
                rule_id: rule.id.clone(),
                config_name: rule.config_name.clone(),
            });
            // 新方案已经生效，排队的恢复动作作废；旧激活的恢复所有权也一并作废
            take_pending_restore(&mut st);
            st.recovery = recovery;
        }
    }
    pw_log(format!(
        "reactivated rule={} → {}",
        rule.id, rule.config_name
    ));
    if was_applied {
        let _ = app.emit("config-applied", &rule.config_name);
    }
}

/// 重试队列的驱动器，由 watcher 主循环每次迭代调用。
fn tick_pending_restore(app: &AppHandle) {
    if state().lock().unwrap().pending_restore.is_none() {
        return;
    }

    // 期间有别的应用改写了屏幕（托盘 / 快捷键 / 前端手动应用）→ 作废，
    // 否则重试会把用户刚选的配色覆盖掉
    let stale_ids = {
        let st = state().lock().unwrap();
        st.pending_restore
            .as_ref()
            .filter(|p| restore_retry_stale(p.generation, apply_generation()))
            .map(|p| p.rule_ids.clone())
    };
    if let Some(ids) = stale_ids {
        pw_log("pending restore dropped: the screen was changed elsewhere");
        {
            let mut st = state().lock().unwrap();
            take_pending_restore(&mut st);
            // 屏幕现在由别人决定，监听器已经不知道它是什么方案了
            if st
                .applied
                .as_ref()
                .is_some_and(|a| ids.contains(&a.rule_id))
            {
                st.applied = None;
            }
        }
        return;
    }

    let due = {
        let st = state().lock().unwrap();
        st.pending_restore
            .as_ref()
            .filter(|p| Instant::now() >= p.due_at)
            .map(|p| {
                (
                    p.target.clone(),
                    p.rule_ids.clone(),
                    p.process_names.clone(),
                    p.attempt,
                    p.notify,
                )
            })
    };
    let Some((target, rule_ids, process_names, attempt, notify)) = due else {
        return;
    };

    // 等待期间进程又起来了：这时屏幕上的配色对它是正确的，这次恢复已经没有意义
    let running = running_process_names();
    if process_names
        .iter()
        .any(|n| running.iter().any(|p| p.eq_ignore_ascii_case(n)))
    {
        pw_log("pending restore dropped: the process came back");
        state().lock().unwrap().pending_restore = None;
        return;
    }

    match apply_target(&target) {
        Ok(ownership) => {
            let recovery = recovery_from_ownership(ownership);
            {
                let mut st = state().lock().unwrap();
                st.pending_restore = None;
                st.active_rules.retain(|r| !rule_ids.contains(&r.id));
                st.applied = match &target {
                    SwitchTarget::Rule {
                        rule_id,
                        config_name,
                    } => Some(AppliedConfig {
                        rule_id: rule_id.clone(),
                        config_name: config_name.clone(),
                    }),
                    _ => None,
                };
                st.recovery = recovery;
            }
            pw_log(format!(
                "pending restore succeeded on attempt {attempt}: {}",
                describe_target(&target)
            ));
            // 幸存规则的新激活已使用本次应用返回的持闩快照。

            announce_target(app, &target, notify);
        }
        Err(e) => {
            if attempt >= RESTORE_RETRY_MAX {
                // 放弃重试但状态保持真实：规则仍在 active_rules 里，status 报的就是屏幕上的方案。
                // 下次重订 WMI 时 reconcile 会发现它的进程已退出，再走一遍修复流程。
                pw_log(format!(
                    "restore failed {attempt} times, giving up until next reconcile: {e}"
                ));
                state().lock().unwrap().pending_restore = None;
                return;
            }
            pw_log(format!("restore attempt {attempt} failed: {e}"));
            let mut st = state().lock().unwrap();
            if let Some(p) = st.pending_restore.as_mut() {
                p.attempt = attempt + 1;
                // 本次失败过程中可能已经有部分应用落到了屏幕上，重新对齐代号
                p.generation = apply_generation();
                p.due_at = Instant::now() + Duration::from_secs(1u64 << (attempt + 1).min(3));
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
                    .find(|r| {
                        r.enabled
                            && r.process_name.eq_ignore_ascii_case(&name)
                            // 已经在激活列表里 → 方案早就应用过了，不用重复切
                            && !st.active_rules.iter().any(|a| a.id == r.id)
                    })
                    .cloned()
            };
            let Some(rule) = rule else {
                return;
            };
            pw_log(format!(
                "event Started matched rule={} config={}",
                rule.id, rule.config_name
            ));

            let config_name = rule.config_name.clone();
            let notify = settings.process_notification;

            let applied = config::load_config(config_name.clone())
                .and_then(|cfg| tray::apply_color_config_with_recovery(&rule.id, &cfg));
            match applied {
                Ok(ownership) => {
                    let recovery = recovery_from_ownership(ownership);
                    // 先登记再 emit，避免前端 get_watcher_status 与事件竞态
                    {
                        let mut st = state().lock().unwrap();
                        st.active_rules.retain(|r| r.id != rule.id);
                        st.active_rules.push(rule.clone());
                        st.applied = Some(AppliedConfig {
                            rule_id: rule.id.clone(),
                            config_name: config_name.clone(),
                        });
                        // 新方案成功：丢弃旧待办，只登记本次持闩捕获的新所有权。
                        take_pending_restore(&mut st);
                        st.recovery = recovery;
                    }
                    let active = state().lock().unwrap().active_rules.len();
                    pw_log(format!(
                        "activated rule={} config={config_name} source=process-watcher (active={active})",
                        rule.id
                    ));
                    let _ = app.emit("config-applied", &config_name);
                    if notify {
                        show_toast(&format!("进程触发：已切换到「{}」", config_name));
                    }
                }
                Err(e) => {
                    // 应用失败就不登记：状态里没有它，屏幕上也还是旧方案，两边一致
                    pw_log(format!("apply config '{config_name}' failed: {e}"));
                }
            }
        }
        ProcessEvent::Stopped(name) => {
            pw_log(format!("event Stopped name={name}"));

            // 同名多实例：仍有同名进程在跑则忽略本次退出（恢复所有权保持）
            if stop_should_be_ignored(&name, &running_process_names()) {
                pw_log(format!("Stopped ignored, instance still running: {name}"));
                return;
            }

            let leaving: Vec<ProcessRule> = {
                let st = state().lock().unwrap();
                st.active_rules
                    .iter()
                    .filter(|r| r.process_name.eq_ignore_ascii_case(&name))
                    .cloned()
                    .collect()
            };

            deactivate_rules(
                app,
                &leaving,
                "process stopped",
                settings.process_notification,
            );
        }
    }
}

// ─── 颜色恢复协调（Task 2 / Task 3）──────────────────────────────────────────
//
// 调度模型：所有恢复决策与规则生命周期事件都跑在 watcher 线程上（无每规则
// 线程，无进程表定时轮询——主循环每次迭代 tick 一次，休眠窗口只校验所有权）；
// 跨线程风险只有「外部颜色写入递增代号」，它在 nvidia.rs 门闩内被复核拦截。

/// 只使用整组应用在持颜色写门闩时返回的所有权；不得在此重新采样代号/LUT。
/// 新激活会开启启动窗口；之后的手动写改变代号，让本快照在首次 tick 前失效。
fn recovery_from_ownership(
    ownership: Option<crate::color_recovery::RecoveryOwnership>,
) -> Option<ActiveRecovery> {
    let o = ownership?;
    pw_log(format!(
        "color recovery armed: rule={} target={} pnp={} gen={} activation={}",
        o.rule_id, o.device_id, o.pnp_id, o.generation, o.activation_seq
    ));
    Some(ActiveRecovery {
        ownership: o,
        window: crate::color_recovery::RecoveryWindow::open_startup(Instant::now()),
    })
}

/// 恢复写入前的所有权与状态复核（纯函数，真实 tick 与测试共用）。
/// 返回 Err(reason) 时调度器不得写屏。
fn recovery_permission(
    st: &WatcherState,
    ownership: &crate::color_recovery::RecoveryOwnership,
    current_generation: u64,
) -> Result<(), &'static str> {
    if ownership.generation != current_generation {
        return Err("generation changed: screen was rewritten elsewhere");
    }
    let Some(applied) = st.applied.as_ref() else {
        return Err("no config applied");
    };
    if applied.rule_id != ownership.rule_id {
        return Err("a different rule owns the screen now");
    }
    if !st.active_rules.iter().any(|r| r.id == ownership.rule_id) {
        return Err("rule no longer active");
    }
    match st.recovery.as_ref() {
        Some(rec) if rec.ownership.activation_seq == ownership.activation_seq => Ok(()),
        _ => Err("ownership superseded by a newer activation"),
    }
}

/// 显示器身份复核：设备名复用（重新连接成另一台物理屏）时旧 pnp 不能继续写；
/// 设备消失（None）同样不算持有身份。
fn pnp_identity_holds(owned_pnp: &str, current: Option<&str>) -> bool {
    current.is_some_and(|p| p == owned_pnp)
}

/// 当前枚举里该设备名对应的 pnp_id；设备不存在时为 None
fn current_pnp_for_device(device_id: &str) -> Option<String> {
    if crate::color_backend::active_backend(device_id) == crate::color_backend::Backend::Nvidia {
        return crate::display_capability::inspect(device_id).ok().and_then(|cap| cap.stable_key);
    }
    crate::icc::enumerate_display_monitors()
        .into_iter()
        .find(|m| m.device_id == device_id)
        .map(|m| m.pnp_id)
}

/// 前台信号的进程匹配：只查这个 PID 的可执行文件名（不是进程快照，也不是
/// 原始窗口标题——按计划忽略标题）。
#[cfg(windows)]
fn process_name_of_pid(pid: u32) -> Option<String> {
    let path = get_process_exe_path(pid)?;
    Some(path.rsplit(['\\', '/']).next().unwrap_or(&path).to_string())
}

#[cfg(not(windows))]
fn process_name_of_pid(_pid: u32) -> Option<String> {
    None
}

/// 执行一次恢复写入：pnp 身份复核 → 门闩内代号复核 + 写缓存 LUT → 记预算。
/// 写入失败（含代号变化）同样计预算；代号变化或预算耗尽时不放回窗口。
fn finish_recovery_write(
    watcher: &Mutex<WatcherState>,
    io: &impl ColorRecoveryIo,
    now: Instant,
    mut rec: ActiveRecovery,
    reason: crate::color_recovery::WriteReason,
) {
    // 显示器身份复核：device_id 复用但 pnp 变化（重新连接成另一台物理屏）→
    // 旧身份不能继续写，恢复所有权作废
    let current_pnp = io.current_pnp(&rec.ownership.device_id);
    if !pnp_identity_holds(&rec.ownership.pnp_id, current_pnp.as_deref()) {
        pw_log(format!(
            "color recovery dropped: display identity changed for {} (now {}, owned pnp {})",
            rec.ownership.device_id,
            current_pnp.as_deref().unwrap_or("<device gone>"),
            rec.ownership.pnp_id
        ));
        return; // 不放回
    }

    let result = io.write_ramp(
        &rec.ownership.device_id,
        &rec.ownership.expected_lut,
        rec.ownership.generation,
        rec.ownership.backend,
    );
    let exhausted = rec.window.finish_write(now);
    match result {
        Ok(()) => {
            // 写入后回读仅用于日志：区分「写完系统层就一致」与「写入被拒/再次被覆盖」
            let post = if rec.ownership.backend == crate::color_backend::Backend::Nvidia {
                Err("NVIDIA 私有曲线无可靠硬件回读；本次只确认接口写入成功".into())
            } else { io.read_ramp(&rec.ownership.device_id) };
            let post_desc = match post {
                Ok(actual) => format!(
                    "写入后回读高字节差={}",
                    crate::color_recovery::ramp_high_byte_diff(
                        &rec.ownership.expected_lut,
                        &actual
                    )
                ),
                Err(e) => format!("写入后回读失败：{e}"),
            };
            pw_log(format!(
                "color recovery wrote cached LUT ({}) writes={}/{} {}",
                reason.as_str(),
                rec.window.writes_used,
                crate::color_recovery::MAX_RECOVERY_WRITES,
                post_desc
            ));
        }
        Err(crate::nvidia::RecoveryWriteError::GenerationChanged) => {
            pw_log("color recovery write aborted: generation changed inside latch");
            return; // 所有权已死，不放回
        }
        Err(crate::nvidia::RecoveryWriteError::WriteFailed(e)) => {
            pw_log(format!(
                "color recovery write failed ({}): {e}",
                reason.as_str()
            ));
        }
    }
    if exhausted {
        pw_log(format!(
            "color recovery stopped for this activation: {}",
            crate::color_recovery::CloseReason::BudgetExhausted.as_str()
        ));
        return; // 预算耗尽，不放回
    }
    watcher.lock().unwrap().recovery = Some(rec);
}

/// 恢复调度 I/O 边界：生产与测试共用同一 tick 编排，测试不写真实桌面。
trait ColorRecoveryIo {
    fn generation(&self) -> u64;
    fn current_pnp(&self, device_id: &str) -> Option<String>;
    fn read_ramp(&self, device_id: &str) -> Result<[u16; 768], String>;
    fn write_ramp(
        &self,
        device_id: &str,
        ramp: &[u16; 768],
        generation: u64,
        backend: crate::color_backend::Backend,
    ) -> Result<(), crate::nvidia::RecoveryWriteError>;
}
struct LiveColorRecoveryIo;
impl ColorRecoveryIo for LiveColorRecoveryIo {
    fn generation(&self) -> u64 {
        apply_generation()
    }
    fn current_pnp(&self, device_id: &str) -> Option<String> {
        current_pnp_for_device(device_id)
    }
    fn read_ramp(&self, device_id: &str) -> Result<[u16; 768], String> {
        crate::nvidia::read_gamma_ramp(device_id)
    }
    fn write_ramp(
        &self,
        device_id: &str,
        ramp: &[u16; 768],
        generation: u64,
        backend: crate::color_backend::Backend,
    ) -> Result<(), crate::nvidia::RecoveryWriteError> {
        crate::nvidia::recovery_write_ramp(device_id, ramp, generation, backend)
    }
}
fn tick_color_recovery() {
    tick_color_recovery_with(state().as_ref(), &LiveColorRecoveryIo, Instant::now());
}

/// 单次 tick 与规则生命周期事件在同一 watcher 线程执行；期限/冷却/预算由
/// color_recovery 决策，所有真实回读与受门闩保护的写入均在状态锁外执行。
fn tick_color_recovery_with(
    watcher: &Mutex<WatcherState>,
    io: &impl ColorRecoveryIo,
    now: Instant,
) {
    // 在同一次状态加锁中先校验再取出任务。若先 take 再复核，校验看到的
    // recovery 必然是 None，会把自己的有效激活误判成被替代。
    let mut rec = {
        let mut st = watcher.lock().unwrap();
        let Some(rec) = st.recovery.as_ref() else {
            return;
        };
        if let Err(reason) = recovery_permission(&st, &rec.ownership, io.generation()) {
            st.recovery = None;
            pw_log(format!("color recovery dropped: {reason}"));
            return;
        }
        st.recovery
            .take()
            .expect("validated recovery must still exist")
    };

    match rec.window.due_request(now) {
        crate::color_recovery::RecoveryRequest::Wait => {
            watcher.lock().unwrap().recovery = Some(rec);
        }
        crate::color_recovery::RecoveryRequest::Close(reason) => {
            pw_log(format!("color recovery window closed: {}", reason.as_str()));
            // 到期只是休眠：不回读，但有效所有权、已用预算和冷却仍由该激活持有。
            // 之后匹配的前台/模式事件可以重新开窗；预算耗尽则永久停止本次激活。
            if reason == crate::color_recovery::CloseReason::WindowExpired {
                watcher.lock().unwrap().recovery = Some(rec);
            }
        }
        crate::color_recovery::RecoveryRequest::Verify => {
            if rec.ownership.backend == crate::color_backend::Backend::Nvidia {
                // B 仅在有预算的环境事件后重写；不能用 GDI 缓存假装回读私有曲线。
                rec.window.finish_verify(now, None);
                watcher.lock().unwrap().recovery = Some(rec);
                return;
            }
            let read = io.read_ramp(&rec.ownership.device_id);
            let diff = read.as_ref().ok().map(|actual| {
                crate::color_recovery::ramp_high_byte_diff(&rec.ownership.expected_lut, actual)
            });
            match rec.window.finish_verify(now, diff) {
                crate::color_recovery::VerifyFollowup::None => {
                    if let Err(e) = &read {
                        // 读取失败仅记录并等待后续检查，不伪称覆盖、不直接写入
                        pw_log(format!(
                            "color recovery readback failed (will retry next tick): {e}"
                        ));
                    }
                    watcher.lock().unwrap().recovery = Some(rec);
                }
                crate::color_recovery::VerifyFollowup::WriteAfterCooldown => {
                    pw_log("color recovery: deviation detected, waiting write cooldown");
                    watcher.lock().unwrap().recovery = Some(rec);
                }
                crate::color_recovery::VerifyFollowup::CloseWindow(reason) => {
                    pw_log(format!("color recovery window closed: {}", reason.as_str()));
                }
                crate::color_recovery::VerifyFollowup::WriteNow(reason) => {
                    finish_recovery_write(watcher, io, now, rec, reason);
                }
            }
        }
        crate::color_recovery::RecoveryRequest::Write => {
            // 待办的事件后重写（安定 800ms 与冷却均已满足；即使 OS 回读一致
            // 也要重写一次——回读一致不能证明硬件扫描输出持续采用该曲线）
            finish_recovery_write(
                watcher,
                io,
                now,
                rec,
                crate::color_recovery::WriteReason::EventReassert,
            );
        }
    }
}

/// 颜色环境事件（显示模式变化 / 目标进程前台）：只在恢复所有权有效时重新开启
/// 10 秒检查窗口；无关窗口、设置窗口、其他游戏不触发任何恢复。
/// 环境事件与 tick 共用恢复状态；过期所有权不能被事件复活。
fn open_recovery_event_window(st: &mut WatcherState, generation: u64, now: Instant) -> bool {
    let Some(rec) = st.recovery.as_ref() else {
        return false;
    };
    if recovery_permission(st, &rec.ownership, generation).is_err()
        || rec.window.writes_used >= crate::color_recovery::MAX_RECOVERY_WRITES
    {
        st.recovery = None;
        return false;
    }
    st.recovery.as_mut().unwrap().window.note_event(now);
    true
}

fn handle_color_env_event(event: ColorEnvEvent) {
    match event {
        ColorEnvEvent::DisplayModeChanged => {
            pw_log("color-env: display mode changed (WM_DISPLAYCHANGE)");
            let mut st = state().lock().unwrap();
            if open_recovery_event_window(&mut st, apply_generation(), Instant::now()) {
                pw_log("color recovery: display mode event → 10s window opened");
            }
        }
        ColorEnvEvent::Foreground { pid, hwnd } => {
            // 无恢复任务时忽略：不读设置、不查进程，避免每次 Alt+Tab 都白做
            let active_rule_id = {
                let st = state().lock().unwrap();
                match st.recovery.as_ref() {
                    Some(rec) if rec.ownership.generation == apply_generation() => {
                        Some(rec.ownership.rule_id.clone())
                    }
                    _ => None,
                }
            };
            let Some(rule_id) = active_rule_id else {
                return;
            };

            let settings = match config::get_app_settings() {
                Ok(s) => s,
                Err(e) => {
                    pw_log(format!(
                        "color-env foreground: get_app_settings failed: {e}"
                    ));
                    return;
                }
            };
            if !settings.process_watcher_enabled {
                return;
            }

            let Some(name) = process_name_of_pid(pid) else {
                return;
            };
            // 只对「当前有效规则」的目标进程成为前台开窗：规则必须仍是
            // 恢复所有权对应的那条 applied 规则
            let matched = settings.process_rules.iter().any(|r| {
                r.enabled && r.id == rule_id && r.process_name.eq_ignore_ascii_case(&name)
            });
            if !matched {
                return;
            }
            pw_log(format!(
                "color recovery: foreground pid={pid} hwnd={hwnd:#x} name={name} matches applied rule → 10s window"
            ));
            let mut st = state().lock().unwrap();
            if st
                .recovery
                .as_ref()
                .is_some_and(|rec| rec.ownership.rule_id == rule_id)
            {
                open_recovery_event_window(&mut st, apply_generation(), Instant::now());
            }
        }
    }
}

// ─── 订阅后对账 ──────────────────────────────────────────────────────────────

/// WMI 的 `__InstanceOperationEvent` 只上报订阅之后发生的启动/退出事件，
/// 订阅前就已在运行的进程不会补发「启动」事件。
///
/// 在（重新）订阅成功后调用一次（非周期轮询）：
/// - 校正每条 active_rule（规则被删/停用/改过方案、进程已退出）
/// - 对仍在运行但尚未激活的规则补 Started
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

    // A. 校正已有的 active_rules（快照后再逐个处理，处理过程中不得持锁）
    let snapshot = state().lock().unwrap().active_rules.clone();
    for active in snapshot {
        let current = settings.process_rules.iter().find(|r| r.id == active.id);
        match current {
            None => {
                pw_log(format!(
                    "reconcile: rule removed → deactivate {}",
                    active.id
                ));
                deactivate_rules(
                    app,
                    &[active],
                    "rule removed",
                    settings.process_notification,
                );
            }
            Some(cur) if !cur.enabled => {
                pw_log(format!(
                    "reconcile: rule disabled → deactivate {}",
                    active.id
                ));
                deactivate_rules(
                    app,
                    &[active],
                    "rule disabled",
                    settings.process_notification,
                );
            }
            Some(_)
                if !running
                    .iter()
                    .any(|p| p.eq_ignore_ascii_case(&active.process_name)) =>
            {
                pw_log(format!(
                    "reconcile: active process gone → Stopped {}",
                    active.process_name
                ));
                handle_event(ProcessEvent::Stopped(active.process_name.clone()), app);
            }
            // 规则换了监听对象 → 旧进程的关联作废；新对象是否该激活交给下面的 B
            // 和后续 WMI 事件，这里只把旧的收掉
            Some(cur) if cur.process_name != active.process_name => {
                pw_log(format!(
                    "reconcile: rule {} retargeted {} → {}",
                    cur.id, active.process_name, cur.process_name
                ));
                deactivate_rules(
                    app,
                    &[active],
                    "rule retargeted",
                    settings.process_notification,
                );
            }
            // 规则换了绑定方案 → 按新方案重新应用，
            // 否则屏幕会一直停在编辑前的旧方案上
            Some(cur) if cur.config_name != active.config_name => {
                pw_log(format!(
                    "reconcile: rule {} rebound {} → {}",
                    cur.id, active.config_name, cur.config_name
                ));
                reactivate_rule(app, cur);
            }
            Some(_) => {}
        }
    }

    // B. 对仍在运行、但还没进入 active_rules 的规则补 Started
    let mut started_any = false;
    for rule in settings.process_rules.iter().filter(|r| r.enabled) {
        let already_active = state()
            .lock()
            .unwrap()
            .active_rules
            .iter()
            .any(|a| a.id == rule.id);
        if already_active {
            continue;
        }
        if running
            .iter()
            .any(|p| p.eq_ignore_ascii_case(&rule.process_name))
        {
            pw_log(format!("reconcile Started name={}", rule.process_name));
            handle_event(ProcessEvent::Started(rule.process_name.clone()), app);
            started_any = true;
        }
    }
    if !started_any {
        pw_log("reconcile skip: no missing running process");
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
    // 先取出 attempt 释放锁，再调 set_health —— set_health 内部会再锁同一把 state()，
    // 若把 state().lock() 直接写进实参，guard 要到语句结束才释放，会与内部加锁死锁。
    let attempt = state().lock().unwrap().reconnect_attempt;
    set_health(false, None, attempt);
}

/// 订阅结果最多等这么久；WMI 起 COM + 连接 ROOT\CIMV2 偶尔要几秒
const SUBSCRIBE_READY_TIMEOUT: Duration = Duration::from_secs(15);

fn try_start_subscription(settings: &AppSettings) -> Option<SubscribeParts> {
    if !settings.process_watcher_enabled {
        return None;
    }
    let wql = build_wql(&settings.process_rules)?;
    let names = subscribed_names(&settings.process_rules);
    state().lock().unwrap().subscribed_processes = names.clone();

    let (etx, erx) = mpsc::channel();
    let (stx, srx) = mpsc::channel();
    let Some((handle, ready_rx)) = spawn_wmi_monitor(etx, wql, srx) else {
        pw_log("WMI subscribe failed: spawn wmi-monitor thread failed");
        let attempt = state().lock().unwrap().reconnect_attempt;
        set_health(
            false,
            Some("failed to spawn wmi-monitor thread".into()),
            attempt,
        );
        return None;
    };

    // 等线程真正订阅成功再上报健康：只看 spawn 成功会把「线程起来了但订阅失败」
    // 也算成已连接，前端显示正常、实际收不到任何事件。
    match ready_rx.recv_timeout(SUBSCRIBE_READY_TIMEOUT) {
        Ok(Ok(())) => {
            pw_log(format!("WMI subscribed: names={names:?}"));
            set_health(true, None, 0);
            Some((Some(handle), erx, stx))
        }
        Ok(Err(e)) => {
            // 线程已经自己退出了，句柄直接丢弃
            pw_log(format!("WMI subscribe failed: {e}"));
            let attempt = state().lock().unwrap().reconnect_attempt;
            set_health(false, Some(format!("WMI subscribe failed: {e}")), attempt);
            None
        }
        Err(e) => {
            // 超时：让线程尽快收摊，这次按失败处理
            pw_log(format!("WMI subscribe timed out waiting for ready: {e}"));
            drop(stx);
            let attempt = state().lock().unwrap().reconnect_attempt;
            set_health(false, Some("WMI subscribe timed out".into()), attempt);
            None
        }
    }
}

/// 关监听或无规则时：停 WMI、清订阅名、把所有激活规则收掉。
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
        let leaving = state().lock().unwrap().active_rules.clone();
        if !leaving.is_empty() {
            // notify 用 settings
            let notify = config::get_app_settings()
                .map(|s| s.process_notification)
                .unwrap_or(true);
            deactivate_rules(app, &leaving, "watcher disabled or no rules", notify);
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
                set_health(false, Some("initial subscribe failed".into()), 1);
                pw_log(format!("reconnect in {backoff_secs}s"));
            } else {
                pw_log("idle: watcher disabled or no enabled rules");
            }

            loop {
                // 上次恢复默认失败的话，到点了在这里重试
                tick_pending_restore(&app_handle);

                // 颜色恢复调度 tick：只在存在活跃恢复窗口时做真实工作
                tick_color_recovery();

                match cmd_receiver.try_recv() {
                    Ok(WatcherCommand::Resubscribe) => {
                        pw_log("command Resubscribe");
                        stop_subscription(&mut monitor_handle, &mut event_rx, &mut stop_tx);
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
                                set_health(false, Some("resubscribe failed".into()), 1);
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
                    Ok(WatcherCommand::ColorEnvChanged(evt)) => {
                        handle_color_env_event(evt);
                    }
                    Ok(WatcherCommand::Stop) => {
                        pw_log("command Stop");
                        stop_subscription(&mut monitor_handle, &mut event_rx, &mut stop_tx);
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
                            stop_subscription(&mut monitor_handle, &mut event_rx, &mut stop_tx);
                            let settings = config::get_app_settings().unwrap_or_default();
                            if settings.process_watcher_enabled
                                && build_wql(&settings.process_rules).is_some()
                            {
                                backoff_secs = next_backoff_secs(backoff_secs);
                                next_retry_at =
                                    Some(Instant::now() + Duration::from_secs(backoff_secs));
                                let attempt = {
                                    let n =
                                        state().lock().unwrap().reconnect_attempt.saturating_add(1);
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
                        let due = next_retry_at.map(|t| Instant::now() >= t).unwrap_or(true);
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
                                set_health(false, Some("subscribe failed".into()), attempt.max(1));
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
        active_rule: st.active_rules.last().cloned(),
        active_config_name: st.applied.as_ref().map(|a| a.config_name.clone()),
        active_rules: st.active_rules.clone(),
        subscribed_processes: st.subscribed_processes.clone(),
        wmi_connected: st.wmi_connected,
        last_error: st.last_error.clone(),
        reconnect_attempt: st.reconnect_attempt,
    })
}

// ─── 恢复所有权状态测试（不接触真实桌面）──────────────────────────────────────
//
// 全部通过纯函数驱动：直接构造 WatcherState / 所有权 / 合成时间，不调 Win32、
// 不触碰全局 state() 单例（也不启动 watcher 线程）。

#[cfg(test)]
mod recovery_state_tests {
    use super::*;
    use crate::color_recovery::{RecoveryOwnership, RecoveryWindow};

    fn rule(id: &str, name: &str, config: &str) -> ProcessRule {
        ProcessRule {
            id: id.into(),
            process_name: name.into(),
            config_name: config.into(),
            enabled: true,
            restore_on_exit: true,
        }
    }

    fn ownership(rule_id: &str, seq: u64, gen: u64) -> RecoveryOwnership {
        RecoveryOwnership {
            backend: crate::color_backend::Backend::Standard,
            rule_id: rule_id.into(),
            activation_seq: seq,
            device_id: "\\\\.\\DISPLAY1".into(),
            pnp_id: "MONITOR\\TEST1111\\{aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee}".into(),
            generation: gen,
            expected_lut: [0x8000; 768],
        }
    }

    fn state_with(
        active: Vec<ProcessRule>,
        applied_rule: Option<&str>,
        recovery: Option<RecoveryOwnership>,
    ) -> WatcherState {
        WatcherState {
            active_rules: active,
            applied: applied_rule.map(|id| AppliedConfig {
                rule_id: id.into(),
                config_name: "cfg".into(),
            }),
            pending_restore: None,
            recovery: recovery.map(|o| ActiveRecovery {
                ownership: o,
                window: RecoveryWindow::open_startup(Instant::now()),
            }),
            subscribed_processes: vec![],
            wmi_connected: false,
            last_error: None,
            reconnect_attempt: 0,
        }
    }

    /// 场景 1：用户在别处改了配色（滑块 / 手动 ICC / 托盘）→ 代号变化使旧所有权
    /// 与旧重试一并取消；恢复调度拒绝写屏
    #[test]
    fn user_edit_cancels_stale_ownership_and_retry() {
        let st = state_with(
            vec![rule("r1", "game.exe", "cfg")],
            Some("r1"),
            Some(ownership("r1", 1, 5)),
        );
        let owned = st.recovery.as_ref().unwrap().ownership.clone();
        assert!(
            recovery_permission(&st, &owned, 5).is_ok(),
            "代号未变时所有权有效"
        );
        // 用户拖动滑块 → 门闩内代号递增为 6
        assert!(
            recovery_permission(&st, &owned, 6).is_err(),
            "旧代号下不得写屏（用户编辑使旧所有权失效）"
        );
        // 旧重试同理：待办的发起代号已过期
        assert!(restore_retry_stale(5, 6), "代号变化 → 旧重试取消");
        assert!(!restore_retry_stale(6, 6));
    }

    /// 场景 2：两个规则先后激活，只有贴屏的那条（后者）能恢复；
    /// active_rules 中已被后来方案盖住的规则不在保护范围
    #[test]
    fn only_the_applied_rule_may_recover() {
        let st = state_with(
            vec![rule("r1", "a.exe", "cfgA"), rule("r2", "b.exe", "cfgB")],
            Some("r2"),
            Some(ownership("r2", 2, 7)),
        );
        // r2（applied、最新激活）→ 允许
        assert!(recovery_permission(&st, &ownership("r2", 2, 7), 7).is_ok());
        // r1 的旧所有权 → 拒绝（applied 已不是它，激活序号也旧了）
        assert!(
            recovery_permission(&st, &ownership("r1", 1, 6), 7).is_err(),
            "被后来方案盖住的规则不得恢复"
        );
    }

    /// 场景 3：同名进程多实例，退出一个不触发恢复默认——
    /// 恢复所有权随「最后一个实例退出」才走 deactivate 清除
    #[test]
    fn one_of_two_instances_leaving_keeps_ownership() {
        let running = vec!["game.exe".to_string(), "other.exe".to_string()];
        assert!(
            stop_should_be_ignored("GAME.EXE", &running),
            "仍有同名实例在跑（大小写不敏感）→ 忽略退出，所有权保持"
        );
        assert!(
            !stop_should_be_ignored("game.exe", &["other.exe".to_string()]),
            "最后一个实例退出 → 才走 deactivate 清所有权"
        );
    }

    /// 场景 4：显示器重新连接且设备名复用时，旧 pnp 身份不能继续写
    #[test]
    fn reused_device_name_with_new_pnp_cannot_write() {
        let owned = "MONITOR\\AAA1111\\{aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee}";
        // 同名设备挂在另一台物理显示器（pnp 变了）→ 旧身份失效
        assert!(!pnp_identity_holds(owned, Some("MONITOR\\BBB2222\\{2}")));
        // 设备消失（显示器移除）→ 同样不算持有身份
        assert!(!pnp_identity_holds(owned, None), "设备移除 → 旧身份失效");
        // 同一台物理屏（pnp 一致）→ 身份成立
        assert!(pnp_identity_holds(owned, Some(owned)));
    }

    struct FakeRecoveryIo<'a> {
        watcher: &'a Mutex<WatcherState>,
        generation: std::cell::Cell<u64>,
        actual: std::cell::RefCell<[u16; 768]>,
        reads: std::cell::Cell<usize>,
        writes: std::cell::Cell<usize>,
        last_backend: std::cell::Cell<crate::color_backend::Backend>,
        edit_on_read: std::cell::Cell<bool>,
    }
    impl<'a> FakeRecoveryIo<'a> {
        fn new(watcher: &'a Mutex<WatcherState>) -> Self {
            Self {
                watcher,
                generation: std::cell::Cell::new(5),
                actual: std::cell::RefCell::new([0x8000; 768]),
                reads: std::cell::Cell::new(0),
                writes: std::cell::Cell::new(0),
                last_backend: std::cell::Cell::new(crate::color_backend::Backend::Standard),
                edit_on_read: std::cell::Cell::new(false),
            }
        }
        fn assert_unlocked(&self) {
            assert!(
                self.watcher.try_lock().is_ok(),
                "真实 I/O 不得持 watcher 状态锁"
            );
        }
    }
    impl ColorRecoveryIo for FakeRecoveryIo<'_> {
        fn generation(&self) -> u64 {
            self.generation.get()
        }
        fn current_pnp(&self, _: &str) -> Option<String> {
            self.assert_unlocked();
            Some(ownership("r1", 1, 5).pnp_id)
        }
        fn read_ramp(&self, _: &str) -> Result<[u16; 768], String> {
            self.assert_unlocked();
            self.reads.set(self.reads.get() + 1);
            if self.edit_on_read.replace(false) {
                self.generation.set(self.generation.get() + 1);
            }
            Ok(*self.actual.borrow())
        }
        fn write_ramp(
            &self,
            _: &str,
            ramp: &[u16; 768],
            expected_generation: u64,
            backend: crate::color_backend::Backend,
        ) -> Result<(), crate::nvidia::RecoveryWriteError> {
            self.assert_unlocked();
            if self.generation.get() != expected_generation {
                return Err(crate::nvidia::RecoveryWriteError::GenerationChanged);
            }
            self.writes.set(self.writes.get() + 1);
            self.last_backend.set(backend);
            *self.actual.borrow_mut() = *ramp;
            Ok(())
        }
    }
    fn tick_state(started: Instant, writes_used: u8) -> Mutex<WatcherState> {
        let mut st = state_with(
            vec![rule("r1", "game.exe", "cfg")],
            Some("r1"),
            Some(ownership("r1", 1, 5)),
        );
        let rec = st.recovery.as_mut().unwrap();
        rec.window = RecoveryWindow::open_startup(started);
        rec.window.writes_used = writes_used;
        Mutex::new(st)
    }

    #[test]
    fn nvidia_recovery_never_reads_gdi_and_reasserts_only_after_event() {
        let now = Instant::now();
        let watcher = tick_state(now, 0);
        watcher.lock().unwrap().recovery.as_mut().unwrap().ownership.backend = crate::color_backend::Backend::Nvidia;
        let io = FakeRecoveryIo::new(&watcher);
        tick_color_recovery_with(&watcher, &io, now + Duration::from_secs(2));
        assert_eq!(io.reads.get(), 0);
        assert_eq!(io.writes.get(), 0);
        assert!(open_recovery_event_window(&mut watcher.lock().unwrap(), 5, now + Duration::from_secs(3)));
        tick_color_recovery_with(&watcher, &io, now + Duration::from_secs(4));
        assert_eq!(io.reads.get(), 0);
        assert_eq!(io.writes.get(), 1);
        assert_eq!(io.last_backend.get(), crate::color_backend::Backend::Nvidia);
    }
    #[test]
    fn tick_keeps_valid_ownership_before_first_check() {
        let now = Instant::now();
        let watcher = tick_state(now, 0);
        let io = FakeRecoveryIo::new(&watcher);
        tick_color_recovery_with(&watcher, &io, now);
        assert!(watcher.lock().unwrap().recovery.is_some());
        assert_eq!(io.reads.get(), 0);
        assert_eq!(io.writes.get(), 0);
    }
    #[test]
    fn tick_reads_and_recovers_deviation_using_real_dispatch() {
        let now = Instant::now();
        let watcher = tick_state(now - Duration::from_secs(2), 0);
        let io = FakeRecoveryIo::new(&watcher);
        *io.actual.borrow_mut() = [0; 768];
        tick_color_recovery_with(&watcher, &io, now);
        assert_eq!(io.reads.get(), 2, "检查和写后回读各一次");
        assert_eq!(io.writes.get(), 1);
        assert_eq!(*io.actual.borrow(), [0x8000; 768]);
        assert_eq!(
            watcher
                .lock()
                .unwrap()
                .recovery
                .as_ref()
                .unwrap()
                .window
                .writes_used,
            1
        );
    }
    #[test]
    fn expired_tick_sleeps_then_event_reopens_with_original_budget() {
        let now = Instant::now();
        let watcher = tick_state(now - Duration::from_secs(121), 1);
        let io = FakeRecoveryIo::new(&watcher);
        tick_color_recovery_with(&watcher, &io, now);
        assert!(
            watcher.lock().unwrap().recovery.is_some(),
            "到期只关闭窗口，不丢有效所有权"
        );
        tick_color_recovery_with(&watcher, &io, now + Duration::from_secs(1));
        assert_eq!(io.reads.get(), 0, "休眠期间不轮询曲线");
        assert_eq!(io.writes.get(), 0);
        assert!(open_recovery_event_window(
            &mut watcher.lock().unwrap(),
            5,
            now + Duration::from_secs(2)
        ));
        tick_color_recovery_with(&watcher, &io, now + Duration::from_secs(3));
        assert_eq!(io.writes.get(), 1);
        assert_eq!(
            watcher
                .lock()
                .unwrap()
                .recovery
                .as_ref()
                .unwrap()
                .window
                .writes_used,
            2
        );
    }
    #[test]
    fn manual_edit_cancels_sleeping_owner_and_events_do_not_revive_it() {
        let now = Instant::now();
        let watcher = tick_state(now - Duration::from_secs(121), 0);
        let io = FakeRecoveryIo::new(&watcher);
        tick_color_recovery_with(&watcher, &io, now);
        assert!(watcher.lock().unwrap().recovery.is_some());
        io.generation.set(6);
        tick_color_recovery_with(&watcher, &io, now + Duration::from_secs(1));
        assert!(watcher.lock().unwrap().recovery.is_none());
        assert!(!open_recovery_event_window(
            &mut watcher.lock().unwrap(),
            6,
            now + Duration::from_secs(2)
        ));
        assert_eq!(io.reads.get(), 0);
        assert_eq!(io.writes.get(), 0);
    }
    #[test]
    fn reopened_window_does_not_reset_total_write_budget() {
        let now = Instant::now();
        let watcher = tick_state(now - Duration::from_secs(121), 2);
        let io = FakeRecoveryIo::new(&watcher);
        tick_color_recovery_with(&watcher, &io, now);
        assert!(open_recovery_event_window(
            &mut watcher.lock().unwrap(),
            5,
            now + Duration::from_secs(1)
        ));
        tick_color_recovery_with(&watcher, &io, now + Duration::from_secs(2));
        assert_eq!(io.writes.get(), 1);
        assert!(
            watcher.lock().unwrap().recovery.is_none(),
            "第三次尝试后停止本次激活"
        );
        assert!(!open_recovery_event_window(
            &mut watcher.lock().unwrap(),
            5,
            now + Duration::from_secs(3)
        ));
    }
    #[test]
    fn edit_between_verify_and_write_aborts_old_lut() {
        let now = Instant::now();
        let watcher = tick_state(now - Duration::from_secs(2), 0);
        let io = FakeRecoveryIo::new(&watcher);
        *io.actual.borrow_mut() = [0; 768];
        io.edit_on_read.set(true);
        tick_color_recovery_with(&watcher, &io, now);
        assert_eq!(io.reads.get(), 1);
        assert_eq!(io.writes.get(), 0, "写门闩内代号复核阻止旧 LUT");
        assert!(watcher.lock().unwrap().recovery.is_none());
    }
}
