use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NvidiaSettings {
    pub brightness: i32,       // -125 to 125 (maps to 0.0~1.0, default 0 = 0.5)
    pub contrast: i32,         // -82 to 82   (maps to 0.0~1.0, default 0 = 0.5)
    pub gamma: f64,            // 0.4 to 2.8  (direct, default 1.0)
    pub digital_vibrance: i32, // 0 to 100
}

impl Default for NvidiaSettings {
    fn default() -> Self {
        Self { brightness: 0, contrast: 0, gamma: 1.0, digital_vibrance: 50 }
    }
}

// Per-display settings: key = device_id (e.g. "\\.\DISPLAY1")
static DISPLAY_SETTINGS: Mutex<Option<HashMap<String, NvidiaSettings>>> = Mutex::new(None);
static DISPLAY_ICC_RAMPS: Mutex<Option<HashMap<String, [[u16; 256]; 3]>>> = Mutex::new(None);

fn get_settings_map() -> std::sync::MutexGuard<'static, Option<HashMap<String, NvidiaSettings>>> {
    DISPLAY_SETTINGS.lock().unwrap()
}

fn get_or_default_settings(device_id: &str) -> NvidiaSettings {
    let map = DISPLAY_SETTINGS.lock().unwrap();
    map.as_ref()
        .and_then(|m| m.get(device_id))
        .cloned()
        .unwrap_or_default()
}

fn update_settings(device_id: &str, f: impl FnOnce(&mut NvidiaSettings)) -> NvidiaSettings {
    let mut map = DISPLAY_SETTINGS.lock().unwrap();
    let m = map.get_or_insert_with(HashMap::new);
    let s = m.entry(device_id.to_string()).or_insert_with(NvidiaSettings::default);
    f(s);
    s.clone()
}

/// 由 icc.rs 调用，设置指定显示器的 ICC vcgt 基础 ramp
pub fn set_icc_base_ramp_for_display(device_id: &str, ramp: Option<[[u16; 256]; 3]>) {
    let mut map = DISPLAY_ICC_RAMPS.lock().unwrap();
    let m = map.get_or_insert_with(HashMap::new);
    match ramp {
        Some(r) => { m.insert(device_id.to_string(), r); }
        None => { m.remove(device_id); }
    }
}

/// 兼容旧调用：设置主显示器的 ICC base ramp
pub fn set_icc_base_ramp(ramp: Option<[[u16; 256]; 3]>) {
    // 找主显示器 device_id，fallback 到 \\.\DISPLAY1
    let primary = crate::icc::get_display_monitors()
        .ok()
        .and_then(|ms| ms.into_iter().find(|m| m.is_primary).map(|m| m.device_id))
        .unwrap_or_else(|| "\\\\.\\DISPLAY1".to_string());
    set_icc_base_ramp_for_display(&primary, ramp);
}

// NVAPI
const NVAPI_ID_INITIALIZE: u32 = 0x0150E828;
const NVAPI_ID_ENUM_DISPLAY_HANDLE: u32 = 0x9ABDD40D;
const NVAPI_ID_SET_DVC_LEVEL: u32 = 0x172409B4;
const NVAPI_ID_SET_DVC_LEVEL_EX: u32 = 0x4A82C2B1;
const NVAPI_ID_GET_DVC_INFO_EX: u32 = 0x0E45002D;

#[repr(C)]
struct NvDvcInfoEx {
    version: u32,
    current_level: i32,
    min_level: i32,
    max_level: i32,
    default_level: i32,
}

type NvQueryInterface = unsafe extern "C" fn(id: u32) -> *mut std::ffi::c_void;
type NvInitialize = unsafe extern "C" fn() -> i32;
type NvEnumDisplayHandle = unsafe extern "C" fn(this_enum: i32, p_nv_disp_handle: *mut u32) -> i32;
type NvSetDvcLevel = unsafe extern "C" fn(h_nv_disp: u32, output_id: u32, level: i32) -> i32;
type NvSetDvcLevelEx = unsafe extern "C" fn(h_nv_disp: u32, output_id: u32, p_dvc_info: *mut NvDvcInfoEx) -> i32;
type NvGetDvcInfoEx = unsafe extern "C" fn(h_nv_disp: u32, output_id: u32, p_dvc_info: *mut NvDvcInfoEx) -> i32;

