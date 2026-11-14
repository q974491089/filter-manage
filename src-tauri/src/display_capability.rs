//! Read-only mapping from an active GDI output to its monitor devnode.
//! A disabled monitor is not proof of NVIDIA ownership: query those separately.

use crate::display_identity::{
    connection_key, device_instance_from_monitor_path, monitor_problem_state,
};
use serde::Serialize;
use windows::Win32::Devices::Display::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DisplayCapability {
    pub device_id: String,
    pub stable_key: Option<String>,
    pub monitor_state: String,
    pub gpu_vendor: String,
    pub hdr_enabled: Option<bool>,
    pub advanced_color_enabled: Option<bool>,
    pub output_ambiguous: bool,
}

#[link(name = "user32")]
extern "system" {
    fn GetDisplayConfigBufferSizes(flags: i32, paths: *mut u32, modes: *mut u32) -> i32;
    fn QueryDisplayConfig(
        flags: i32,
        paths: *mut u32,
        path_array: *mut DISPLAYCONFIG_PATH_INFO,
        modes: *mut u32,
        mode_array: *mut DISPLAYCONFIG_MODE_INFO,
        topology: *mut i32,
    ) -> i32;
    fn DisplayConfigGetDeviceInfo(header: *mut DISPLAYCONFIG_DEVICE_INFO_HEADER) -> i32;
}

#[link(name = "cfgmgr32")]
extern "system" {
    fn CM_Locate_DevNodeW(devinst: *mut u32, instance_id: *mut u16, flags: u32) -> u32;
    fn CM_Get_DevNode_Status(status: *mut u32, problem: *mut u32, devinst: u32, flags: u32) -> u32;
}

// Windows SDK 10.0.26100 wingdi.h: GET_ADVANCED_COLOR_INFO_2 adds the active
// SDR/WCG/HDR mode. windows 0.58 does not expose it. Only this documented tail
// is declared here; the common header uses the official Windows type.
#[repr(C)]
struct AdvancedColorInfo2 {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    flags: u32,
    color_encoding: u32,
    bits_per_channel: u32,
    active_color_mode: u32,
}

fn string_from_wide(value: &[u16]) -> String {
    let end = value.iter().position(|c| *c == 0).unwrap_or(value.len());
    String::from_utf16_lossy(&value[..end])
}

fn active_paths() -> Result<Vec<DISPLAYCONFIG_PATH_INFO>, String> {
    let flags = (QDC_ONLY_ACTIVE_PATHS.0 | QDC_VIRTUAL_MODE_AWARE.0) as i32;
    for _ in 0..4 {
        let (mut path_count, mut mode_count) = (0, 0);
        let result =
            unsafe { GetDisplayConfigBufferSizes(flags, &mut path_count, &mut mode_count) };
        if result != 0 {
            return Err(format!("GetDisplayConfigBufferSizes winerr={result}"));
        }
        // A topology can change between the two calls; retry ERROR_INSUFFICIENT_BUFFER.
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
        let result = unsafe {
            QueryDisplayConfig(
                flags,
                &mut path_count,
                paths.as_mut_ptr(),
                &mut mode_count,
                modes.as_mut_ptr(),
                std::ptr::null_mut(),
            )
        };
        if result == 122 {
            continue;
        }
        if result != 0 {
            return Err(format!("QueryDisplayConfig winerr={result}"));
        }
        paths.truncate(path_count as usize);
        return Ok(paths);
    }
    Err("显示拓扑仍在变化，请稍后重新检查".into())
}

fn source_name(path: &DISPLAYCONFIG_PATH_INFO) -> Option<String> {
    let mut request = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
    request.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
    request.header.size = std::mem::size_of_val(&request) as u32;
    request.header.adapterId = path.sourceInfo.adapterId;
    request.header.id = path.sourceInfo.id;
    (unsafe { DisplayConfigGetDeviceInfo(&mut request.header) } == 0)
        .then(|| string_from_wide(&request.viewGdiDeviceName))
}

fn monitor_state(instance: &str) -> String {
    let mut name: Vec<_> = instance.encode_utf16().chain(Some(0)).collect();
    let (mut node, mut flags, mut problem) = (0, 0, 0);
    // NORMAL includes registered disabled devices; it does not require an enabled
    // monitor interface and does not scan unrelated monitors or enable anything.
    let result = unsafe { CM_Locate_DevNodeW(&mut node, name.as_mut_ptr(), 0) };
    if result != 0 {
        return "unknown".into();
    }
    let result = unsafe { CM_Get_DevNode_Status(&mut flags, &mut problem, node, 0) };
    if result != 0 {
        return "unknown".into();
    }
    monitor_problem_state(flags, problem).into()
}

