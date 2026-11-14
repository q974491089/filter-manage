use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;
use winreg::enums::*;
use winreg::RegKey;

/// ICC 单文件上限：与上传网关（Worker）和投稿链路保持一致，避免出现
/// 「客户端能选、服务端拒收」的错位。
const MAX_ICC_BYTES: usize = 2 * 1024 * 1024;

/// 系统 ICC 配置文件目录。Windows 通常在 C 盘，但系统可能装在其它盘符，
/// 用 %SystemRoot%（找不到时回退 C:\Windows）拼出真实路径。
fn color_dir() -> PathBuf {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
    PathBuf::from(root).join("System32\\spool\\drivers\\color")
}

/// 判断文件是否为 ICC/ICM 配置（扩展名大小写不敏感）
fn is_icc_ext(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            let e = e.to_ascii_lowercase();
            e == "icc" || e == "icm"
        })
        .unwrap_or(false)
}

/// 将打包的内置 ICC 文件安装到系统 ICC 目录（首次启动调用）
#[tauri::command]
pub fn install_builtin_icc_profiles(app: tauri::AppHandle) -> Result<u32, String> {
    let color_dir = color_dir();
    let resource_dir = app.path().resource_dir()
        .map_err(|e| format!("Failed to get resource dir: {}", e))?
        .join("icc");

    if !resource_dir.exists() {
        return Ok(0);
    }

    let mut count = 0u32;
    for entry in fs::read_dir(&resource_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let src = entry.path();
        if src.extension().map(|e| e == "icc" || e == "icm").unwrap_or(false) {
            let dest = color_dir.join(entry.file_name());
            if !dest.exists() {
                fs::copy(&src, &dest).map_err(|e| format!("Failed to copy {:?}: {}", src, e))?;
                count += 1;
            }
        }
    }
    Ok(count)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IccProfile {
    pub name: String,
    pub path: String,
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct DisplayMonitor {
    pub name: String,
    pub device_id: String,
    pub pnp_id: String,
    pub is_primary: bool,
}

/// 从 EDID 数据解析 Monitor Name Descriptor (tag 0xFC)
fn parse_edid_monitor_name(edid: &[u8]) -> Option<String> {
    if edid.len() < 128 { return None; }
    // 4 个 18-byte descriptor blocks，从 offset 54 开始
    for i in 0..4 {
        let base = 54 + i * 18;
        if base + 18 > edid.len() { break; }
        // Monitor Name descriptor: 00 00 00 FC 00 ...
        if edid[base] == 0 && edid[base+1] == 0 && edid[base+2] == 0 && edid[base+3] == 0xFC {
            let name_bytes = &edid[base+5..base+18];
            let name = name_bytes.iter()
                .take_while(|&&b| b != b'\n' && b != 0)
                .map(|&b| b as char)
                .collect::<String>();
            let name = name.trim().to_string();
            if !name.is_empty() { return Some(name); }
        }
    }
    None
}

/// 从注册表 EDID 获取显示器型号名称
/// pnp_id 格式: \\?\DISPLAY#ACR0838#...#{...}  或  DISPLAY\ACR0838\...
pub(crate) fn get_monitor_name_from_edid(pnp_id: &str) -> Option<String> {
    let upper = pnp_id.to_uppercase();
    // 支持多种格式:
    //   MONITOR\ACR0838\{...}\0001        (EnumDisplayDevices 不带flag)
    //   \\?\DISPLAY#ACR0838#4&...#{...}   (带EDD_GET_DEVICE_INTERFACE_NAME)
    //   DISPLAY\ACR0838\...
    let hw_id = if let Some(pos) = upper.find("MONITOR\\") {
        let after = &pnp_id[pos + 8..];
        after.split('\\').next().unwrap_or("").to_string()
    } else if let Some(pos) = upper.find("DISPLAY#") {
        let after = &pnp_id[pos + 8..];
        after.split('#').next().unwrap_or("").to_string()
    } else if let Some(pos) = upper.find("DISPLAY\\") {
        let after = &pnp_id[pos + 8..];
        after.split('\\').next().unwrap_or("").to_string()
    } else {
        return None;
    };

    if hw_id.is_empty() { return None; }

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let enum_display = hklm.open_subkey_with_flags(
        format!("SYSTEM\\CurrentControlSet\\Enum\\DISPLAY\\{}", hw_id),
        KEY_READ,
    ).ok()?;

    for instance in enum_display.enum_keys().flatten() {
        let instance_key = enum_display.open_subkey_with_flags(&instance, KEY_READ).ok()?;
        let params = instance_key.open_subkey_with_flags("Device Parameters", KEY_READ).ok()?;
        let edid: Vec<u8> = params.get_raw_value("EDID").ok()?.bytes;
        if let Some(name) = parse_edid_monitor_name(&edid) {
            return Some(name);
        }
    }
    None
}

const DISPLAY_DEVICE_PRIMARY_DEVICE: u32 = 0x00000004;
const DISPLAY_DEVICE_ACTIVE: u32 = 0x00000001;
const CLASS_MONITOR: u32 = 0x6D6E7472; // 'mntr'

#[repr(C)]
#[allow(non_snake_case)]
struct DISPLAY_DEVICEW {
    cb: u32,
    DeviceName: [u16; 32],
    DeviceString: [u16; 128],
    StateFlags: u32,
    DeviceID: [u16; 128],
    DeviceKey: [u16; 128],
}

impl Default for DISPLAY_DEVICEW {
    fn default() -> Self {
        Self {
            cb: std::mem::size_of::<Self>() as u32,
            DeviceName: [0; 32],
            DeviceString: [0; 128],
            StateFlags: 0,
            DeviceID: [0; 128],
            DeviceKey: [0; 128],
        }
    }
}

fn u16_to_str(arr: &[u16]) -> String {
    let len = arr.iter().position(|&c| c == 0).unwrap_or(arr.len());
    String::from_utf16_lossy(&arr[..len])
}

fn active_monitor_pnp(monitors: impl IntoIterator<Item = (u32, String)>) -> String {
    // 同一输出下可能残留未启用的旧屏；枚举第 0 项不一定是当前活动监视器。
    monitors.into_iter()
        .find(|(flags, _)| flags & DISPLAY_DEVICE_ACTIVE != 0)
        .map(|(_, pnp)| pnp)
        .unwrap_or_default()
}

#[cfg(test)]
mod active_monitor_tests {
    use super::active_monitor_pnp;

    #[test]
    fn skips_inactive_monitor_before_active_monitor() {
        assert_eq!(
            active_monitor_pnp([
                (2, r"MONITOR\AOC2730\old".to_string()),
                (3, r"MONITOR\AOCB453\current".to_string()),
            ]),
            r"MONITOR\AOCB453\current",
        );
    }

    #[test]
    fn keeps_active_first_monitor() {
        assert_eq!(
            active_monitor_pnp([(1, "current".into()), (2, "old".into())]),
            "current",
        );
    }

    #[test]
    fn does_not_fall_back_to_inactive_monitor() {
        assert_eq!(active_monitor_pnp([(2, "old".into())]), "");
    }

    #[test]
    fn accepts_empty_monitor_list() {
        assert_eq!(active_monitor_pnp([]), "");
    }

    #[test]
    #[ignore = "requires an active AMD primary display; only reads driver state"]
    fn live_primary_amd_saturation_probe() {
        let monitor = super::enumerate_display_monitors().into_iter()
            .find(|monitor| monitor.is_primary).expect("active primary monitor");
        println!("monitor={monitor:?}");
        let (min, max, default, current) = crate::amd::get_saturation_info(&monitor.device_id)
            .expect("primary AMD display saturation is supported");
        println!("AMD saturation min={min} max={max} default={default} current={current}");
        assert!(max > min && (min..=max).contains(&current));
        crate::amd::shutdown();
    }
}

/// 枚举当前活动显示器（无日志、可高频调用）。
/// 显示器拓扑监听线程（display_watch）与 get_display_monitors 命令共用。
pub fn enumerate_display_monitors() -> Vec<DisplayMonitor> {
    let mut monitors = Vec::new();

    unsafe {
        let Ok(user32) = windows::Win32::System::LibraryLoader::LoadLibraryW(windows::core::w!("user32.dll")) else {
            eprintln!("ICC: Failed to load user32.dll for monitor enumeration");
            return monitors;
        };

        type FnEnumDisplayDevicesW = unsafe extern "system" fn(*const u16, u32, *mut DISPLAY_DEVICEW, u32) -> i32;
        if let Some(proc_addr) =
            windows::Win32::System::LibraryLoader::GetProcAddress(user32, windows::core::s!("EnumDisplayDevicesW"))
        {
            let enum_fn: FnEnumDisplayDevicesW = std::mem::transmute(proc_addr);

            let mut adapter_idx = 0;
            let mut adapter: DISPLAY_DEVICEW = Default::default();

            while enum_fn(std::ptr::null(), adapter_idx, &mut adapter, 0) != 0 {
                let is_primary = adapter.StateFlags & DISPLAY_DEVICE_PRIMARY_DEVICE != 0;
                let adapter_name_str = u16_to_str(&adapter.DeviceName);

                if adapter.StateFlags & DISPLAY_DEVICE_ACTIVE != 0 {
                    // 用不带 EDD flag 的第二层枚举拿 DeviceID（PnP 路径）
                    let pnp_id = active_monitor_pnp((0..).map_while(|index| {
                        let mut mon: DISPLAY_DEVICEW = Default::default();
                        (enum_fn(adapter.DeviceName.as_ptr(), index, &mut mon, 0) != 0)
                            .then(|| (mon.StateFlags, u16_to_str(&mon.DeviceID)))
                    }));

                    let model = get_monitor_name_from_edid(&pnp_id)
                        .unwrap_or_else(|| u16_to_str(&adapter.DeviceString));

                    let display_num: String = adapter_name_str
                        .chars()
                        .rev()
                        .take_while(|c| c.is_ascii_digit())
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect();

                    let label = if display_num.is_empty() {
                        model
                    } else {
                        format!("显示器 {} {}", display_num, model)
                    };

                    monitors.push(DisplayMonitor {
                        name: label,
                        device_id: adapter_name_str,
                        pnp_id,
                        is_primary,
                    });
                }

                adapter_idx += 1;
                adapter = Default::default();
            }
        } else {
            eprintln!("ICC: EnumDisplayDevicesW not found");
        }

        let _ = windows::Win32::Foundation::FreeLibrary(user32);
    }

    monitors
}

#[tauri::command]
pub fn get_display_monitors() -> Result<Vec<DisplayMonitor>, String> {
    let monitors = enumerate_display_monitors();
    eprintln!("ICC: Found {} monitors", monitors.len());
    Ok(monitors)
}

fn resolve_device_pnp(target: Option<String>) -> Result<String, String> {
    let monitors = get_display_monitors()?;

    if let Some(target_id) = target {
        // 用户传的是 device_id (如 \\.\DISPLAY1)，找到对应的 pnp_id
        if let Some(m) = monitors.iter().find(|m| m.device_id == target_id) {
            return Ok(m.pnp_id.clone());
        }
        if let Some(m) = monitors.iter().find(|m| m.pnp_id == target_id) {
            return Ok(m.pnp_id.clone());
        }
        return Err(format!("Monitor '{}' not found", target_id));
    }

    if let Some(m) = monitors.iter().find(|m| m.is_primary) {
        return Ok(m.pnp_id.clone());
    }

    monitors.first()
        .map(|m| m.pnp_id.clone())
        .ok_or("No monitors found".to_string())
}

// ---------- 显示模式（分辨率）枚举与切换 ----------
//
// 与 get_display_monitors 相同的动态加载模式：user32 的 EnumDisplaySettingsExW /
// ChangeDisplaySettingsExW 不通过 windows crate 静态链接，避免引入
// Win32_Graphics_Display feature。DEVMODEW 按 MSDN 布局手写（联合体取显示器分支：
// POINTL + 2×DWORD 与打印机分支 8×SHORT 大小对齐一致）。

const ENUM_CURRENT_SETTINGS: u32 = 0xFFFF_FFFF;
const DM_PELSWIDTH: u32 = 0x0008_0000;
const DM_PELSHEIGHT: u32 = 0x0010_0000;
const DM_DISPLAYFREQUENCY: u32 = 0x0040_0000;
const CDS_UPDATEREGISTRY: u32 = 0x0000_0001;
const CDS_TEST: u32 = 0x0000_0002;
const DM_SPECVERSION: u16 = 0x0400;

#[repr(C)]
#[allow(non_snake_case)]
#[allow(clippy::upper_case_acronyms)]
struct DEVMODEW {
    dmDeviceName: [u16; 32],
    dmSpecVersion: u16,
    dmDriverVersion: u16,
    dmSize: u16,
    dmDriverExtra: u16,
    dmFields: u32,
    dmPosition: windows::Win32::Foundation::POINTL,
    dmDisplayOrientation: u32,
    dmDisplayFixedOutput: u32,
    dmColor: i16,
    dmDuplex: i16,
    dmYResolution: i16,
    dmTTOption: i16,
    dmCollate: i16,
    dmFormName: [u16; 32],
    dmLogPixels: u16,
    dmBitsPerPel: u32,
    dmPelsWidth: u32,
    dmPelsHeight: u32,
    dmDisplayFlags: u32,
    dmDisplayFrequency: u32,
    dmICMMethod: u32,
    dmICMIntent: u32,
    dmMediaType: u32,
    dmDitherType: u32,
    dmReserved1: u32,
    dmReserved2: u32,
    dmPanningWidth: u32,
    dmPanningHeight: u32,
}

impl Default for DEVMODEW {
    fn default() -> Self {
        Self {
            dmDeviceName: [0; 32],
            dmSpecVersion: 0,
            dmDriverVersion: 0,
            dmSize: std::mem::size_of::<Self>() as u16,
            dmDriverExtra: 0,
            dmFields: 0,
            dmPosition: windows::Win32::Foundation::POINTL { x: 0, y: 0 },
            dmDisplayOrientation: 0,
            dmDisplayFixedOutput: 0,
            dmColor: 0,
            dmDuplex: 0,
            dmYResolution: 0,
            dmTTOption: 0,
            dmCollate: 0,
            dmFormName: [0; 32],
            dmLogPixels: 0,
            dmBitsPerPel: 0,
            dmPelsWidth: 0,
            dmPelsHeight: 0,
            dmDisplayFlags: 0,
            dmDisplayFrequency: 0,
            dmICMMethod: 0,
            dmICMIntent: 0,
            dmMediaType: 0,
            dmDitherType: 0,
            dmReserved1: 0,
            dmReserved2: 0,
            dmPanningWidth: 0,
            dmPanningHeight: 0,
        }
    }
}

type FnEnumDisplaySettingsExW = unsafe extern "system" fn(*const u16, u32, *mut DEVMODEW, u32) -> i32;
type FnChangeDisplaySettingsExW =
    unsafe extern "system" fn(*const u16, *mut DEVMODEW, *mut std::ffi::c_void, u32, *mut std::ffi::c_void) -> i32;

/// 校验目标显示器并返回其适配器设备名（\\.\DISPLAYn）。
/// device_id 为空时回退主显示器，与 resolve_device_pnp 的语义保持一致。
fn resolve_display_device_name(device_id: Option<String>) -> Result<String, String> {
    let monitors = get_display_monitors()?;
    if let Some(id) = device_id.filter(|s| !s.is_empty()) {
        if let Some(m) = monitors.iter().find(|m| m.device_id == id) {
            return Ok(m.device_id.clone());
        }
        return Err(format!("Monitor '{}' not found", id));
    }
    if let Some(m) = monitors.iter().find(|m| m.is_primary) {
        return Ok(m.device_id.clone());
    }
    monitors
        .first()
        .map(|m| m.device_id.clone())
        .ok_or_else(|| "No monitors found".to_string())
}

fn wide_c_string(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn disp_change_error(code: i32) -> String {
    match code {
        -1 => "Display driver failed the specified graphics mode".to_string(),
        -2 => "Graphics mode not supported".to_string(),
        -3 => "Unable to write settings to registry (applied for this session)".to_string(),
        -4 => "Invalid flags combination".to_string(),
        -5 => "Invalid parameter".to_string(),
        -6 => "Dual view mode not supported".to_string(),
        1 => "Applied, but requires a computer restart".to_string(),
        other => format!("ChangeDisplaySettingsExW failed: {}", other),
    }
}

#[derive(Debug, Serialize)]
pub struct DisplayModeInfo {
    pub width: u32,
    pub height: u32,
    /// 该分辨率下驱动提供的全部刷新率，降序
    pub refresh_rates: Vec<u32>,
    pub is_current: bool,
}

#[derive(Debug, Serialize)]
pub struct CurrentDisplayMode {
    pub width: u32,
    pub height: u32,
    pub refresh_rate: u32,
}

#[derive(Debug, Serialize)]
pub struct DisplayModeList {
    pub modes: Vec<DisplayModeInfo>,
    /// 当前正在使用的模式（枚举失败时为 null）
    pub current: Option<CurrentDisplayMode>,
}

/// 枚举指定显示器支持的显示模式，按分辨率去重（同分辨率聚合全部刷新率），
/// 面积降序排列，并携带当前正在使用的分辨率与刷新率。
#[tauri::command]
pub fn get_display_modes(device_id: Option<String>) -> Result<DisplayModeList, String> {
    let device_name = resolve_display_device_name(device_id)?;
    let device_wide = wide_c_string(&device_name);

    unsafe {
        let user32 = windows::Win32::System::LibraryLoader::LoadLibraryW(windows::core::w!("user32.dll"))
            .map_err(|e| format!("Failed to load user32.dll: {}", e))?;
        let result = (|| -> Result<DisplayModeList, String> {
            let enum_fn: FnEnumDisplaySettingsExW = std::mem::transmute(
                windows::Win32::System::LibraryLoader::GetProcAddress(user32, windows::core::s!("EnumDisplaySettingsExW"))
                    .ok_or("EnumDisplaySettingsExW not found")?,
            );

            let mut current: DEVMODEW = Default::default();
            let has_current =
                enum_fn(device_wide.as_ptr(), ENUM_CURRENT_SETTINGS, &mut current, 0) != 0;

            // (宽, 高) -> 该分辨率下全部刷新率
            let mut best: Vec<(u32, u32, Vec<u32>)> = Vec::new();
            for mode_num in 0..1024u32 {
                let mut dm: DEVMODEW = Default::default();
                if enum_fn(device_wide.as_ptr(), mode_num, &mut dm, 0) == 0 {
                    break;
                }
                let (w, h, freq) = (dm.dmPelsWidth, dm.dmPelsHeight, dm.dmDisplayFrequency);
                if w == 0 || h == 0 || freq == 0 {
                    continue;
                }
                match best.iter_mut().find(|(bw, bh, _)| *bw == w && *bh == h) {
                    Some((_, _, rates)) => {
                        if !rates.contains(&freq) {
                            rates.push(freq);
                        }
                    }
                    None => best.push((w, h, vec![freq])),
                }
            }

            best.sort_by_key(|(w, h, _)| std::cmp::Reverse(*w as u64 * *h as u64));
            let modes = best
                .into_iter()
                .map(|(w, h, mut rates)| {
                    rates.sort_unstable_by(|a, b| b.cmp(a));
                    DisplayModeInfo {
                        width: w,
                        height: h,
                        refresh_rates: rates,
                        is_current: has_current && current.dmPelsWidth == w && current.dmPelsHeight == h,
                    }
                })
                .collect();

            let current_info = if has_current {
                Some(CurrentDisplayMode {
                    width: current.dmPelsWidth,
                    height: current.dmPelsHeight,
                    refresh_rate: current.dmDisplayFrequency,
                })
            } else {
                None
            };
            Ok(DisplayModeList { modes, current: current_info })
        })();

        let _ = windows::Win32::Foundation::FreeLibrary(user32);
        result
    }
}

/// 切换指定显示器分辨率的实际执行（无门闩）。调用方必须已持颜色写门闩：
/// 手动命令入口负责加闩并递增代号（显示模式切换同样可能重置曲线、使旧恢复
/// 所有权失效）；方案应用路径在整组应用的门闩作用域内调用本函数。
/// refresh_rate 传 Some(hz) 时精确指定刷新率（驱动枚举里不存在则报错）；None 时
/// 优先保持当前值（目标分辨率支持时），否则取该分辨率下的最高刷新率。
/// 先 CDS_TEST 验证再正式应用。
pub(crate) fn set_display_mode_unlocked(
    device_id: Option<String>,
    width: u32,
    height: u32,
    refresh_rate: Option<u32>,
) -> Result<(), String> {
    let device_name = resolve_display_device_name(device_id)?;
    let device_wide = wide_c_string(&device_name);

    unsafe {
        let user32 = windows::Win32::System::LibraryLoader::LoadLibraryW(windows::core::w!("user32.dll"))
            .map_err(|e| format!("Failed to load user32.dll: {}", e))?;
        let result = (|| -> Result<(), String> {
            let enum_fn: FnEnumDisplaySettingsExW = std::mem::transmute(
                windows::Win32::System::LibraryLoader::GetProcAddress(user32, windows::core::s!("EnumDisplaySettingsExW"))
                    .ok_or("EnumDisplaySettingsExW not found")?,
            );
            let change_fn: FnChangeDisplaySettingsExW = std::mem::transmute(
                windows::Win32::System::LibraryLoader::GetProcAddress(user32, windows::core::s!("ChangeDisplaySettingsExW"))
                    .ok_or("ChangeDisplaySettingsExW not found")?,
            );

            let mut current: DEVMODEW = Default::default();
            let current_freq = if enum_fn(device_wide.as_ptr(), ENUM_CURRENT_SETTINGS, &mut current, 0) != 0 {
                current.dmDisplayFrequency
            } else {
                0
            };

            // 在目标分辨率的可用模式里挑刷新率：精确指定优先，其次保持当前值，退而求最高
            let mut best_freq = 0u32;
            let mut exact_freq = 0u32;
            let mut requested_found = false;
            for mode_num in 0..1024u32 {
                let mut dm: DEVMODEW = Default::default();
                if enum_fn(device_wide.as_ptr(), mode_num, &mut dm, 0) == 0 {
                    break;
                }
                if dm.dmPelsWidth != width || dm.dmPelsHeight != height {
                    continue;
                }
                let freq = dm.dmDisplayFrequency;
                if freq > best_freq {
                    best_freq = freq;
                }
                if freq == current_freq {
                    exact_freq = freq;
                }
                if Some(freq) == refresh_rate {
                    requested_found = true;
                }
            }
            if best_freq == 0 {
                return Err(format!("Display mode {}x{} not supported", width, height));
            }
            let target_freq = if let Some(hz) = refresh_rate {
                if !requested_found {
                    return Err(format!(
                        "Refresh rate {} Hz not supported at {}x{}",
                        hz, width, height
                    ));
                }
                hz
            } else if exact_freq > 0 {
                exact_freq
            } else {
                best_freq
            };

            let mut dm: DEVMODEW = DEVMODEW {
                dmSpecVersion: DM_SPECVERSION,
                dmFields: DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY,
                dmPelsWidth: width,
                dmPelsHeight: height,
                dmDisplayFrequency: target_freq,
                ..Default::default()
            };

            let test = change_fn(device_wide.as_ptr(), &mut dm, std::ptr::null_mut(), CDS_TEST, std::ptr::null_mut());
            if test != 0 {
                return Err(disp_change_error(test));
            }

            let applied = change_fn(
                device_wide.as_ptr(),
                &mut dm,
                std::ptr::null_mut(),
                CDS_UPDATEREGISTRY,
                std::ptr::null_mut(),
            );
            match applied {
                0 => Ok(()),
                // -3 (DISP_CHANGE_NOTUPDATED)：模式已动态生效，仅注册表写入失败，对用户等同成功
                -3 => Ok(()),
                other => Err(disp_change_error(other)),
            }
        })();

        let _ = windows::Win32::Foundation::FreeLibrary(user32);
        result
    }
}

/// 手动显示模式命令：进入颜色写门闩并递增代号，使旧恢复所有权失效
/// （模式切换可能重置 gamma 曲线）。
#[tauri::command]
pub fn set_display_mode(
    device_id: Option<String>,
    width: u32,
    height: u32,
    refresh_rate: Option<u32>,
) -> Result<(), String> {
    let _latch = crate::nvidia::begin_external_write();
    set_display_mode_unlocked(device_id, width, height, refresh_rate)
}

/// 全局唯一的「分辨率还原点」：本应用把某台显示器从原始模式切走时记录
/// (设备名, 宽, 高, 刷新率)，只在第一次真正切换成功时提交（切换失败不写入，
/// 避免污染既有还原点）；应用不带分辨率的方案时自动还原，恢复失败保留以便
/// 重试。Mutex 保并发安全——托盘/快捷键/进程监听/前端可能同时走应用路径，
/// 锁在切换期间保持持有，与旧实现的串行化行为一致。
static RESOLUTION_UNDO: std::sync::Mutex<Option<(String, u32, u32, u32)>> = std::sync::Mutex::new(None);

/// 解析方案里的 "宽x高" 或 "宽x高@刷新率" 字符串，容忍空格、大写 X 和 ×
fn parse_resolution(s: &str) -> Option<(u32, u32, Option<u32>)> {
    let mut parts = s.split(['x', 'X', '×']);
    let w: u32 = parts.next()?.trim().parse().ok()?;
    let rest = parts.next()?;
    let (h_str, hz_str) = match rest.split_once('@') {
        Some((h, r)) => (h, Some(r)),
        None => (rest, None),
    };
    let h: u32 = h_str.trim().parse().ok()?;
    let hz: Option<u32> = match hz_str {
        Some(r) => Some(r.trim().parse().ok()?),
        None => None,
    };
    (w > 0 && h > 0 && hz.is_none_or(|v| v > 0)).then_some((w, h, hz))
}

/// 方案应用路径的结构化结果：把「跳过」从错误里拆出来，调用方能准确区分
/// 「部分成功（颜色继续应用、分辨率保留）」与「真实失败」。
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PresetResolutionResult {
    /// 目标与当前模式一致，未做任何切换
    Unchanged,
    /// 目标模式切换成功
    Applied,
    /// `resolution: null` 时把此前的还原点恢复成功
    Restored,
    /// 显示器不支持方案请求的模式：整次切换跳过，保留当前模式与既有还原点
    SkippedUnsupported { requested: String, reason: String },
    /// 方案里的分辨率字段格式非法：忽略本次分辨率调节
    SkippedInvalid { requested: String },
}

/// 判断方案请求的模式是否被当前显示器支持：当前模式本身算数（枚举缺失时
/// 仍可判定「已在目标模式」），显式指定的刷新率必须命中该分辨率下的某个
/// 可用值——不支持就跳过整次切换，不擅自降级分辨率或刷新率。
fn preset_mode_supported(
    list: &DisplayModeList,
    width: u32,
    height: u32,
    refresh_rate: Option<u32>,
) -> bool {
    let matches = |w, h, hz| {
        w == width && h == height && refresh_rate.is_none_or(|target| target == hz)
    };
    list.current.as_ref().is_some_and(|c| matches(c.width, c.height, c.refresh_rate))
        || list.modes.iter().any(|m| {
            m.width == width && m.height == height
                && m.refresh_rates.iter().any(|hz| *hz > 0 && matches(m.width, m.height, *hz))
        })
}

/// 与 set_display_mode 一致的不支持文案：手动入口把 skipped_unsupported 映射
/// 回 Err 时沿用，保证两个入口对外暴露的错误信息相同。
fn preset_unsupported_reason(
    modes: &DisplayModeList,
    width: u32,
    height: u32,
    refresh_rate: Option<u32>,
) -> String {
    let resolution_known = modes
        .modes
        .iter()
        .any(|m| m.width == width && m.height == height);
    match refresh_rate {
        Some(hz) if resolution_known => {
            format!("Refresh rate {} Hz not supported at {}x{}", hz, width, height)
        }
        _ => format!("Display mode {}x{} not supported", width, height),
    }
}

/// 把目标模式格式化成人可读的请求描述（结构化结果里的 requested 字段）
fn format_requested_mode(width: u32, height: u32, refresh_rate: Option<u32>) -> String {
    match refresh_rate {
        Some(hz) => format!("{}x{}@{}Hz", width, height, hz),
        None => format!("{}x{}", width, height),
    }
}

/// 还原点事务：执行一次真正的模式切换。
///
/// 首次真正切换成功后才把切换前的模式提交为还原点；切换失败（`Err`）时既有
/// 还原点原样保留——旧实现在切换前登记，失败会留下指向污染模式的还原点。
/// `switch` 注入以便离屏测试；锁在切换期间保持持有，串行化并发应用路径。
fn commit_resolution_switch(
    device_name: &str,
    current: (u32, u32, u32),
    target: (u32, u32, Option<u32>),
    switch: &mut dyn FnMut(u32, u32, Option<u32>) -> Result<(), String>,
) -> Result<PresetResolutionResult, String> {
    let mut undo = RESOLUTION_UNDO.lock().unwrap();
    let had_undo = undo.is_some();
    switch(target.0, target.1, target.2)?;
    if !had_undo {
        *undo = Some((device_name.to_string(), current.0, current.1, current.2));
    }
    // 还原点恰为目标模式（本次切换等于归位）时清除；Hz 缺省按切换前值比对
    if *undo
        == Some((
            device_name.to_string(),
            target.0,
            target.1,
            target.2.unwrap_or(current.2),
        ))
    {
        *undo = None;
    }
    Ok(PresetResolutionResult::Applied)
}

/// 还原点事务：应用不带分辨率的方案时，把本应用此前改走的模式还原回去。
///
/// 恢复失败时保留还原点（下一次应用无分辨率方案时可重试）并返回 `Err`；
/// 已在还原点模式（例如目标方案本身把模式带了回去）则直接清除。
fn commit_resolution_restore(
    device_name: &str,
    current: (u32, u32, u32),
    switch: &mut dyn FnMut(u32, u32, Option<u32>) -> Result<(), String>,
) -> Result<PresetResolutionResult, String> {
    let mut undo = RESOLUTION_UNDO.lock().unwrap();
    match undo.clone() {
        Some((dev, uw, uh, uhz)) if dev == device_name && (uw, uh, uhz) != current => {
            switch(uw, uh, Some(uhz))?;
            *undo = None;
            Ok(PresetResolutionResult::Restored)
        }
        Some((dev, _, _, _)) if dev == device_name => {
            // 已归位：无需切换，仅清除还原点
            *undo = None;
            Ok(PresetResolutionResult::Unchanged)
        }
        _ => Ok(PresetResolutionResult::Unchanged),
    }
}

/// 方案分辨率的核心状态机：设备名、模式枚举结果、目标模式与切换函数全部
/// 注入，纯逻辑、可离屏测试。判定顺序：
///
/// 1. 无目标（`None`）→ 还原事务；
/// 2. 目标与当前一致 → `Unchanged`（还原点已归位时顺手清除）；
/// 3. 目标不被支持 → `SkippedUnsupported`（跳过整次切换，不降级分辨率或
///    刷新率，不动还原点）；
/// 4. 否则执行切换事务 → `Applied` / 真实错误。
fn apply_preset_resolution_core(
    device_name: &str,
    modes: &DisplayModeList,
    target: Option<(u32, u32, Option<u32>)>,
    switch: &mut dyn FnMut(u32, u32, Option<u32>) -> Result<(), String>,
) -> Result<PresetResolutionResult, String> {
    let current = modes
        .current
        .as_ref()
        .map(|c| (c.width, c.height, c.refresh_rate))
        .ok_or_else(|| "Cannot determine current display mode".to_string())?;

    match target {
        None => commit_resolution_restore(device_name, current, switch),
        Some((w, h, hz)) => {
            // 与当前模式一致（Hz 缺省视为不关心）则不动，仅在已归位时清还原点
            if w == current.0 && h == current.1 && hz.is_none_or(|v| v == current.2) {
                let mut undo = RESOLUTION_UNDO.lock().unwrap();
                if *undo == Some((device_name.to_string(), current.0, current.1, current.2)) {
                    *undo = None;
                }
                return Ok(PresetResolutionResult::Unchanged);
            }
            if !preset_mode_supported(modes, w, h, hz) {
                let reason = preset_unsupported_reason(modes, w, h, hz);
                eprintln!(
                    "[apply] preset resolution skipped (unsupported): {} on {}: {}",
                    format_requested_mode(w, h, hz),
                    device_name,
                    reason
                );
                return Ok(PresetResolutionResult::SkippedUnsupported {
                    requested: format_requested_mode(w, h, hz),
                    reason,
                });
            }
            commit_resolution_switch(device_name, current, (w, h, hz), switch)
        }
    }
}

/// 方案应用路径的分辨率落地（前端加载方案与后端托盘/快捷键/进程监听共用），
/// 返回结构化结果供调用方区分「部分成功（跳过）」与「真实失败」。
///
/// - `Some("宽x高[@Hz]")`：格式非法 → `SkippedInvalid`；与当前模式相同 →
///   `Unchanged`；显示器不支持 → `SkippedUnsupported`（保留当前模式与既有
///   还原点；明确指定的刷新率不被支持时同样跳过，不擅自降级）；需要切换时
///   走还原点事务，首次真正切换成功才提交还原点 → `Applied`。
/// - `None`：方案不调节分辨率，但若还原点属于该显示器且当前模式已被改走，
///   则还原并清除 → `Restored`——托盘从 4:3 方案切回旧方案时分辨率跟着回去。
///
/// 设备不存在、模式枚举失败、驱动实际拒绝切换仍返回 `Err`；`Err` 不应阻断
/// 方案里其余配色参数的应用，由调用方决定容错策略。
pub fn apply_preset_resolution_state(
    device_id: Option<String>,
    resolution: Option<&str>,
) -> Result<PresetResolutionResult, String> {
    let device_name = resolve_display_device_name(device_id)?;
    let target = match resolution {
        None => None,
        Some(s) => match parse_resolution(s) {
            Some(t) => Some(t),
            None => {
                eprintln!("[apply] invalid resolution in config: '{}'", s);
                return Ok(PresetResolutionResult::SkippedInvalid {
                    requested: s.to_string(),
                });
            }
        },
    };

    let modes = get_display_modes(Some(device_name.clone()))?;
    apply_preset_resolution_core(&device_name, &modes, target, &mut |w, h, hz| {
        set_display_mode_unlocked(Some(device_name.clone()), w, h, hz)
    })
}

/// Tauri 命令：方案加载的兼容分辨率入口。与 apply_config_resolution 共用同一份
/// 还原点事务；区别仅在于不兼容的目标模式以 skipped_unsupported 结构化回报
/// 而不是 Err，前端据此提示「颜色已应用、分辨率已保留」的部分成功。
/// 前端手动调用属于外部写入：持颜色写门闩并递增代号（方案应用路径由
/// apply_color_config_from 持闩调用 apply_preset_resolution_state，不经此处）。
#[tauri::command]
pub fn apply_preset_resolution(
    device_id: Option<String>,
    resolution: Option<String>,
) -> Result<PresetResolutionResult, String> {
    let _latch = crate::nvidia::begin_external_write();
    apply_preset_resolution_state(device_id, resolution.as_deref())
}

/// Tauri 命令：旧版方案分辨率入口（严格报错语义保持不变）。与新入口共用同一
/// 份还原点事务；不支持的目标模式仍按原有错误文案返回 Err，既有调用方的
/// 错误提示不回退。
#[tauri::command]
pub fn apply_config_resolution(device_id: Option<String>, resolution: Option<String>) -> Result<(), String> {
    let _latch = crate::nvidia::begin_external_write();
    match apply_preset_resolution_state(device_id, resolution.as_deref())? {
        // 旧行为保持：格式非法按忽略处理（已在状态函数里留痕）
        PresetResolutionResult::SkippedUnsupported { reason, .. } => Err(reason),
        _ => Ok(()),
    }
}

/// 方案分辨率兼容策略的纯逻辑回归测试（不接触模式切换 API）
#[cfg(test)]
mod preset_resolution_tests {
    use super::*;

    #[test]
    fn unsupported_resolution_or_explicit_refresh_is_rejected() {
        let list = DisplayModeList {
            modes: vec![DisplayModeInfo {
                width: 1920, height: 1080,
                refresh_rates: vec![240, 144, 60], is_current: true,
            }],
            current: Some(CurrentDisplayMode {
                width: 1920, height: 1080, refresh_rate: 240,
            }),
        };
        assert!(!preset_mode_supported(&list, 2560, 1440, None));
        assert!(!preset_mode_supported(&list, 1920, 1080, Some(280)));
        assert!(preset_mode_supported(&list, 1920, 1080, Some(144)));
        assert!(preset_mode_supported(&list, 1920, 1080, None));
    }

    #[test]
    fn current_mode_is_valid_even_if_missing_from_enumeration() {
        let list = DisplayModeList {
            modes: vec![],
            current: Some(CurrentDisplayMode {
                width: 1920, height: 1080, refresh_rate: 240,
            }),
        };
        assert!(preset_mode_supported(&list, 1920, 1080, Some(240)));
        assert!(!preset_mode_supported(&list, 1920, 1080, Some(144)));
    }
}

/// 还原点事务回归测试：注入假切换函数，不接触真实屏幕。
/// 全局还原点是进程级单例，用例间以串行锁互斥，避免并行测试互相踩状态。
#[cfg(test)]
mod resolution_transaction_tests {
    use super::*;

    /// 测试间串行化（还原点是全局状态）
    fn undo_test_guard() -> std::sync::MutexGuard<'static, ()> {
        static UNDO_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        UNDO_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn set_undo(value: Option<(String, u32, u32, u32)>) {
        *RESOLUTION_UNDO.lock().unwrap() = value;
    }

    fn undo_snapshot() -> Option<(String, u32, u32, u32)> {
        RESOLUTION_UNDO.lock().unwrap().clone()
    }

    fn undo_of(device: &str, w: u32, h: u32, hz: u32) -> Option<(String, u32, u32, u32)> {
        Some((device.to_string(), w, h, hz))
    }

    /// 首次切换失败：不得创建还原点（旧实现在切换前登记，失败会留下污染）
    #[test]
    fn first_failed_switch_does_not_create_undo_point() {
        let _guard = undo_test_guard();
        set_undo(None);

        let mut switch = |_w: u32, _h: u32, _hz: Option<u32>| -> Result<(), String> {
            Err("Display mode 2560x1440 not supported".to_string())
        };
        let result = commit_resolution_switch(
            "\\\\.\\DISPLAY1",
            (1920, 1080, 240),
            (2560, 1440, None),
            &mut switch,
        );
        assert!(result.is_err());
        assert_eq!(undo_snapshot(), None, "切换失败不得留下还原点");
    }

    /// 已有还原点时切换失败：既有还原点必须原样保留
    #[test]
    fn existing_undo_point_survives_failed_switch() {
        let _guard = undo_test_guard();
        set_undo(undo_of("\\\\.\\DISPLAY1", 1920, 1080, 240));

        let mut switch = |_w: u32, _h: u32, _hz: Option<u32>| -> Result<(), String> {
            Err("Display mode 3840x2160 not supported".to_string())
        };
        let result = commit_resolution_switch(
            "\\\\.\\DISPLAY1",
            (1024, 768, 60), // 此前已被本应用切到 4:3
            (3840, 2160, None),
            &mut switch,
        );
        assert!(result.is_err());
        assert_eq!(
            undo_snapshot(),
            undo_of("\\\\.\\DISPLAY1", 1920, 1080, 240),
            "既有还原点不得被失败的切换覆盖或清除"
        );
    }

    /// 切换成功：首次真正成功后才提交原模式为还原点；随后恢复成功并清除还原点
    #[test]
    fn successful_switch_commits_undo_and_restore_clears_it() {
        let _guard = undo_test_guard();
        set_undo(None);

        let mut switch = |_w: u32, _h: u32, _hz: Option<u32>| -> Result<(), String> { Ok(()) };
        let applied = commit_resolution_switch(
            "\\\\.\\DISPLAY1",
            (1920, 1080, 240),
            (1024, 768, None),
            &mut switch,
        )
        .expect("switch should succeed");
        assert!(matches!(applied, PresetResolutionResult::Applied));
        assert_eq!(
            undo_snapshot(),
            undo_of("\\\\.\\DISPLAY1", 1920, 1080, 240),
            "切换成功后还原点应记录切换前的完整模式"
        );

        // 注入的恢复切换：记录调用参数，验证恢复走的是还原点里的完整模式
        let restore_calls = std::cell::RefCell::new(Vec::new());
        let mut restore = |w: u32, h: u32, hz: Option<u32>| -> Result<(), String> {
            restore_calls.borrow_mut().push((w, h, hz));
            Ok(())
        };
        let restored =
            commit_resolution_restore("\\\\.\\DISPLAY1", (1024, 768, 60), &mut restore)
                .expect("restore should succeed");
        assert!(matches!(restored, PresetResolutionResult::Restored));
        assert_eq!(undo_snapshot(), None, "恢复成功后还原点应被清除");
        assert_eq!(restore_calls.into_inner(), vec![(1920, 1080, Some(240))]);
    }

    /// 恢复失败：还原点保留，重试可成功
    #[test]
    fn failed_restore_keeps_undo_point_for_retry() {
        let _guard = undo_test_guard();
        set_undo(undo_of("\\\\.\\DISPLAY1", 1920, 1080, 240));

        let mut failing = |_w: u32, _h: u32, _hz: Option<u32>| -> Result<(), String> {
            Err("Display driver failed the specified graphics mode".to_string())
        };
        assert!(
            commit_resolution_restore("\\\\.\\DISPLAY1", (1024, 768, 60), &mut failing).is_err()
        );
        assert_eq!(
            undo_snapshot(),
            undo_of("\\\\.\\DISPLAY1", 1920, 1080, 240),
            "恢复失败时还原点必须保留以便重试"
        );

        let mut retry = |_w: u32, _h: u32, _hz: Option<u32>| -> Result<(), String> { Ok(()) };
        let result =
            commit_resolution_restore("\\\\.\\DISPLAY1", (1024, 768, 60), &mut retry)
                .expect("retry should succeed");
        assert!(matches!(result, PresetResolutionResult::Restored));
        assert_eq!(undo_snapshot(), None);
    }

    /// 已成功切换后（还原点存在），应用不支持的方案：不清除既有还原点、不发起切换
    #[test]
    fn unsupported_request_keeps_existing_undo_point() {
        let _guard = undo_test_guard();
        set_undo(undo_of("\\\\.\\DISPLAY1", 1920, 1080, 240));

        // 当前已在本应用切换后的 2560x1440@144，方案请求 3840x2160（枚举里不存在）
        let modes = DisplayModeList {
            modes: vec![DisplayModeInfo {
                width: 2560,
                height: 1440,
                refresh_rates: vec![144, 60],
                is_current: true,
            }],
            current: Some(CurrentDisplayMode {
                width: 2560,
                height: 1440,
                refresh_rate: 144,
            }),
        };
        let calls = std::cell::RefCell::new(Vec::new());
        let mut switch = |w: u32, h: u32, hz: Option<u32>| -> Result<(), String> {
            calls.borrow_mut().push((w, h, hz));
            Ok(())
        };
        let result = apply_preset_resolution_core(
            "\\\\.\\DISPLAY1",
            &modes,
            Some((3840, 2160, None)),
            &mut switch,
        )
        .expect("unsupported preset must be reported, not an error");
        match result {
            PresetResolutionResult::SkippedUnsupported { requested, reason } => {
                assert_eq!(requested, "3840x2160");
                assert!(reason.contains("3840x2160"), "reason 应包含请求的模式：{reason}");
            }
            other => panic!("expected skipped_unsupported, got {other:?}"),
        }
        assert_eq!(
            undo_snapshot(),
            undo_of("\\\\.\\DISPLAY1", 1920, 1080, 240),
            "跳过不支持方案不得清除既有还原点"
        );
        assert!(calls.into_inner().is_empty(), "不支持时不得发起切换");
    }

    /// 目标与当前一致：Unchanged；还原点已归位时清除，否则保留
    #[test]
    fn unchanged_target_clears_undo_only_when_restored() {
        let _guard = undo_test_guard();
        let modes = DisplayModeList {
            modes: vec![DisplayModeInfo {
                width: 1920,
                height: 1080,
                refresh_rates: vec![240],
                is_current: true,
            }],
            current: Some(CurrentDisplayMode {
                width: 1920,
                height: 1080,
                refresh_rate: 240,
            }),
        };
        let mut switch = |_w: u32, _h: u32, _hz: Option<u32>| -> Result<(), String> { Ok(()) };

        // 还原点与当前模式一致（已归位）→ 应用相同模式应清除还原点
        set_undo(undo_of("\\\\.\\DISPLAY1", 1920, 1080, 240));
        let result = apply_preset_resolution_core(
            "\\\\.\\DISPLAY1",
            &modes,
            Some((1920, 1080, None)),
            &mut switch,
        )
        .expect("same mode should not fail");
        assert!(matches!(result, PresetResolutionResult::Unchanged));
        assert_eq!(undo_snapshot(), None, "已归位时应清除还原点");

        // 还原点与当前模式不一致（还停在 4:3）→ 应用当前模式不得清除还原点
        let modes_43 = DisplayModeList {
            modes: vec![DisplayModeInfo {
                width: 1024,
                height: 768,
                refresh_rates: vec![60],
                is_current: true,
            }],
            current: Some(CurrentDisplayMode {
                width: 1024,
                height: 768,
                refresh_rate: 60,
            }),
        };
        set_undo(undo_of("\\\\.\\DISPLAY1", 1920, 1080, 240));
        let result = apply_preset_resolution_core(
            "\\\\.\\DISPLAY1",
            &modes_43,
            Some((1024, 768, None)),
            &mut switch,
        )
        .expect("same mode should not fail");
        assert!(matches!(result, PresetResolutionResult::Unchanged));
        assert_eq!(
            undo_snapshot(),
            undo_of("\\\\.\\DISPLAY1", 1920, 1080, 240),
            "未归位时还原点必须保留"
        );
    }
}

/// 只读集成测试：枚举主显示器的可用模式，不改动任何系统状态。
#[cfg(test)]
mod display_mode_tests {
    use super::*;

    #[test]
    fn enumerate_primary_display_modes() {
        let list = get_display_modes(None).expect("enumerate display modes");
        assert!(!list.modes.is_empty(), "primary display should expose at least one mode");
        assert!(
            list.modes.iter().any(|m| m.is_current),
            "the current mode should be flagged"
        );
        assert!(
            list.modes
                .iter()
                .all(|m| m.width > 0 && m.height > 0 && !m.refresh_rates.is_empty()),
            "all modes should carry valid dimensions and refresh rates"
        );
        assert!(
            list.modes
                .iter()
                .all(|m| m.refresh_rates.iter().all(|&r| r > 0)),
            "refresh rates should be positive"
        );
        for pair in list.modes.windows(2) {
            assert!(
                pair[0].width as u64 * pair[0].height as u64
                    >= pair[1].width as u64 * pair[1].height as u64,
                "modes should be sorted by area, descending"
            );
        }
        // 刷新率列表降序
        for m in &list.modes {
            for pair in m.refresh_rates.windows(2) {
                assert!(pair[0] > pair[1], "refresh rates should be sorted descending");
            }
        }
        // 当前模式信息应与标记 is_current 的条目一致
        let current = list.current.expect("current mode should be present");
        assert!(
            list.modes
                .iter()
                .any(|m| m.is_current && m.width == current.width && m.height == current.height),
            "current entry should match the flagged mode"
        );
        assert!(
            list.modes
                .iter()
                .find(|m| m.is_current)
                .map(|m| m.refresh_rates.contains(&current.refresh_rate))
                .unwrap_or(false),
            "current refresh rate should be among the current resolution's rates"
        );
    }

    #[test]
    fn parse_resolution_formats() {
        assert_eq!(parse_resolution("1024x768"), Some((1024, 768, None)));
        assert_eq!(parse_resolution("2560X1440"), Some((2560, 1440, None)));
        assert_eq!(parse_resolution(" 1920 × 1080 "), Some((1920, 1080, None)));
        assert_eq!(parse_resolution("1024x768@60"), Some((1024, 768, Some(60))));
        assert_eq!(parse_resolution("2560x1440@ 280"), Some((2560, 1440, Some(280))));
        assert_eq!(parse_resolution("0x1080"), None);
        assert_eq!(parse_resolution("1080"), None);
        assert_eq!(parse_resolution("axb"), None);
        assert_eq!(parse_resolution("1024x768@0"), None);
    }
}

#[tauri::command]
pub fn get_icc_profiles() -> Result<Vec<IccProfile>, String> {
    let color_dir = color_dir();

    if !color_dir.exists() {
        return Err("ICC profile directory not found".to_string());
    }

    let mut profiles = Vec::new();

    if let Ok(entries) = fs::read_dir(color_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if is_icc_ext(&path) {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Unknown")
                    .to_string();

                profiles.push(IccProfile {
                    name,
                    path: path.to_string_lossy().to_string(),
                    is_active: false,
                });
            }
        }
    }

    Ok(profiles)
}

fn read_u16_be(data: &[u8], offset: usize) -> u16 {
    // 越界返回 0，避免解析损坏/截断的 ICC 时直接 panic（这些数据来自用户导入的文件）
    match data.get(offset..offset + 2) {
        Some(b) => u16::from_be_bytes([b[0], b[1]]),
        None => 0,
    }
}

fn read_u32_be(data: &[u8], offset: usize) -> u32 {
    match data.get(offset..offset + 4) {
        Some(b) => u32::from_be_bytes([b[0], b[1], b[2], b[3]]),
        None => 0,
    }
}

/// 从 ICC 文件解析 vcgt 标签，返回 [R256, G256, B256] 的 gamma ramp（每个值 0..=65535）
pub(crate) fn parse_vcgt(icc_data: &[u8]) -> Option<[[u16; 256]; 3]> {
    if icc_data.len() < 132 { return None; }

    // ICC header: tag count at offset 128
    let tag_count = read_u32_be(icc_data, 128) as usize;
    let tag_table_start = 132;

    // 查找 vcgt 标签 (0x76636774)
    let vcgt_sig: u32 = 0x76636774;
    let mut vcgt_offset = 0usize;
    let mut vcgt_size = 0usize;

    for i in 0..tag_count {
        let base = tag_table_start + i * 12;
        if base + 12 > icc_data.len() { break; }
        let sig = read_u32_be(icc_data, base);
        if sig == vcgt_sig {
            vcgt_offset = read_u32_be(icc_data, base + 4) as usize;
            vcgt_size = read_u32_be(icc_data, base + 8) as usize;
            break;
        }
    }

    if vcgt_offset == 0 || vcgt_offset + 12 > icc_data.len() {
        eprintln!("ICC: No vcgt tag found");
        return None;
    }

    let vcgt_end = vcgt_offset.checked_add(vcgt_size)?;
    if vcgt_size < 12 || vcgt_end > icc_data.len() { return None; }
    let vcgt = &icc_data[vcgt_offset..vcgt_end];
    let gamma_type = read_u32_be(vcgt, 8);

    let mut ramp = [[0u16; 256]; 3];

    if gamma_type == 0 {
        // Table type
        if vcgt.len() < 18 { return None; }
        let channels = read_u16_be(vcgt, 12) as usize;
        let entry_count = read_u16_be(vcgt, 14) as usize;
        let entry_size = read_u16_be(vcgt, 16) as usize;

        eprintln!("ICC: vcgt table: channels={} entries={} entry_size={}", channels, entry_count, entry_size);

        if channels < 3 || entry_count == 0 || entry_size == 0 { return None; }
        if entry_size != 1 && entry_size != 2 { return None; }
        // 用 checked 运算算总长，避免超大 entry_count/entry_size 相乘溢出后绕过边界检查
        let need = channels
            .checked_mul(entry_count)
            .and_then(|v| v.checked_mul(entry_size))
            .and_then(|v| v.checked_add(18));
        match need {
            Some(n) if vcgt.len() >= n => {}
            _ => return None,
        }

        for ch in 0..3 {
            for i in 0..256 {
                // 插值到 256 个条目
                let src_idx = i * (entry_count - 1) / 255;
                let src_idx = src_idx.min(entry_count - 1);
                let data_offset = 18 + ch * entry_count * entry_size + src_idx * entry_size;
                let val = if entry_size == 2 {
                    read_u16_be(vcgt, data_offset)
                } else {
                    (vcgt[data_offset] as u16) << 8
                };
                ramp[ch][i] = val;
            }
        }
    } else if gamma_type == 1 {
        // Formula type：每通道 3 个 s15Fixed16 值（各 4 字节，= raw_i32 / 65536），
        // 顺序为 R{gamma,min,max} G{gamma,min,max} B{gamma,min,max}，标签体至少 12 + 9*4 = 48 字节
        if vcgt.len() < 12 + 9 * 4 { return None; }
        eprintln!("ICC: vcgt formula type");
        let read_s15f16 = |off: usize| read_u32_be(vcgt, off) as i32 as f64 / 65536.0;
        for ch in 0..3 {
            let base = 12 + ch * 12;
            let gamma = read_s15f16(base);
            let min = read_s15f16(base + 4);
            let max = read_s15f16(base + 8);
            // 非法参数（gamma<=0 或 max<=min）会算出无意义/全 0 曲线，直接放弃这个 ICC
            if !(gamma > 0.0) || max <= min {
                return None;
            }
            for i in 0..256 {
                let x = i as f64 / 255.0;
                let y = (min + (max - min) * x.powf(gamma)).clamp(0.0, 1.0);
                ramp[ch][i] = (y * 65535.0).round() as u16;
            }
        }
    } else {
        eprintln!("ICC: Unknown vcgt type: {}", gamma_type);
        return None;
    }

    Some(ramp)
}

/// Read the same local/system ICC used by the existing loader, without WCS or
/// display PnP dependency. Absence of VCGT differs from an invalid ICC file.
pub(crate) fn read_profile_curve(profile_path: &str) -> Result<Option<[[u16; 256]; 3]>, String> {
    let source = PathBuf::from(profile_path);
    let name = source.file_name().ok_or("Invalid ICC path")?;
    let installed = color_dir().join(name);
    let path = if installed.exists() { installed } else { source };
    let bytes = fs::read(&path).map_err(|e| format!("Failed to read ICC '{}': {e}", path.display()))?;
    let has_vcgt = crate::icc_validation::validate_profile_tags(&bytes)
        .map_err(|error| format!("Invalid ICC '{}': {error}", path.display()))?;
    let declared = read_u32_be(&bytes, 0) as usize;
    let tags = read_u32_be(&bytes, 128) as usize;
    if declared < 132 || declared > bytes.len()
        || tags.checked_mul(12).and_then(|n| n.checked_add(132)).is_none_or(|n| n > declared) {
        return Err(format!("Truncated ICC profile: {}", path.display()));
    }
    let curve = parse_vcgt(&bytes[..declared]);
    if has_vcgt && curve.is_none() {
        return Err(format!("ICC VCGT cannot be parsed: {}", path.display()));
    }
    Ok(curve)
}

#[cfg(test)]
mod vcgt_extent_tests {
    use super::*;

    fn table_profile(tag_size: u32) -> Vec<u8> {
        let mut bytes = vec![0u8; 144 + 18 + 3 * 256 * 2];
        let size = bytes.len() as u32;
        bytes[0..4].copy_from_slice(&size.to_be_bytes());
        bytes[36..40].copy_from_slice(b"acsp");
        bytes[128..132].copy_from_slice(&1u32.to_be_bytes());
        bytes[132..136].copy_from_slice(b"vcgt");
        bytes[136..140].copy_from_slice(&144u32.to_be_bytes());
        bytes[140..144].copy_from_slice(&tag_size.to_be_bytes());
        bytes[144..148].copy_from_slice(b"vcgt");
        bytes[156..158].copy_from_slice(&3u16.to_be_bytes());
        bytes[158..160].copy_from_slice(&256u16.to_be_bytes());
        bytes[160..162].copy_from_slice(&2u16.to_be_bytes());
        for channel in 0..3 {
            for i in 0..256 {
                let offset = 162 + channel * 512 + i * 2;
                bytes[offset..offset + 2].copy_from_slice(&((i * 257) as u16).to_be_bytes());
            }
        }
        bytes
    }

    #[test]
    fn table_parser_cannot_consume_data_outside_declared_tag() {
        let truncated = table_profile(18);
        assert!(parse_vcgt(&truncated).is_none());
        assert!(crate::icc_validation::validate_profile_tags(&truncated).is_err());
        let valid = table_profile(18 + 3 * 256 * 2);
        let ramp = parse_vcgt(&valid).unwrap();
        assert_eq!(ramp[0][128], 32896);
        assert_eq!(ramp[2][255], 65535);
        assert_eq!(crate::icc_validation::validate_profile_tags(&valid), Ok(true));
    }

    #[test]
    fn invalid_tag_extent_never_panics_or_reads_neighbor_bytes() {
        assert!(parse_vcgt(&table_profile(u32::MAX)).is_none());
        assert!(parse_vcgt(&table_profile(11)).is_none());
    }
}

/// 应用 ICC 配置（WCS 注册 + vcgt 直写 LUT）。**调用方必须已持颜色写门闩**
/// （set_icc_profile 命令 / apply_color_config_from / restore_default_icc 都在
/// 闩内调用）；外部写入的代号递增由门闩统一完成。
pub(crate) fn apply_icc_profile(profile_path: &str, device_id: Option<String>) -> Result<(), String> {
    let target = crate::nvidia::resolve_display_id(device_id.clone());
    if crate::color_backend::active_backend(&target) == crate::color_backend::Backend::Nvidia {
        return crate::color_backend::apply_icc_locked(&target, profile_path);
    }
    apply_icc_profile_standard(profile_path, device_id)
}

/// Transition helper: explicitly uses WCS/GDI, without selecting B recursively.
/// Caller already holds the shared color-write latch.
pub(crate) fn apply_icc_profile_standard(profile_path: &str, device_id: Option<String>) -> Result<(), String> {
    let target = crate::nvidia::resolve_display_id(device_id.clone());
    use windows::Win32::UI::ColorSystem::*;
    use windows::core::PCWSTR;

    let profile_path_buf = PathBuf::from(profile_path);
    let file_name = profile_path_buf
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid profile path")?
        .to_string();

    crate::debug_recorder::log("icc", format!("开始应用：'{file_name}' device={device_id:?}"));

    let dest_path = color_dir().join(&file_name);

    if !std::path::Path::new(&dest_path).exists() {
        fs::copy(profile_path, &dest_path).map_err(|e| {
            crate::debug_recorder::log("icc", format!("复制 ICC 到系统目录失败：{e}"));
            format!("Failed to copy ICC profile: {}", e)
        })?;
    }

    // 读取 ICC 文件解析 vcgt
    let icc_data = fs::read(&dest_path).map_err(|e| {
        crate::debug_recorder::log("icc", format!("读取 ICC 失败：{e}"));
        format!("Failed to read ICC file: {}", e)
    })?;
    crate::icc_validation::validate_profile_tags(&icc_data)
        .map_err(|error| format!("Invalid ICC profile '{file_name}': {error}"))?;
    let previous_base = crate::nvidia::icc_base_for_display(&target);
    crate::debug_recorder::log(
        "icc",
        format!("文件 {} 字节，来源 '{}'，目标目录文件已存在={}", icc_data.len(), profile_path, std::path::Path::new(&dest_path).exists()),
    );

    let pnp_id = resolve_device_pnp(device_id.clone()).map_err(|e| {
        crate::debug_recorder::log("icc", format!("显示器定位失败：{e}"));
        e
    })?;
    let device_id_w: Vec<u16> = pnp_id.encode_utf16().chain(std::iter::once(0)).collect();
    let file_name_w: Vec<u16> = file_name.encode_utf16().chain(std::iter::once(0)).collect();

    eprintln!("ICC: Applying profile: {} to PnP: {}", file_name, pnp_id);

    unsafe {
        // 1. WCS 注册（让颜色管理面板同步）
        let dest_path_w: Vec<u16> = dest_path
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let _ = InstallColorProfileW(None, PCWSTR(dest_path_w.as_ptr()));
        let _ = WcsSetUsePerUserProfiles(PCWSTR(device_id_w.as_ptr()), CLASS_MONITOR, true);
        let _ = WcsAssociateColorProfileWithDevice(
            WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER,
            PCWSTR(file_name_w.as_ptr()),
            PCWSTR(device_id_w.as_ptr()),
        );
        let _ = WcsSetDefaultColorProfile(
            WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER,
            PCWSTR(device_id_w.as_ptr()),
            CPT_ICC,
            CPST_PERCEPTUAL,
            0,
            PCWSTR(file_name_w.as_ptr()),
        );

        // 2. 直接用 SetDeviceGammaRamp 写入显卡 LUT（立即生效）
        if let Some(ramp) = parse_vcgt(&icc_data) {
            crate::debug_recorder::log(
                "icc",
                format!(
                    "vcgt 有效：中间调 R={} G={} B={}（高字节 0-255）",
                    ramp[0][128] >> 8,
                    ramp[1][128] >> 8,
                    ramp[2][128] >> 8
                ),
            );
            // 获取目标显示器的 adapter device_id (\\.\DISPLAYx)
            let adapter_id = device_id.unwrap_or_else(|| {
                get_display_monitors()
                    .ok()
                    .and_then(|ms| ms.into_iter().find(|m| m.is_primary).map(|m| m.device_id))
                    .unwrap_or_default()
            });

            // 同步给 nvidia 调节层，后续滑块调节将在此基础上叠加
            crate::nvidia::set_icc_base_ramp_for_display(&adapter_id, Some(ramp));

            // 用 adapter name 创建 DC
            let adapter_w: Vec<u16> = adapter_id.encode_utf16().chain(std::iter::once(0)).collect();
            let hdc = windows::Win32::Graphics::Gdi::CreateDCW(
                windows::core::PCWSTR(adapter_w.as_ptr()),
                None,
                None,
                None,
            );

            if !hdc.is_invalid() {
                // SetDeviceGammaRamp 需要 3x256 u16 数组，值在高位字节
                let mut gamma_ramp = [0u16; 768]; // R[256] G[256] B[256]
                for i in 0..256 {
                    gamma_ramp[i]       = ramp[0][i]; // R
                    gamma_ramp[256 + i] = ramp[1][i]; // G
                    gamma_ramp[512 + i] = ramp[2][i]; // B
                }
                let result = SetDeviceGammaRamp(
                    hdc,
                    gamma_ramp.as_ptr() as *const _,
                );
                eprintln!("ICC: SetDeviceGammaRamp result: {:?}", result);
                crate::debug_recorder::log(
                    "icc",
                    format!(
                        "SetDeviceGammaRamp device={adapter_id} 返回={}（TRUE≠生效，见 verify 行）",
                        result.as_bool()
                    ),
                );
                crate::debug_recorder::verify_ramp(&adapter_id, &gamma_ramp);
                // 写入成功即缓存为该显示器的最终 LUT（恢复调度的期望曲线）。
                // 若随后还有 NVIDIA 滑块叠加，apply_gamma_ramp_locked 会再以
                // 合成结果覆盖缓存，方案应用结束时缓存就是真正的最终曲线。
                if result.as_bool() {
                    crate::nvidia::cache_final_ramp(&adapter_id, &gamma_ramp);
                }
                let _ = windows::Win32::Graphics::Gdi::DeleteDC(hdc);
                if !result.as_bool() {
                    crate::nvidia::set_icc_base_ramp_for_display(&target, previous_base);
                    return Err(format!("SetDeviceGammaRamp failed for {adapter_id}"));
                }
            } else {
                eprintln!("ICC: Failed to create DC for adapter: {}", adapter_id);
                crate::debug_recorder::log("icc", format!("CreateDCW 失败：{adapter_id}"));
                crate::nvidia::set_icc_base_ramp_for_display(&target, previous_base);
                return Err(format!("CreateDCW failed for {adapter_id}"));
            }
        } else {
            eprintln!("ICC: No vcgt in profile, only WCS registration done");
            crate::debug_recorder::log(
                "icc",
                "该 ICC 无有效 vcgt：只做 WCS 注册，不写 LUT（屏幕不会有可见变化）".to_string(),
            );
            // 没有 vcgt，清空基础 ramp（等同于线性）
            let adapter_id = device_id.unwrap_or_else(|| {
                get_display_monitors()
                    .ok()
                    .and_then(|ms| ms.into_iter().find(|m| m.is_primary).map(|m| m.device_id))
                    .unwrap_or_default()
            });
            crate::nvidia::set_icc_base_ramp_for_display(&adapter_id, None);
            // 触发 calibration loader
            let _ = WcsSetCalibrationManagementState(false);
            let _ = WcsSetCalibrationManagementState(true);
        }

        crate::color_backend::note_profile(&target, profile_path);
        Ok(())
    }
}

#[tauri::command]
pub async fn set_icc_profile(profile_path: String, device_id: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        // 手动 ICC 应用属于外部写入：持颜色写门闩并递增代号
        let _latch = crate::nvidia::begin_external_write();
        apply_icc_profile(&profile_path, device_id)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
pub fn get_current_icc_profile() -> Result<String, String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let desktop_key = hkcu
        .open_subkey_with_flags("Control Panel\\Desktop", KEY_READ)
        .map_err(|e| format!("Failed to open Desktop key: {}", e))?;

    let icm_profile: String = desktop_key
        .get_value("ICMProfile")
        .unwrap_or_else(|_| "sRGB IEC61966-2.1".to_string());

    Ok(icm_profile)
}

/// 同步恢复系统默认 sRGB 配置（清除 vcgt ramp），并将 DVC 复位为 50。
/// 供托盘/快捷键等同步上下文直接调用（避免在回调里 block_on 跑 async）。
/// **调用方必须已持颜色写门闩**（apply_default_config_from / 命令包装层负责）。
pub(crate) fn restore_default_icc(device_id: Option<String>) -> Result<(), String> {
    let target = crate::nvidia::resolve_display_id(device_id.clone());
    if crate::color_backend::active_backend(&target) == crate::color_backend::Backend::Nvidia {
        let previous_settings = crate::nvidia::get_or_default_settings(&target);
        let previous_base = crate::nvidia::icc_base_for_display(&target);
        let previous_profile = crate::color_backend::profile_for_display(&target);
        crate::nvidia::replace_settings(&target, crate::nvidia::NvidiaSettings::default());
        crate::nvidia::set_icc_base_ramp_for_display(&target, None);
        if let Err(error) = crate::nvidia::apply_gamma_ramp_locked(&target) {
            crate::nvidia::replace_settings(&target, previous_settings);
            crate::nvidia::set_icc_base_ramp_for_display(&target, previous_base);
            if let Some(path) = previous_profile { crate::color_backend::note_profile(&target, &path); }
            return Err(error);
        }
        crate::color_backend::clear_profile(&target);
        if let Err(error) = crate::nvidia::dvc_set_locked(&target, 50) {
            crate::color_backend::note_warning(&target, format!("默认曲线已写入，数字振动恢复失败：{error}"));
        }
        return Ok(());
    }
    restore_default_icc_standard(device_id)
}

pub(crate) fn find_system_default_profile() -> Result<PathBuf, String> {
    let color_dir = color_dir();

    let default_profile = [
        "sRGB Color Space Profile.icm",
        "sRGB IEC61966-2.1.icc",
    ]
    .iter()
    .map(|name| color_dir.join(name))
    .find(|p| p.exists())
    .ok_or("Default sRGB profile not found".to_string())?;

    Ok(default_profile)
}

pub(crate) fn restore_default_icc_standard(device_id: Option<String>) -> Result<(), String> {
    let profile_path = find_system_default_profile()?.to_string_lossy().to_string();
    eprintln!("ICC: Restoring default = {}", profile_path);

    apply_icc_profile_standard(&profile_path, device_id.clone())?;

    // 恢复数字振动为 NVIDIA 面板默认 50%。仅 NVIDIA 支持，
    // 失败不影响 ICC 本身已经恢复成功这个事实。
    // 门闩已由调用方持有，走已持闩 helper，不得再经公开命令入口取闩。
    let did = crate::nvidia::resolve_display_id(device_id);
    if let Err(e) = crate::nvidia::dvc_set_locked(&did, 50) {
        eprintln!("[ICC] restore default: digital vibrance reset skipped: {}", e);
    }
    Ok(())
}

#[tauri::command]
pub async fn restore_default_icc_profile(device_id: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        // 手动恢复默认属于外部写入：持颜色写门闩并递增代号
        let _latch = crate::nvidia::begin_external_write();
        restore_default_icc(device_id)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// 用资源管理器打开系统 ICC 目录
#[tauri::command]
pub fn open_icc_directory() -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(color_dir())
        .spawn()
        .map_err(|e| format!("Failed to open directory: {}", e))?;
    Ok(())
}

/// 导入 ICC 文件到系统目录（支持拖拽/文件选择传入路径）
/// 返回导入后的文件名
#[tauri::command]
pub fn import_icc_profile(src_path: String) -> Result<String, String> {
    let src = PathBuf::from(&src_path);
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "icc" && ext != "icm" {
        return Err("Only .icc or .icm files are supported".to_string());
    }

    let file_name = src
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();

    let color_dir = color_dir();
    let dest = color_dir.join(&file_name);

    fs::copy(&src, &dest).map_err(|e| format!("Failed to copy ICC profile: {}", e))?;

    eprintln!("ICC: Imported '{}' -> '{}'", src_path, dest.display());
    Ok(file_name)
}

/// 校验 ICC 字节：大小区间、`acsp` 魔数、头部声明长度与文件长度一致。
/// 市场下载、分享下载、本地读取共用这一份判断，避免各处宽严不一。
fn validate_icc_bytes(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < 128 || bytes.len() > MAX_ICC_BYTES {
        return Err("配置文件大小异常".to_string());
    }
    if bytes.get(36..40) != Some(b"acsp".as_slice()) {
        return Err("不是有效的 ICC 配置文件".to_string());
    }
    let declared = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    if declared != bytes.len() {
        return Err("ICC 头部长度与文件不一致".to_string());
    }
    Ok(())
}

/// 把 ICC 字节写入系统 ICC 目录，返回最终文件名。
///
/// 同名且内容一致 → 直接复用（不重复写盘）；同名但内容不同 → 自动加序号。
/// `tag` 只用于日志定位来源（market / share）。
fn install_icc_bytes(bytes: &[u8], stem: &str, tag: &str) -> Result<String, String> {
    let color_dir = color_dir();
    let mut file_name = format!("{}.icc", stem);
    let mut suffix = 2;

    loop {
        let dest = color_dir.join(&file_name);
        if !dest.exists() {
            fs::write(&dest, bytes).map_err(|e| format!("写入 ICC 失败: {}", e))?;
            eprintln!(
                "ICC: Imported {} profile -> '{}'",
                tag,
                dest.display()
            );
            return Ok(file_name);
        }
        if let Ok(existing) = fs::read(&dest) {
            if existing.as_slice() == bytes {
                return Ok(file_name);
            }
        }
        if suffix > 50 {
            return Err("同名配置文件过多，请先整理 ICC 目录".to_string());
        }
        file_name = format!("{}-{}.icc", stem, suffix);
        suffix += 1;
    }
}

/// 从 ICC 市场下载并安装一份配置。
///
/// 只接受市场服务自己的配置 id（不接收任意 URL），下载地址由后端拼接，
/// 避免这个命令变成通用的“任意地址下载器”。文件名取自标题，写入系统 ICC
/// 目录；同名且内容一致视为已导入，同名但内容不同则自动加序号。
#[tauri::command]
pub async fn import_market_icc(id: String, title: String) -> Result<String, String> {
    let market_api = crate::server_env::market_api_base();

    if id.is_empty()
        || id.len() > 64
        || !id
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == '-')
    {
        return Err("无效的市场配置 ID".to_string());
    }

    let url = format!("{market_api}/api/market/icc/{id}/download");
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("下载客户端初始化失败: {}", e))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("下载失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("下载失败: HTTP {}", resp.status()));
    }
    if let Some(len) = resp.content_length() {
        if len as usize > MAX_ICC_BYTES {
            return Err("配置文件超过 2 MB".to_string());
        }
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("读取配置失败: {}", e))?;
    validate_icc_bytes(&bytes)?;

    let stem = sanitize_icc_file_stem(&title, &id);
    install_icc_bytes(&bytes, &stem, "market")
}

