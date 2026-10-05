//! 调试记录器：诊断模式下把「屏幕写了什么、系统怎么回应」落到磁盘。
//!
//! 触发（前端）：连续点击左上角图标 10 次 → 弹窗征得用户同意 → 开始记录。
//! 记录内容：ICC 应用、伽马写入（滑条/托盘/快捷键/进程监听）的参数、API 结果、
//! **回读验证差值**、显示器/驱动环境快照。只写本机磁盘，不联网；
//! 报告由用户手动发送。
//!
//! 为什么回读验证挂在调试模式：正常路径行为与开销零变化；调试模式下每次
//! LUT 写入后读回比对，才能区分「正常生效」「API 假成功」「被静默拒绝」——
//! `GetDeviceGammaRamp` 读的是系统缓存，`SetDeviceGammaRamp` 返回 TRUE 不代表
//! 硬件真的编程了 LUT，这正是「点了没反应」类问题唯一可靠的判定手段。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

/// 单日志上限；超过后整体改名让位（保留一代）
const MAX_LOG_BYTES: u64 = 1024 * 1024;

#[derive(Debug)]
struct Session {
    log_path: PathBuf,
    started_ms: u128,
    /// 会话开始的本地时间戳（YYYYMMDD-HHMMSS），会话日志与桌面报告共用，
    /// 让「哪个日志配哪份报告、哪份在前」一眼可辨
    started_label: String,
}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);
static EVENT_COUNT: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn logs_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("filter-manage").join("logs"))
}

fn desktop_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\".into());
    let desktop = PathBuf::from(&home).join("Desktop");
    if desktop.is_dir() {
        desktop
    } else {
        PathBuf::from(home)
    }
}

/// 自 1970-01-01 起的天数 → (年, 月, 日)，UTC；Howard Hinnant 的 civil_from_days。
/// 仅非 Windows 平台的 local_now 回退在用（Windows 构建走 GetLocalTime，这里只剩测试引用）。
#[cfg_attr(windows, allow(dead_code))]
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let (y, m) = if m <= 2 { (y + 1, m) } else { (y, m) };
    (y, m, d)
}

/// 本地时间 (年, 月, 日, 时, 分, 秒, 毫秒)。
/// Windows 直接取 GetLocalTime；非 Windows 回退到 UTC 换算（本应用仅面向 Windows）。
#[cfg(windows)]
fn local_now() -> (u32, u32, u32, u32, u32, u32, u32) {
    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        millis: u16,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetLocalTime(out: *mut SystemTime);
    }
    unsafe {
        let mut t = SystemTime::default();
        GetLocalTime(&mut t);
        (
            t.year as u32,
            t.month as u32,
            t.day as u32,
            t.hour as u32,
            t.minute as u32,
            t.second as u32,
            t.millis as u32,
        )
    }
}

#[cfg(not(windows))]
fn local_now() -> (u32, u32, u32, u32, u32, u32, u32) {
    let ms = now_ms() as u64;
    let (y, mo, d) = civil_from_days((ms / 86_400_000) as i64);
    (
        y as u32,
        mo,
        d,
        (ms / 3_600_000 % 24) as u32,
        (ms / 60_000 % 60) as u32,
        (ms / 1_000 % 60) as u32,
        (ms % 1000) as u32,
    )
}

/// 日志行时间戳。统一用**本地时间**：用户核对「我 21:40 点的按钮」时，
/// 不能要求他再做一次时区换算。
fn fmt_ts() -> String {
    let (y, mo, d, h, mi, s, ms) = local_now();
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}.{ms:03}")
}

/// 文件名用的本地时间戳（2026-10-01_00-00-00 式）。
/// 下划线分段让用户一眼看懂；冒号是 Windows 文件名非法字符，时间分隔用 - 代替。
/// 字段定宽、字典序 = 时间序，多份报告按名称排序即可分辨先后。
fn local_stamp() -> String {
    let (y, mo, d, h, mi, s, _) = local_now();
    format!("{y:04}-{mo:02}-{d:02}_{h:02}-{mi:02}-{s:02}")
}