/// 从 device_id (如 "\\.\DISPLAY2") 提取显示器索引（0-based）
fn display_index_from_device_id(device_id: Option<&str>) -> i32 {
    device_id
        .and_then(|id| id.chars().rev().take_while(|c| c.is_ascii_digit()).collect::<String>().chars().rev().collect::<String>().parse::<i32>().ok())
        .map(|n| (n - 1).max(0)) // DISPLAY1 -> index 0, DISPLAY2 -> index 1
        .unwrap_or(0)
}

fn nvapi_load_for_display(display_index: i32) -> Result<(windows::Win32::Foundation::HMODULE, u32), String> {
    unsafe {
        let lib = windows::Win32::System::LibraryLoader::LoadLibraryW(
            windows::core::w!("nvapi64.dll")
        ).map_err(|e| format!("nvapi64.dll not found: {}", e))?;

        let query_ptr = windows::Win32::System::LibraryLoader::GetProcAddress(
            lib, windows::core::s!("nvapi_QueryInterface"),
        ).ok_or("nvapi_QueryInterface not found")?;
        let query_fn: NvQueryInterface = std::mem::transmute(query_ptr);

        let init_ptr = query_fn(NVAPI_ID_INITIALIZE);
        if init_ptr.is_null() { return Err("NvAPI_Initialize not found".into()); }
        let init_fn: NvInitialize = std::mem::transmute(init_ptr);
        if init_fn() != 0 { return Err("NvAPI_Initialize failed".into()); }

        let enum_ptr = query_fn(NVAPI_ID_ENUM_DISPLAY_HANDLE);
        if enum_ptr.is_null() { return Err("NvAPI_EnumNvidiaDisplayHandle not found".into()); }
        let enum_fn: NvEnumDisplayHandle = std::mem::transmute(enum_ptr);
        let mut handle: u32 = 0;
        if enum_fn(display_index, &mut handle) != 0 {
            let _ = windows::Win32::Foundation::FreeLibrary(lib);
            return Err(format!("No NVIDIA display at index {}", display_index));
        }

        Ok((lib, handle))
    }
}

/// 读取驱动的 DVC 信息（min/max/default/current）
fn nvapi_get_dvc_info(device_id: Option<&str>) -> Result<(i32, i32, i32, i32), String> {
    unsafe {
        let idx = display_index_from_device_id(device_id);
        let (lib, handle) = nvapi_load_for_display(idx)?;

        let query_ptr = windows::Win32::System::LibraryLoader::GetProcAddress(
            lib, windows::core::s!("nvapi_QueryInterface"),
        ).ok_or("nvapi_QueryInterface not found")?;
        let query_fn: NvQueryInterface = std::mem::transmute(query_ptr);

        let get_ptr = query_fn(NVAPI_ID_GET_DVC_INFO_EX);
        if get_ptr.is_null() {
            let _ = windows::Win32::Foundation::FreeLibrary(lib);
            return Err("NvAPI_GetDVCInfoEx not found".into());
        }
        let get_fn: NvGetDvcInfoEx = std::mem::transmute(get_ptr);

        let mut info = NvDvcInfoEx {
            version: (std::mem::size_of::<NvDvcInfoEx>() as u32) | 0x10000,
            current_level: 0,
            min_level: 0,
            max_level: 100,
            default_level: 50,
        };
        let status = get_fn(handle, 0, &mut info);

        let _ = windows::Win32::Foundation::FreeLibrary(lib);

        if status == 0 {
            Ok((info.min_level, info.max_level, info.default_level, info.current_level))
        } else {
            Err(format!("GetDVCInfoEx failed: {}", status))
        }
    }
}