fn gpu_vendor(path: &DISPLAYCONFIG_PATH_INFO) -> String {
    let mut request = DISPLAYCONFIG_ADAPTER_NAME::default();
    request.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_ADAPTER_NAME;
    request.header.size = std::mem::size_of_val(&request) as u32;
    request.header.adapterId = path.targetInfo.adapterId;
    request.header.id = path.targetInfo.id;
    if unsafe { DisplayConfigGetDeviceInfo(&mut request.header) } != 0 {
        return "unknown".into();
    }
    let adapter = string_from_wide(&request.adapterDevicePath).to_ascii_uppercase();
    if adapter.contains("PCI#VEN_10DE&") || adapter.contains("PCI\\VEN_10DE&") {
        "nvidia".into()
    } else if adapter.contains("PCI#VEN_") || adapter.contains("PCI\\VEN_") {
        "other".into()
    } else {
        "unknown".into()
    }
}

fn color_state(path: &DISPLAYCONFIG_PATH_INFO) -> (Option<bool>, Option<bool>) {
    let mut request = AdvancedColorInfo2 {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER::default(),
        flags: 0,
        color_encoding: 0,
        bits_per_channel: 0,
        active_color_mode: 0,
    };
    request.header.r#type = DISPLAYCONFIG_DEVICE_INFO_TYPE(15);
    request.header.size = std::mem::size_of_val(&request) as u32;
    request.header.adapterId = path.targetInfo.adapterId;
    request.header.id = path.targetInfo.id;
    if unsafe { DisplayConfigGetDeviceInfo(&mut request.header) } == 0 {
        return (
            Some(request.active_color_mode == 2),
            Some(request.flags & 2 != 0),
        );
    }
    // The older API reports all advanced color (including SDR ACM), not HDR
    // specifically. Never tell users HDR is enabled solely from this old bit.
    let mut legacy = DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO::default();
    legacy.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO;
    legacy.header.size = std::mem::size_of_val(&legacy) as u32;
    legacy.header.adapterId = path.targetInfo.adapterId;
    legacy.header.id = path.targetInfo.id;
    if unsafe { DisplayConfigGetDeviceInfo(&mut legacy.header) } != 0 {
        return (None, None);
    }
    let advanced = unsafe { legacy.Anonymous.value } & 2 != 0;
    (if advanced { None } else { Some(false) }, Some(advanced))
}

pub(crate) fn inspect(device: &str) -> Result<DisplayCapability, String> {
    let paths = active_paths()?;
    let matching: Vec<_> = paths
        .iter()
        .filter(|path| {
            source_name(path)
                .as_deref()
                .is_some_and(|name| name.eq_ignore_ascii_case(device))
        })
        .collect();
    let path = matching
        .first()
        .ok_or_else(|| format!("活动显示输出 '{device}' 不存在"))?;
    let output_ambiguous = matching.len() != 1;
    let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
    target.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
    target.header.size = std::mem::size_of_val(&target) as u32;
    target.header.adapterId = path.targetInfo.adapterId;
    target.header.id = path.targetInfo.id;
    let target_ok = unsafe { DisplayConfigGetDeviceInfo(&mut target.header) } == 0;
    let instance = target_ok
        .then(|| string_from_wide(&target.monitorDevicePath))
        .and_then(|path| device_instance_from_monitor_path(&path));
    let stable_key = if output_ambiguous {
        None
    } else {
        instance.as_ref().map(|instance| {
            connection_key(
                instance,
                target.outputTechnology.0,
                target.connectorInstance,
            )
        })
    };
    let (hdr_enabled, advanced_color_enabled) = color_state(path);
    Ok(DisplayCapability {
        device_id: device.into(),
        stable_key,
        monitor_state: instance
            .as_deref()
            .map(monitor_state)
            .unwrap_or_else(|| "unknown".into()),
        gpu_vendor: gpu_vendor(path),
        hdr_enabled,
        advanced_color_enabled,
        output_ambiguous,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn documented_advanced_color_extension_has_sdk_layout() {
        assert_eq!(std::mem::size_of::<AdvancedColorInfo2>(), 36);
        assert_eq!(
            std::mem::offset_of!(AdvancedColorInfo2, active_color_mode),
            32
        );
    }
}