/// 记录一条调试事件。未开启调试模式时零成本返回；任何 IO 错误静默忽略
/// （日志绝不能反过来影响业务路径）。
pub fn log(kind: &str, message: impl AsRef<str>) {
    let Ok(mut guard) = SESSION.lock() else { return };
    let Some(session) = guard.as_mut() else { return };
    if let Ok(meta) = fs::metadata(&session.log_path) {
        if meta.len() > MAX_LOG_BYTES {
            let rotated = session.log_path.with_extension("log.1");
            let _ = fs::rename(&session.log_path, rotated);
        }
    }
    let line = format!("{} [{}] {}\n", fmt_ts(), kind, message.as_ref());
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&session.log_path) {
        let _ = f.write_all(line.as_bytes());
        EVENT_COUNT.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn active() -> bool {
    SESSION.lock().map(|s| s.is_some()).unwrap_or(false)
}

/// panic 兜底：无论是否在调试记录中，都落一份 crash 日志（logs/crash.log，追加）；
/// 会话进行中则同时进会话日志。挂载点在 lib.rs::run()，全局唯一。
pub fn log_panic(message: &str) {
    log("panic", message);
    if let Some(dir) = logs_dir() {
        let _ = fs::create_dir_all(&dir);
        let line = format!("{} [panic] {}\n", fmt_ts(), message);
        if let Ok(mut f) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("crash.log"))
        {
            let _ = f.write_all(line.as_bytes());
        }
    }
}

/// 写入后回读验证（仅调试模式）：高字节最大差 0 = 系统层面确实接受了这条曲线。
/// 注意这只能证明到 OS 缓存一层；硬件是否真的用了它是另一回事，
/// 但「API 成功 + 回读不一致」可以当场坐实静默拒绝。
pub fn verify_ramp(device_id: &str, expected: &[u16; 768]) {
    if !active() {
        return;
    }
    let Some(actual) = read_ramp(device_id) else {
        log("verify", format!("device={device_id} 回读失败"));
        return;
    };
    let mut max_diff = 0i32;
    for i in 0..768 {
        let d = ((actual[i] >> 8) as i32 - (expected[i] >> 8) as i32).abs();
        if d > max_diff {
            max_diff = d;
        }
    }
    let verdict = if max_diff == 0 { "生效（回读一致）" } else { "被静默拒绝（回读不一致）" };
    log("verify", format!("device={device_id} 高字节最大差={max_diff} → {verdict}"));
}

/// 读取某显示器的当前 gamma ramp（768 个 u16，R/G/B 各 256）
fn read_ramp(device_id: &str) -> Option<[u16; 768]> {
    use windows::Win32::UI::ColorSystem::GetDeviceGammaRamp;
    use windows::Win32::Graphics::Gdi::{CreateDCW, DeleteDC};
    let wide: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let hdc = CreateDCW(windows::core::PCWSTR(wide.as_ptr()), None, None, None);
        if hdc.is_invalid() {
            return None;
        }
        let mut values = [0u16; 768];
        let ok = GetDeviceGammaRamp(hdc, values.as_mut_ptr() as *mut _);
        let _ = DeleteDC(hdc);
        ok.as_bool().then_some(values)
    }
}

// ─── 环境快照 ────────────────────────────────────────────────────────────────

fn reg_sz(path: &str, name: &str) -> Option<String> {
    use winreg::enums::*;
    let key = winreg::RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(path, KEY_READ)
        .ok()?;
    key.get_value(name).ok()
}

fn os_info() -> String {
    const CV: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let product = reg_sz(CV, "ProductName").unwrap_or_else(|| "?".into());
    let display = reg_sz(CV, "DisplayVersion").unwrap_or_default();
    let build = reg_sz(CV, "CurrentBuildNumber").unwrap_or_default();
    if display.is_empty() {
        format!("{product} (build {build})")
    } else {
        format!("{product} {display} (build {build})")
    }
}

fn gpu_drivers() {
    const CLASS: &str =
        r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";
    let mut found = 0;
    for idx in 0..10 {
        let sub = format!("{CLASS}\\{idx:04}");
        let Some(desc) = reg_sz(&sub, "DriverDesc") else { continue };
        found += 1;
        let version = reg_sz(&sub, "DriverVersion").unwrap_or_default();
        let date = reg_sz(&sub, "DriverDate").unwrap_or_default();
        log("env", format!("显卡驱动：{desc}  版本={version}  日期={date}"));
    }
    if found == 0 {
        log("env", "显卡驱动：（注册表里读不到记录）");
    }
}