/// 读取系统 ICC 目录里的一份配置，供分享流程计算 sha256 并上传。
///
/// 只接受纯文件名：分享 payload 来自本地方案，但仍是外部输入，拒绝任何
/// 路径分隔符以免变成任意文件读取器。返回 base64（前端已有 sha256 与上传
/// 实现，无需在 Rust 侧再引一个哈希依赖）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IccFileContent {
    pub name: String,
    pub size_bytes: usize,
    pub base64: String,
}

#[tauri::command]
pub fn read_icc_file(name: String) -> Result<IccFileContent, String> {
    if name.is_empty()
        || name.len() > 255
        || name.contains('/')
        || name.contains('\\')
        || name == "."
        || name == ".."
    {
        return Err("无效的 ICC 文件名".to_string());
    }

    let path = color_dir().join(&name);
    if !path.exists() {
        return Err(format!("ICC 文件不存在: {}", name));
    }
    let meta = fs::metadata(&path).map_err(|e| format!("读取 ICC 信息失败: {}", e))?;
    if meta.len() as usize > MAX_ICC_BYTES {
        return Err("配置文件超过 2 MB".to_string());
    }
    let bytes = fs::read(&path).map_err(|e| format!("读取 ICC 失败: {}", e))?;
    // 分享前就校验一次：坏文件不该被上传到库里
    validate_icc_bytes(&bytes)?;

    Ok(IccFileContent {
        name,
        size_bytes: bytes.len(),
        base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
    })
}

