use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;
use winreg::enums::*;
use winreg::RegKey;

/// 将打包的内置 ICC 文件安装到系统 ICC 目录（首次启动调用）
#[tauri::command]
pub fn install_builtin_icc_profiles(app: tauri::AppHandle) -> Result<u32, String> {
    let color_dir = PathBuf::from("C:\\Windows\\System32\\spool\\drivers\\color");
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

#[derive(Debug, Serialize, Deserialize)]
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
fn get_monitor_name_from_edid(pnp_id: &str) -> Option<String> {
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

#[tauri::command]
pub fn get_display_monitors() -> Result<Vec<DisplayMonitor>, String> {
    unsafe {
        let user32 = windows::Win32::System::LibraryLoader::LoadLibraryW(windows::core::w!("user32.dll"))
            .map_err(|e| format!("Failed to load user32.dll: {}", e))?;

        type FnEnumDisplayDevicesW = unsafe extern "system" fn(*const u16, u32, *mut DISPLAY_DEVICEW, u32) -> i32;
        let enum_fn: FnEnumDisplayDevicesW = std::mem::transmute(
            windows::Win32::System::LibraryLoader::GetProcAddress(user32, windows::core::s!("EnumDisplayDevicesW"))
                .ok_or("EnumDisplayDevicesW not found")?
        );

        let mut monitors = Vec::new();
        let mut adapter_idx = 0;
        let mut adapter: DISPLAY_DEVICEW = Default::default();

        while enum_fn(std::ptr::null(), adapter_idx, &mut adapter, 0) != 0 {
            let is_primary = adapter.StateFlags & DISPLAY_DEVICE_PRIMARY_DEVICE != 0;
            let adapter_name_str = u16_to_str(&adapter.DeviceName);

            if adapter.StateFlags & DISPLAY_DEVICE_ACTIVE != 0 {
                // 用不带 EDD flag 的第二层枚举拿 DeviceID（PnP 路径）
                let mut mon: DISPLAY_DEVICEW = Default::default();
                let pnp_id = if enum_fn(adapter.DeviceName.as_ptr(), 0, &mut mon, 0) != 0 {
                    u16_to_str(&mon.DeviceID)
                } else {
                    String::new()
                };

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

                eprintln!("ICC: Monitor '{}' adapter='{}' pnp='{}'", label, adapter_name_str, pnp_id);
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

        let _ = windows::Win32::Foundation::FreeLibrary(user32);

        eprintln!("ICC: Found {} monitors", monitors.len());
        Ok(monitors)
    }
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

#[tauri::command]
pub fn get_icc_profiles() -> Result<Vec<IccProfile>, String> {
    let color_dir = PathBuf::from("C:\\Windows\\System32\\spool\\drivers\\color");

    if !color_dir.exists() {
        return Err("ICC profile directory not found".to_string());
    }

    let mut profiles = Vec::new();

    if let Ok(entries) = fs::read_dir(color_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("icc") || path.extension().and_then(|e| e.to_str()) == Some("icm") {
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
    u16::from_be_bytes([data[offset], data[offset + 1]])
}

fn read_u32_be(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
}

/// 从 ICC 文件解析 vcgt 标签，返回 [R256, G256, B256] 的 gamma ramp（每个值 0..=65535）
fn parse_vcgt(icc_data: &[u8]) -> Option<[[u16; 256]; 3]> {
    if icc_data.len() < 132 { return None; }

    // ICC header: tag count at offset 128
    let tag_count = read_u32_be(icc_data, 128) as usize;
    let tag_table_start = 132;

    // 查找 vcgt 标签 (0x76636774)
    let vcgt_sig: u32 = 0x76636774;
    let mut vcgt_offset = 0usize;
    let mut _vcgt_size = 0usize;

    for i in 0..tag_count {
        let base = tag_table_start + i * 12;
        if base + 12 > icc_data.len() { break; }
        let sig = read_u32_be(icc_data, base);
        if sig == vcgt_sig {
            vcgt_offset = read_u32_be(icc_data, base + 4) as usize;
            _vcgt_size = read_u32_be(icc_data, base + 8) as usize;
            break;
        }
    }

    if vcgt_offset == 0 || vcgt_offset + 8 > icc_data.len() {
        eprintln!("ICC: No vcgt tag found");
        return None;
    }

    let vcgt = &icc_data[vcgt_offset..];
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
        if vcgt.len() < 18 + channels * entry_count * entry_size { return None; }

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
        // Formula type: gamma, min, max per channel (each as u16.u16 fixed point)
        if vcgt.len() < 12 + 3 * 6 { return None; }
        eprintln!("ICC: vcgt formula type");
        for ch in 0..3 {
            let base = 12 + ch * 6;
            let gamma = read_u16_be(vcgt, base) as f64 / 256.0
                + read_u16_be(vcgt, base + 2) as f64 / 65536.0;
            let min = read_u16_be(vcgt, base + 2) as f64 / 65535.0;
            let max = read_u16_be(vcgt, base + 4) as f64 / 65535.0;
            for i in 0..256 {
                let x = i as f64 / 255.0;
                let y = min + (max - min) * x.powf(gamma);
                ramp[ch][i] = (y * 65535.0).round() as u16;
            }
        }
    } else {
        eprintln!("ICC: Unknown vcgt type: {}", gamma_type);
        return None;
    }

    Some(ramp)
}

fn apply_icc_profile(profile_path: &str, device_id: Option<String>) -> Result<(), String> {
    use windows::Win32::UI::ColorSystem::*;
    use windows::core::PCWSTR;

    let profile_path_buf = PathBuf::from(profile_path);
    let file_name = profile_path_buf
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid profile path")?
        .to_string();

    let color_dir = "C:\\Windows\\System32\\spool\\drivers\\color";
    let dest_path = format!("{}\\{}", color_dir, file_name);

    if !std::path::Path::new(&dest_path).exists() {
        fs::copy(profile_path, &dest_path)
            .map_err(|e| format!("Failed to copy ICC profile: {}", e))?;
    }

    // 读取 ICC 文件解析 vcgt
    let icc_data = fs::read(&dest_path)
        .map_err(|e| format!("Failed to read ICC file: {}", e))?;

    let pnp_id = resolve_device_pnp(device_id.clone())?;
    let device_id_w: Vec<u16> = pnp_id.encode_utf16().chain(std::iter::once(0)).collect();
    let file_name_w: Vec<u16> = file_name.encode_utf16().chain(std::iter::once(0)).collect();

    eprintln!("ICC: Applying profile: {} to PnP: {}", file_name, pnp_id);

    unsafe {
        // 1. WCS 注册（让颜色管理面板同步）
        let dest_path_w: Vec<u16> = dest_path.encode_utf16().chain(std::iter::once(0)).collect();
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
            // 同步给 nvidia 调节层，后续滑块调节将在此基础上叠加
            crate::nvidia::set_icc_base_ramp(Some(ramp));

            // 获取目标显示器的 adapter device_id (\\.\DISPLAYx)
            let adapter_id = device_id.unwrap_or_else(|| {
                get_display_monitors()
                    .ok()
                    .and_then(|ms| ms.into_iter().find(|m| m.is_primary).map(|m| m.device_id))
                    .unwrap_or_default()
            });

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
                let _ = windows::Win32::Graphics::Gdi::DeleteDC(hdc);
            } else {
                eprintln!("ICC: Failed to create DC for adapter: {}", adapter_id);
            }
        } else {
            eprintln!("ICC: No vcgt in profile, only WCS registration done");
            // 没有 vcgt，清空基础 ramp（等同于线性）
            crate::nvidia::set_icc_base_ramp(None);
            // 触发 calibration loader
            let _ = WcsSetCalibrationManagementState(false);
            let _ = WcsSetCalibrationManagementState(true);
        }

        Ok(())
    }
}

#[tauri::command]
pub async fn set_icc_profile(profile_path: String, device_id: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || apply_icc_profile(&profile_path, device_id))
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

#[tauri::command]
pub async fn restore_default_icc_profile(device_id: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let color_dir = PathBuf::from("C:\\Windows\\System32\\spool\\drivers\\color");

        let default_profile = [
            "sRGB Color Space Profile.icm",
            "sRGB IEC61966-2.1.icc",
        ]
        .iter()
        .map(|name| color_dir.join(name))
        .find(|p| p.exists())
        .ok_or("Default sRGB profile not found".to_string())?;

        let profile_path = default_profile.to_string_lossy().to_string();
        eprintln!("ICC: Restoring default = {}", profile_path);

        apply_icc_profile(&profile_path, device_id)?;

        // 恢复数字震动为NVIDIA面板默认50%（DVC=31）
        crate::nvidia::set_nvidia_digital_vibrance(0, 50)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// 用资源管理器打开系统 ICC 目录
#[tauri::command]
pub fn open_icc_directory() -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg("C:\\Windows\\System32\\spool\\drivers\\color")
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

    let color_dir = PathBuf::from("C:\\Windows\\System32\\spool\\drivers\\color");
    let dest = color_dir.join(&file_name);

    fs::copy(&src, &dest).map_err(|e| format!("Failed to copy ICC profile: {}", e))?;

    eprintln!("ICC: Imported '{}' -> '{}'", src_path, dest.display());
    Ok(file_name)
}

/// 导出 ICC 文件到指定目录
/// profile_name: ICC 文件名（不含路径），dest_dir: 目标目录路径
#[tauri::command]
pub fn export_icc_profile(profile_name: String, dest_dir: String) -> Result<String, String> {
    let color_dir = PathBuf::from("C:\\Windows\\System32\\spool\\drivers\\color");
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