/// 可能参与色彩管理的进程名片段（与诊断探针同一份名单的精简版）
const CLAIMER_TERMS: &[&str] = &[
    "msi", "dragon center", "creator center", "truecolor", "true color", "dcv2",
    "hips", "usysdiag", "wsctrl", "displaycal", "flux", "twinkletray", "monitorian",
    "onscreen control", "controlmymonitor",
];

fn claimer_processes() -> Vec<String> {
    let names = crate::process_watcher::running_process_names();
    names
        .into_iter()
        .filter(|n| {
            let lower = n.to_lowercase();
            CLAIMER_TERMS.iter().any(|t| lower.contains(t))
        })
        .collect()
}

// ─── HDR（高级颜色）状态 ─────────────────────────────────────────────────────
//
// DisplayConfig 三个函数直接走 user32 原始 FFI，结构体和常量用 windows crate 的
// 权威定义（手抄 DisplayConfig 布局必错）。HDR 桌面走另一条管线，legacy gamma
// ramp 对画面无效——它是「应用了没反应」的头号环境因素，快照必须带上。
#[cfg(windows)]
mod hdr {
    use windows::Win32::Devices::Display::{
        DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
        DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_HEADER,
        DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO, DISPLAYCONFIG_MODE_INFO,
        DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SOURCE_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS,
    };

    #[link(name = "user32")]
    extern "system" {
        fn GetDisplayConfigBufferSizes(flags: i32, num_paths: *mut u32, num_modes: *mut u32) -> i32;
        fn QueryDisplayConfig(
            flags: i32,
            num_paths: *mut u32,
            path_array: *mut DISPLAYCONFIG_PATH_INFO,
            num_modes: *mut u32,
            mode_array: *mut DISPLAYCONFIG_MODE_INFO,
            current_topology: *mut i32,
        ) -> i32;
        fn DisplayConfigGetDeviceInfo(request_packet: *mut DISPLAYCONFIG_DEVICE_INFO_HEADER) -> i32;
    }

    /// (GDI 设备名, 支持HDR, 已启用HDR)；单条查询失败时 ok=false
    pub fn states() -> Result<Vec<(String, bool, bool, bool)>, String> {
        unsafe {
            let (mut num_paths, mut num_modes) = (0u32, 0u32);
            let rc = GetDisplayConfigBufferSizes(
                QDC_ONLY_ACTIVE_PATHS.0 as i32,
                &mut num_paths,
                &mut num_modes,
            );
            if rc != 0 {
                return Err(format!("GetDisplayConfigBufferSizes winerr={rc}"));
            }
            let mut paths: Vec<DISPLAYCONFIG_PATH_INFO> = vec![std::mem::zeroed(); num_paths as usize];
            let mut modes: Vec<DISPLAYCONFIG_MODE_INFO> = vec![std::mem::zeroed(); num_modes as usize];
            let rc = QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS.0 as i32,
                &mut num_paths,
                paths.as_mut_ptr(),
                &mut num_modes,
                modes.as_mut_ptr(),
                std::ptr::null_mut(),
            );
            if rc != 0 {
                return Err(format!("QueryDisplayConfig winerr={rc}"));
            }

            let mut out = Vec::new();
            for path in &paths[..num_paths as usize] {
                let mut src: DISPLAYCONFIG_SOURCE_DEVICE_NAME = std::mem::zeroed();
                src.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
                src.header.size = std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
                src.header.adapterId = path.sourceInfo.adapterId;
                src.header.id = path.sourceInfo.id;
                let gdi = if DisplayConfigGetDeviceInfo(&mut src.header) == 0 {
                    let len = src
                        .viewGdiDeviceName
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(32);
                    String::from_utf16_lossy(&src.viewGdiDeviceName[..len])
                } else {
                    "(取不到 GDI 名)".into()
                };

                let mut aci: DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO = std::mem::zeroed();
                aci.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO;
                aci.header.size = std::mem::size_of::<DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO>() as u32;
                aci.header.adapterId = path.targetInfo.adapterId;
                aci.header.id = path.targetInfo.id;
                let (ok, supported, enabled) = if DisplayConfigGetDeviceInfo(&mut aci.header) == 0 {
                    (true, aci.Anonymous.value & 0x1 != 0, aci.Anonymous.value & 0x2 != 0)
                } else {
                    (false, false, false)
                };
                out.push((gdi, ok && supported, ok && enabled, ok));
            }
            Ok(out)
        }
    }
}