/// 按分享 slug 下载并安装 ICC。
///
/// 与服务端的分享接口对齐：只接受 slug（不含路径），下载地址由后端拼接。
/// 服务端会 302 到 R2 公网地址，reqwest 默认跟随重定向。
#[tauri::command]
pub async fn import_shared_icc(
    slug: String,
    file_name: String,
    title: String,
) -> Result<String, String> {
    let market_api = crate::server_env::market_api_base();

    if slug.is_empty()
        || slug.len() > 64
        || !slug
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("无效的分享标识".to_string());
    }

    let url = format!("{market_api}/api/market/shares/{slug}/icc");
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("下载客户端初始化失败: {}", e))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("下载失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("下载失败: HTTP {}", resp.status()));
    }
    if let Some(len) = resp.content_length() {
        if len as usize > MAX_ICC_BYTES {
            return Err("配置文件超过 2 MB".to_string());
        }
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("读取配置失败: {}", e))?;
    validate_icc_bytes(&bytes)?;

    // 文件名优先用分享里记录的 ICC 原名（去掉 .icc/.icm 后缀），
    // 缺失或清洗后为空才退回标题。
    let stem_source = if file_name.trim().is_empty() {
        title
    } else {
        let lower = file_name.to_ascii_lowercase();
        let stripped = lower
            .strip_suffix(".icc")
            .or_else(|| lower.strip_suffix(".icm"))
            .map(|_| &file_name[..file_name.len() - 4])
            .unwrap_or(&file_name);
        stripped.to_string()
    };
    let stem = sanitize_icc_file_stem(&stem_source, &slug);
    install_icc_bytes(&bytes, &stem, "share")
}