fn nvapi_set_dvc(level: i32, device_id: Option<&str>) -> Result<(), String> {
    unsafe {
        let idx = display_index_from_device_id(device_id);
        let (lib, handle) = nvapi_load_for_display(idx)?;

        let query_ptr = windows::Win32::System::LibraryLoader::GetProcAddress(
            lib, windows::core::s!("nvapi_QueryInterface"),
        ).ok_or("nvapi_QueryInterface not found")?;
        let query_fn: NvQueryInterface = std::mem::transmute(query_ptr);

        // 先尝试 SetDVCLevelEx（新接口，直接传结构体，范围与面板一致）
        let set_ex_ptr = query_fn(NVAPI_ID_SET_DVC_LEVEL_EX);
        let status = if !set_ex_ptr.is_null() {
            // 先读取当前 info 获取 min/max
            let get_ptr = query_fn(NVAPI_ID_GET_DVC_INFO_EX);
            let mut info = NvDvcInfoEx {
                version: (std::mem::size_of::<NvDvcInfoEx>() as u32) | 0x10000,
                current_level: level,
                min_level: 0,
                max_level: 100,
                default_level: 50,
            };
            if !get_ptr.is_null() {
                let get_fn: NvGetDvcInfoEx = std::mem::transmute(get_ptr);
                let mut cur = NvDvcInfoEx {
                    version: (std::mem::size_of::<NvDvcInfoEx>() as u32) | 0x10000,
                    current_level: 0, min_level: 0, max_level: 100, default_level: 50,
                };
                if get_fn(handle, 0, &mut cur) == 0 {
                    info.min_level = cur.min_level;
                    info.max_level = cur.max_level;
                    info.default_level = cur.default_level;
                }
            }
            let set_ex_fn: NvSetDvcLevelEx = std::mem::transmute(set_ex_ptr);
            set_ex_fn(handle, 0, &mut info)
        } else {
            // 回退到旧接口
            let set_ptr = query_fn(NVAPI_ID_SET_DVC_LEVEL);
            if set_ptr.is_null() {
                let _ = windows::Win32::Foundation::FreeLibrary(lib);
                return Err("NvAPI_SetDVCLevel not found".into());
            }
            let set_fn: NvSetDvcLevel = std::mem::transmute(set_ptr);
            set_fn(handle, 0, level)
        };

        let _ = windows::Win32::Foundation::FreeLibrary(lib);
        if status == 0 { Ok(()) } else { Err(format!("SetDVCLevel failed: {}", status)) }
    }
}

/// 亮度/对比度与伽马独立计算，与 NVIDIA 面板行为一致：
/// - 亮度/对比度：线性偏移/缩放
/// - 伽马：独立幂函数，不与亮度/对比度耦合
fn calculate_lut(brightness: f64, contrast: f64, gamma: f64) -> [u16; 256] {
    let contrast = (contrast.clamp(0.0, 1.0) - 0.5) * 2.0; // -1..1
    let brightness = (brightness.clamp(0.0, 1.0) - 0.5) * 2.0; // -1..1

    // 亮度/对比度线性变换（不含伽马）
    let offset = if contrast > 0.0 { contrast * -25.4 } else { contrast * -32.0 };
    let range = 255.0 + offset * 2.0;
    let offset = offset + brightness * (range / 5.0);

    let gamma = gamma.clamp(0.4, 2.8);

    let mut lut = [0u16; 256];
    for i in 0..256 {
        // 先做亮度/对比度线性映射
        let linear = ((i as f64 + offset) / range).clamp(0.0, 1.0);
        // 再独立叠加伽马（仅当 gamma != 1.0 时有效果）
        let value = if (gamma - 1.0).abs() < 1e-6 {
            linear
        } else {
            linear.powf(1.0 / gamma)
        };
        lut[i] = (value * 65535.0).round() as u16;
    }
    lut
}