/// 环境快照：**逐行走 log()**，而不是攒一个大字符串再写。
/// 两个原因：① 快照里的探测（NVAPI 初始化、显示器枚举、创建 DC）可能耗时，
/// 逐行记录后就算卡在某一步，日志也能显示停在哪里；② 每行短暂持锁，
/// 快照里将来若调了 log() 也不会和自己持有的锁互卡。
fn write_env_snapshot() {
    log("env", format!("应用版本 = v{}", env!("CARGO_PKG_VERSION")));
    log("env", format!("OS = {}", os_info()));
    gpu_drivers();

    match crate::icc::get_display_monitors() {
        Ok(monitors) => {
            for m in &monitors {
                log(
                    "env",
                    format!(
                        "显示器 {} 主屏={} device={} pnp={}",
                        m.name, m.is_primary, m.device_id, m.pnp_id
                    ),
                );
                if let Some(ramp) = read_ramp(&m.device_id) {
                    let points = [0usize, 16, 64, 128, 192, 255];
                    let samples: String = points
                        .iter()
                        .map(|p| format!("{p}:{}", ramp[*p] >> 8))
                        .collect::<Vec<_>>()
                        .join(" ");
                    log("env", format!("当前 ramp R 采样 = {samples}"));
                }
            }
        }
        Err(e) => log("env", format!("显示器枚举失败：{e}")),
    }

    let cap = crate::nvidia::get_dvc_capability(None);
    log(
        "env",
        format!(
            "数字振动能力：supported={} vendor={} 原因={}",
            cap.supported,
            cap.vendor.as_deref().unwrap_or("无"),
            cap.reason.as_deref().unwrap_or("无")
        ),
    );

    // HDR 状态：HDR 桌面走另一条管线，legacy gamma ramp 对画面无效
    #[cfg(windows)]
    match hdr::states() {
        Ok(list) => {
            for (gdi, supported, enabled, ok) in list {
                if ok {
                    log("env", format!("HDR（高级颜色）[{gdi}]：支持={supported} 已启用={enabled}"));
                } else {
                    log("env", format!("HDR（高级颜色）[{gdi}]：查询失败"));
                }
            }
        }
        Err(e) => log("env", format!("HDR 状态整体查询失败：{e}")),
    }

    // 当前系统关联的 ICC（对比「app 认为应用了什么」和「系统实际挂着什么」）
    match crate::icc::get_current_icc_profile() {
        Ok(name) => log("env", format!("系统当前关联 ICC = {name}")),
        Err(e) => log("env", format!("系统当前关联 ICC 读取失败：{e}")),
    }

    // 进程监听状态：后台自动切换方案造成的「怪现象」会伪装成前台 bug，
    // 快照必须能看到监听是否在管这块屏
    match crate::process_watcher::get_watcher_status() {
        Ok(st) => log(
            "env",
            format!(
                "进程监听：启用={} 活跃规则={} 条 贴屏方案={:?} WMI 连接={}",
                st.enabled,
                st.active_rules.len(),
                st.active_config_name,
                st.wmi_connected
            ),
        ),
        Err(e) => log("env", format!("进程监听状态读取失败：{e}")),
    }

    let claimers = claimer_processes();
    if claimers.is_empty() {
        log("env", "可疑色彩管理进程：（无匹配）");
    } else {
        for name in claimers {
            log("env", format!("可疑色彩管理进程：{name}"));
        }
    }
}

// ─── 剪贴板（报告快速分享）────────────────────────────────────────────────────
//
// 报告生成后自动把**文件**放进剪贴板（CF_HDROP），用户直接粘贴进 QQ/微信即发送文件；
// 「复制报告文案」按钮则放**纯文本**（CF_UNICODETEXT），适合贴进任何文本输入框。
// 官方 clipboard 插件不支持文件列表，所以这里走 Win32 原生 API。
const CF_UNICODETEXT: u32 = 13;
const CF_HDROP: u32 = 15;