/// 把市场标题转成安全的 Windows 文件名（不含扩展名）。
fn sanitize_icc_file_stem(title: &str, id: &str) -> String {
    let mut out = String::new();
    let mut count = 0;
    for ch in title.chars() {
        if count >= 80 {
            break;
        }
        if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
            continue;
        }
        out.push(ch);
        count += 1;
    }
    let trimmed = out.trim().trim_matches('.').trim();
    if trimmed.is_empty() {
        format!("market-{}", &id[..id.len().min(8)])
    } else {
        trimmed.to_string()
    }
}

/// 导出 ICC 文件到指定目录
/// profile_name: ICC 文件名（不含路径），dest_dir: 目标目录路径
#[tauri::command]
pub fn export_icc_profile(profile_name: String, dest_dir: String) -> Result<String, String> {
    let color_dir = color_dir();
    let src = color_dir.join(&profile_name);

    if !src.exists() {
        return Err(format!("Profile '{}' not found", profile_name));
    }

    let dest_path = PathBuf::from(&dest_dir).join(&profile_name);
    fs::copy(&src, &dest_path).map_err(|e| format!("Failed to export ICC profile: {}", e))?;

    eprintln!("ICC: Exported '{}' -> '{}'", profile_name, dest_path.display());
    Ok(dest_path.to_string_lossy().to_string())
}