fn apply_gamma_ramp(device_id: &str, brightness: i32, contrast: i32, gamma: f64) -> Result<(), String> {
    // UI range -> WindowsDisplayAPI range
    let b = (brightness as f64 + 125.0) / 250.0;
    let c = (contrast as f64 + 82.0) / 164.0;
    let g = gamma.clamp(0.4, 2.8);

    let lut = calculate_lut(b, c, g);

    // 在 ICC vcgt 基础上叠加调节：把 lut 作为索引映射应用到 icc_base
    let icc_ramps = DISPLAY_ICC_RAMPS.lock().unwrap();
    let icc_base = icc_ramps.as_ref().and_then(|m| m.get(device_id));
    let mut ramp = [0u16; 768];
    for i in 0..256 {
        for (ch, offset) in [(0usize, 0usize), (1, 256), (2, 512)] {
            let adjusted = if let Some(base) = icc_base {
                let idx = (lut[i] as usize * 255 / 65535).min(255);
                base[ch][idx]
            } else {
                lut[i]
            };
            ramp[offset + i] = adjusted;
        }
    }
    drop(icc_ramps);

    unsafe {
        // 使用 CreateDCW 指定具体显示器
        let device_w: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();
        let hdc = windows::Win32::Graphics::Gdi::CreateDCW(
            windows::core::PCWSTR(device_w.as_ptr()),
            None,
            None,
            None,
        );
        if hdc.is_invalid() {
            return Err(format!("Failed to create DC for display: {}", device_id));
        }
        let result = windows::Win32::UI::ColorSystem::SetDeviceGammaRamp(hdc, ramp.as_ptr() as *const _);
        let _ = windows::Win32::Graphics::Gdi::DeleteDC(hdc);
        if result.as_bool() { Ok(()) } else { Err(format!("SetDeviceGammaRamp failed for {}", device_id)) }
    }
}

#[tauri::command]
pub fn set_nvidia_brightness(device_id: Option<String>, value: i32) -> Result<(), String> {
    let did = resolve_display_id(device_id);
    let s = update_settings(&did, |s| s.brightness = value);
    apply_gamma_ramp(&did, s.brightness, s.contrast, s.gamma)
}

#[tauri::command]
pub fn set_nvidia_contrast(device_id: Option<String>, value: i32) -> Result<(), String> {
    let did = resolve_display_id(device_id);
    let s = update_settings(&did, |s| s.contrast = value);
    apply_gamma_ramp(&did, s.brightness, s.contrast, s.gamma)
}

#[tauri::command]
pub fn set_nvidia_gamma(device_id: Option<String>, value: f64) -> Result<(), String> {
    let did = resolve_display_id(device_id);
    let s = update_settings(&did, |s| s.gamma = value);
    apply_gamma_ramp(&did, s.brightness, s.contrast, s.gamma)
}

#[tauri::command]
pub fn set_nvidia_digital_vibrance(device_id: Option<String>, value: i32) -> Result<(), String> {
    let did = resolve_display_id(device_id);
    update_settings(&did, |s| s.digital_vibrance = value);
    nvapi_set_dvc(value.clamp(0, 100), Some(&did))
}

/// 解析 device_id，fallback 到主显示器
fn resolve_display_id(device_id: Option<String>) -> String {
    device_id.unwrap_or_else(|| {
        crate::icc::get_display_monitors()
            .ok()
            .and_then(|ms| ms.into_iter().find(|m| m.is_primary).map(|m| m.device_id))
            .unwrap_or_else(|| "\\\\.\\DISPLAY1".to_string())
    })
}

/// 启动时从驱动读取当前 DVC 实际值，同步到内存状态
#[tauri::command]
pub fn sync_dvc_from_driver(device_id: Option<String>) -> i32 {
    let did = resolve_display_id(device_id);
    match nvapi_get_dvc_info(Some(&did)) {
        Ok((min, max, default, current)) => {
            let range = max - min;
            let ui_value = if range > 0 {
                ((current - min) * 100 / range).clamp(0, 100)
            } else {
                50
            };
            eprintln!("[DVC] display={} min={} max={} default={} current={} => ui={}", did, min, max, default, current, ui_value);
            update_settings(&did, |s| s.digital_vibrance = ui_value);
            ui_value
        }
        Err(e) => {
            eprintln!("[DVC] sync failed for {}: {}", did, e);
            50
        }
    }
}

#[tauri::command]
pub fn get_dvc_default_ui_value(device_id: Option<String>) -> i32 {
    let did = resolve_display_id(device_id);
    match nvapi_get_dvc_info(Some(&did)) {
        Ok((min, max, default, _)) => {
            let range = max - min;
            if range > 0 {
                ((default - min) * 100 / range).clamp(0, 100)
            } else {
                50
            }
        }
        Err(_) => 50,
    }
}

#[tauri::command]
pub fn get_nvidia_settings(device_id: Option<String>) -> Result<NvidiaSettings, String> {
    let did = resolve_display_id(device_id);
    Ok(get_or_default_settings(&did))
}