fn set_clipboard_bytes(format: u32, bytes: &[u8]) -> Result<(), String> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    unsafe {
        OpenClipboard(None).map_err(|_| "剪贴板打开失败（可能被其他程序占用，稍后重试）".to_string())?;
        let result = (|| -> Result<(), String> {
            EmptyClipboard().map_err(|_| "剪贴板清空失败".to_string())?;
            let handle = GlobalAlloc(GMEM_MOVEABLE, bytes.len())
                .map_err(|_| "剪贴板内存分配失败".to_string())?;
            let ptr = GlobalLock(handle);
            if ptr.is_null() {
                return Err("剪贴板内存锁定失败".into());
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr as *mut u8, bytes.len());
            let _ = GlobalUnlock(handle);
            SetClipboardData(format, HANDLE(handle.0))
                .map_err(|e| format!("剪贴板写入失败：{e}"))?;
            Ok(())
        })();
        let _ = CloseClipboard();
        result
    }
}

/// 把报告**文件**放进剪贴板（CF_HDROP + DROPFILES 双空结尾宽字符路径）
pub(crate) fn copy_report_file(path: &str) -> Result<(), String> {
    let mut wide: Vec<u16> = path.encode_utf16().collect();
    wide.extend_from_slice(&[0, 0]);
    let mut payload = vec![0u8; 20 + wide.len() * 2];
    // DROPFILES：offset 0 = pFiles（文件名数据起始偏移），offset 16 = fWide（1 = 宽字符）
    payload[0..4].copy_from_slice(&20u32.to_le_bytes());
    payload[16..20].copy_from_slice(&1u32.to_le_bytes());
    let bytes =
        unsafe { std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2) };
    payload[20..].copy_from_slice(bytes);
    set_clipboard_bytes(CF_HDROP, &payload)
}

/// 把报告**文本内容**放进剪贴板（CF_UNICODETEXT，双空结尾）
pub(crate) fn copy_report_text(text: &str) -> Result<(), String> {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    wide.push(0);
    let bytes =
        unsafe { std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2) }.to_vec();
    set_clipboard_bytes(CF_UNICODETEXT, &bytes)
}

// ─── Tauri 命令 ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct DebugStartInfo {
    pub log_path: String,
}

#[derive(Debug, Serialize)]
pub struct DebugStopInfo {
    pub report_path: String,
    /// 报告文件是否已自动复制到剪贴板（失败不阻断，前端提示用户手动操作）
    pub file_copied: bool,
}

/// 开始会话的实际逻辑。**必须是 async 命令 + spawn_blocking 调用**：
/// 快照里的 NVAPI 初始化 / 显示器枚举 / 创建 DC 可能耗时，同步命令跑在主线程上
/// 会把整个窗口卡成「未响应」（v0 首测就栽在这里）。
fn start_session_impl() -> Result<DebugStartInfo, String> {
    let dir = logs_dir().ok_or("无法定位日志目录")?;
    fs::create_dir_all(&dir).map_err(|e| format!("创建日志目录失败：{e}"))?;
    let started_label = local_stamp();
    let log_path = dir.join(format!("debug-session-{started_label}.log"));
    fs::write(&log_path, "==== 调试会话开始 ====\n").map_err(|e| format!("写入日志失败：{e}"))?;

    // 先登记会话再做快照：快照的每一行都通过 log() 落盘（逐行短暂持锁），
    // 这样就算探测卡在某一步，日志也能显示停在哪里
    {
        let mut guard = SESSION.lock().map_err(|_| "会话状态锁不可用")?;
        if guard.is_some() {
            return Err("调试记录已在进行中".into());
        }
        *guard = Some(Session {
            log_path: log_path.clone(),
            started_ms: now_ms(),
            started_label,
        });
        EVENT_COUNT.store(0, Ordering::Relaxed);
    }
    log("session", "用户已授权，调试记录开始");
    write_env_snapshot();
    log("session", "环境快照完成");

    Ok(DebugStartInfo {
        log_path: log_path.to_string_lossy().to_string(),
    })
}