/// 按名称关键字搜索 ICC 配置文件（大小写不敏感）
#[tauri::command]
pub fn search_icc_profiles(query: String) -> Result<Vec<IccProfile>, String> {
    let all = get_icc_profiles()?;
    let q = query.to_lowercase();
    Ok(all.into_iter().filter(|p| p.name.to_lowercase().contains(&q)).collect())
}

/// 设置预览图片路径（前端传入本地绝对路径，后端验证文件存在后返回）
#[tauri::command]
pub fn set_preview_image(image_path: String) -> Result<String, String> {
    let path = PathBuf::from(&image_path);
    if !path.exists() {
        return Err(format!("File not found: {}", image_path));
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if !["jpg", "jpeg", "png", "webp", "bmp", "gif"].contains(&ext.as_str()) {
        return Err("Unsupported image format".to_string());
    }
    Ok(image_path)
}

#[tauri::command]
pub async fn capture_icc_effect_preview(
    app: tauri::AppHandle,
    profile_name: Option<String>,
    settings: PreviewColorSettings,
) -> Result<String, String> {
    let ramp = if let Some(name) = profile_name {
        let path = PathBuf::from(&name);
        if path.file_name().and_then(|value| value.to_str()) != Some(name.as_str())
            || !is_icc_ext(&path)
        {
            return Err("ICC 配置文件名称无效".to_string());
        }
        let profile_path = color_dir().join(path);
        let bytes = fs::read(&profile_path)
            .map_err(|error| format!("读取 ICC 配置文件失败：{error}"))?;
        parse_vcgt(&bytes)
    } else {
        None
    };
    let window = app.get_webview_window("main");

    tauri::async_runtime::spawn_blocking(move || {
        if let Some(window) = &window {
            window.hide().map_err(|e| format!("隐藏主窗口失败：{e}"))?;
            std::thread::sleep(std::time::Duration::from_millis(350));
        }

        let result = capture_primary_display_with_ramp(ramp.as_ref(), settings);

        if let Some(window) = window {
            if let Err(error) = window.show() {
                eprintln!("ICC: 恢复主窗口显示失败：{error}");
            }
            if let Err(error) = window.set_focus() {
                eprintln!("ICC: 恢复主窗口焦点失败：{error}");
            }
        }

        result
    })
    .await
    .map_err(|e| format!("截图任务失败：{e}"))?
}

#[tauri::command]
pub fn save_icc_effect_preview(
    app: tauri::AppHandle,
    png_base64: String,
) -> Result<String, String> {
    use base64::Engine;

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(png_base64)
        .map_err(|_| "效果图编码无效".to_string())?;
    if bytes.len() < 8 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("效果图不是有效的 PNG 文件".to_string());
    }

    let directory = app.path().app_local_data_dir()
        .map_err(|error| format!("获取效果图保存目录失败：{error}"))?
        .join("icc-effect-previews");
    fs::create_dir_all(&directory)
        .map_err(|error| format!("创建效果图目录失败：{error}"))?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("读取系统时间失败：{error}"))?
        .as_millis();
    let path = directory.join(format!("icc-effect-{timestamp}.png"));
    fs::write(&path, bytes).map_err(|error| format!("保存效果图失败：{error}"))?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn open_icc_effect_preview_directory(app: tauri::AppHandle) -> Result<(), String> {
    let directory = app.path().app_local_data_dir()
        .map_err(|error| format!("获取效果图保存目录失败：{error}"))?
        .join("icc-effect-previews");
    fs::create_dir_all(&directory)
        .map_err(|error| format!("创建效果图目录失败：{error}"))?;
    std::process::Command::new("explorer")
        .arg(directory)
        .spawn()
        .map_err(|error| format!("打开效果图目录失败：{error}"))?;
    Ok(())
}

#[derive(Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewColorSettings {
    brightness: i32,
    contrast: i32,
    gamma: f64,
    digital_vibrance: i32,
    rgb_r: i32,
    rgb_g: i32,
    rgb_b: i32,
}

fn capture_primary_display_with_ramp(
    icc_ramp: Option<&[[u16; 256]; 3]>,
    settings: PreviewColorSettings,
) -> Result<String, String> {
    use base64::Engine;
    use std::ffi::c_void;
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
        GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS,
        HGDIOBJ, SRCCOPY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

    let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    if width <= 0 || height <= 0 {
        return Err("无法读取主显示器尺寸".to_string());
    }
    let pixel_count = (width as usize)
        .checked_mul(height as usize)
        .filter(|count| *count <= 100_000_000)
        .ok_or_else(|| "主显示器分辨率超出截图限制".to_string())?;

    unsafe {
        let screen_dc = GetDC(None);
        if screen_dc.is_invalid() {
            return Err("无法访问桌面画面".to_string());
        }
        let memory_dc = CreateCompatibleDC(screen_dc);
        if memory_dc.is_invalid() {
            ReleaseDC(None, screen_dc);
            return Err("无法创建截图缓冲区".to_string());
        }
        let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
        if bitmap.is_invalid() {
            let _ = DeleteDC(memory_dc);
            ReleaseDC(None, screen_dc);
            return Err("无法分配截图图像".to_string());
        }

        let previous = SelectObject(memory_dc, HGDIOBJ(bitmap.0));
        let capture_result = (|| {
            BitBlt(
                memory_dc,
                0,
                0,
                width,
                height,
                screen_dc,
                0,
                0,
                SRCCOPY | CAPTUREBLT,
            )
            .map_err(|e| format!("读取桌面画面失败：{e}"))?;

            SelectObject(memory_dc, previous);
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader.biSize = std::mem::size_of_val(&info.bmiHeader) as u32;
            info.bmiHeader.biWidth = width;
            info.bmiHeader.biHeight = -height;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB.0;
            let mut bgra = vec![0u8; pixel_count * 4];
            let lines = GetDIBits(
                screen_dc,
                bitmap,
                0,
                height as u32,
                Some(bgra.as_mut_ptr().cast::<c_void>()),
                &mut info,
                DIB_RGB_COLORS,
            );
            if lines != height {
                return Err("读取截图像素失败".to_string());
            }

            let brightness = (settings.brightness.clamp(-125, 125) as f64 + 125.0) / 250.0;
            let contrast = (settings.contrast.clamp(-82, 82) as f64 + 82.0) / 164.0;
            let gamma = settings.gamma.clamp(0.4, 2.8);
            let contrast = (contrast - 0.5) * 2.0;
            let brightness = (brightness - 0.5) * 2.0;
            let offset = if contrast > 0.0 { contrast * -25.4 } else { contrast * -32.0 };
            let range = 255.0 + offset * 2.0;
            let offset = offset + brightness * (range / 5.0);
            let gains = [settings.rgb_r, settings.rgb_g, settings.rgb_b]
                .map(|gain| 1.0 + (gain.clamp(-100, 100) as f64 / 100.0) * 0.48);
            let saturation = settings.digital_vibrance.clamp(0, 100) as f64 / 50.0;
            let mut rgb = Vec::with_capacity(pixel_count * 3);
            for pixel in bgra.chunks_exact(4) {
                let input = [pixel[2], pixel[1], pixel[0]];
                let mut output = [0.0; 3];
                for (channel, input_value) in input.iter().enumerate() {
                    let linear = ((*input_value as f64 + offset) / range).clamp(0.0, 1.0);
                    let adjusted = if (gamma - 1.0).abs() < 1e-6 {
                        linear
                    } else {
                        linear.powf(1.0 / gamma)
                    };
                    let lut_value = (adjusted * 65535.0).round() as u16;
                    let index = (lut_value as usize * 255 / 65535).min(255);
                    let base = icc_ramp
                        .map(|ramp| ramp[channel][index])
                        .unwrap_or(lut_value);
                    output[channel] = (base as f64 / 257.0 * gains[channel]).clamp(0.0, 255.0);
                }
                let luminance = output[0] * 0.2126 + output[1] * 0.7152 + output[2] * 0.0722;
                for out in output.iter() {
                    rgb.push((luminance + (out - luminance) * saturation)
                        .round()
                        .clamp(0.0, 255.0) as u8);
                }
            }

            let mut png_data = Vec::new();
            {
                let mut encoder = png::Encoder::new(&mut png_data, width as u32, height as u32);
                encoder.set_color(png::ColorType::Rgb);
                encoder.set_depth(png::BitDepth::Eight);
                let mut writer = encoder
                    .write_header()
                    .map_err(|e| format!("创建 PNG 失败：{e}"))?;
                writer
                    .write_image_data(&rgb)
                    .map_err(|e| format!("编码 PNG 失败：{e}"))?;
            }
            Ok(base64::engine::general_purpose::STANDARD.encode(png_data))
        })();

        SelectObject(memory_dc, previous);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(memory_dc);
        ReleaseDC(None, screen_dc);
        capture_result
    }
}