/// 开始一次调试记录会话：创建日志文件并写入环境快照
#[tauri::command]
pub async fn debug_start() -> Result<DebugStartInfo, String> {
    tauri::async_runtime::spawn_blocking(start_session_impl)
        .await
        .map_err(|e| format!("Task join error: {}", e))?
}

/// 停止记录的实际逻辑（同 debug_start：文件 IO 不进主线程）
fn stop_session_impl() -> Result<Option<DebugStopInfo>, String> {
    // 先写停止行再摘掉会话——顺序反了 log() 会因为会话已移除而直接跳过，
    // 报告里就永远找不到「调试记录停止」这一行
    log("session", "调试记录停止");

    // 调试期间用户可能把屏幕拖得面目全非：停止时自动恢复默认配置，
    // 省去每次手动点「重置默认」。恢复发生在会话摘除前，
    // 恢复动作本身（含回读验证）会留在报告里，用户能看到屏幕被恢复成了什么。
    match crate::tray::apply_default_config() {
        Ok(()) => log("session", "已自动恢复默认设置"),
        Err(e) => log(
            "session",
            format!("自动恢复默认设置失败：{e}（可在主界面手动点击「重置默认」）"),
        ),
    }

    let Some(session) = SESSION.lock().map_err(|_| "会话状态锁不可用")?.take() else {
        return Ok(None);
    };
    let events = EVENT_COUNT.load(Ordering::Relaxed);

    let log_content = fs::read_to_string(&session.log_path).unwrap_or_default();
    let duration_secs = (now_ms().saturating_sub(session.started_ms)) / 1000;
    let report = format!(
        "================================================================\n\
         Filter-Manage 调试报告\n\
         =================================================================\n\
         应用版本 = v{}\n\
         会话时长 = {duration_secs} 秒    事件数 = {events}\n\
         详细日志 = {}\n\
         （本报告只含本机操作与系统响应，不含个人文件内容）\n\
         =================================================================\n\n\
         {log_content}",
        env!("CARGO_PKG_VERSION"),
        session.log_path.display(),
    );

    // 报告与会话日志用同一个「会话开始时间」命名：配对关系、先后顺序一目了然
    let report_path = desktop_dir().join(format!(
        "filter-manage-debug_{}.txt",
        session.started_label
    ));
    fs::write(&report_path, report).map_err(|e| format!("写报告失败：{e}"))?;

    // 默认帮忙把报告文件放进剪贴板——用户直接粘贴进 QQ/微信即发送文件；
    // 失败不阻断（剪贴板可能被占用），前端会提示手动选择文件
    let file_copied = copy_report_file(&report_path.to_string_lossy()).is_ok();
    log(
        "session",
        format!("报告已生成：{}（文件自动复制到剪贴板={file_copied}）", report_path.display()),
    );
    Ok(Some(DebugStopInfo {
        report_path: report_path.to_string_lossy().to_string(),
        file_copied,
    }))
}

/// 「复制报告文案」：把报告的文本内容放进剪贴板（适合贴进文本输入框）
#[tauri::command]
pub async fn debug_copy_report_text(path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let text = fs::read_to_string(&path).map_err(|e| format!("读取报告失败：{e}"))?;
        copy_report_text(&text)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

/// 停止记录，生成桌面报告并自动复制文件到剪贴板。返回报告信息。
#[tauri::command]
pub async fn debug_stop() -> Result<Option<DebugStopInfo>, String> {
    tauri::async_runtime::spawn_blocking(stop_session_impl)
        .await
        .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub fn debug_status() -> bool {
    active()
}

/// 前端事件（点击 ICC、应用方案、切显示器等）透传进日志
#[tauri::command]
pub fn debug_note(source: String, event: String) {
    log(&source, event);
}

#[cfg(test)]
mod tests {
    use super::civil_from_days;

    #[test]
    fn civil_dates_round_trip() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        // 1970→2024 共 54 年 13 个闰年 = 19723 天到 2024-01-01，再加 31+28+1-1 天到闰日
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        assert_eq!(civil_from_days(20_727), (2026, 10, 1));
    }
}
