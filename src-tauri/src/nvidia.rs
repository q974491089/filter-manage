use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NvidiaSettings {
    pub brightness: i32,       // -125 to 125 (maps to 0.0~1.0, default 0 = 0.5)
    pub contrast: i32,         // -82 to 82   (maps to 0.0~1.0, default 0 = 0.5)
    pub gamma: f64,            // 0.4 to 2.8  (direct, default 1.0)
    pub digital_vibrance: i32, // 0 to 100
    /// RGB 增益内部统一标度：-100 ~ +100，0 = 无偏色
    #[serde(default)]
    pub rgb_r: i32,
    #[serde(default)]
    pub rgb_g: i32,
    #[serde(default)]
    pub rgb_b: i32,
}

impl Default for NvidiaSettings {
    fn default() -> Self {
        Self {
            brightness: 0,
            contrast: 0,
            gamma: 1.0,
            digital_vibrance: 50,
            rgb_r: 0,
            rgb_g: 0,
            rgb_b: 0,
        }
    }
}

// Per-display settings: key = device_id (e.g. "\\.\DISPLAY1")
static DISPLAY_SETTINGS: Mutex<Option<HashMap<String, NvidiaSettings>>> = Mutex::new(None);
static DISPLAY_ICC_RAMPS: Mutex<Option<HashMap<String, [[u16; 256]; 3]>>> = Mutex::new(None);

// ─── 颜色写门闩（Task 2）──────────────────────────────────────────────────────
//
// 所有会改写屏幕颜色的外部路径——滑块亮度/对比度/伽马/RGB、DVC、ICC、恢复默认、
// 托盘/快捷键/进程监听的整组方案应用、手动显示模式——都要持闩执行；每次外部写入
// 在闩内递增 APPLY_GENERATION（process_watcher::note_color_applied），让恢复调度
// 立即感知「屏幕被别人接管了」。恢复写入走 recovery_write_ramp：同一门闩内复核
// 代号后写缓存的最终 LUT，不递增代号。
//
// 锁序约定（两条方向都单向，杜绝死锁）：
// - 门闩内部绝不锁 process_watcher 的状态锁；
// - 持状态锁的代码也绝不持闩跨 I/O（watcher 的恢复 tick 先放状态锁再取闩）。
static COLOR_WRITE_LATCH: Mutex<()> = Mutex::new(());

/// 每台显示器最近一次**成功**写入的最终合成 LUT（含 ICC vcgt 与 RGB 增益）。
/// 恢复调度用它作为「期望曲线」；写入失败不更新缓存（屏幕仍是旧曲线）。
static FINAL_RAMPS: Mutex<Option<HashMap<String, [u16; 768]>>> = Mutex::new(None);

/// 外部颜色写入的持闩入口：取闩并在闩内递增 APPLY_GENERATION。
/// 返回的 guard 必须保持到写入结束；整组方案应用也持同一把闩（内部走已持闩
/// helper，不得再取公开入口）。
pub(crate) fn begin_external_write() -> std::sync::MutexGuard<'static, ()> {
    let guard = COLOR_WRITE_LATCH.lock().unwrap();
    crate::process_watcher::note_color_applied();
    guard
}

/// watcher 的整组应用事务：成功 apply 与快照捕获都在外部写门闩内完成。
/// `apply` 返回实际写入的 device_id；`capture` 不得重新选择目标显示器、重取
/// 公开颜色写入口或锁 watcher 状态。快照只能使用传入的本次 generation。
/// apply 失败时不调用 capture；成功但无可用快照时保留 Ok(None)。
pub(crate) fn with_external_write_snapshot<S>(
    apply: impl FnOnce() -> Result<String, String>,
    capture: impl FnOnce(&str, u64) -> Option<S>,
) -> Result<Option<S>, String> {
    with_write_snapshot(
        begin_external_write,
        apply,
        crate::process_watcher::apply_generation,
        capture,
    )
}

// 生产路径与局部 mutex/假写测试共用的事务边界；G 持有写门闩直到 capture 完成。
fn with_write_snapshot<G, S>(
    begin_write: impl FnOnce() -> G,
    apply: impl FnOnce() -> Result<String, String>,
    generation: impl FnOnce() -> u64,
    capture: impl FnOnce(&str, u64) -> Option<S>,
) -> Result<Option<S>, String> {
    let latch = begin_write();
    let device_id = apply()?;
    let snapshot = capture(&device_id, generation());
    drop(latch);
    Ok(snapshot)
}

/// 查询某显示器最近一次成功写入的最终 LUT；从未成功写入时返回 None。
/// 缓存本身不代表本次 apply 成功，也不能与闩外采样的 generation 拼成所有权；
/// watcher 必须通过 with_external_write_snapshot 在成功 apply 后、持闩时捕获。
pub(crate) fn final_ramp_for_display(device_id: &str) -> Option<[u16; 768]> {
    FINAL_RAMPS
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(device_id))
        .copied()
}

/// 在门闩作用域内缓存最终 LUT。调用方必须已持闩（apply_gamma_ramp_locked 与
/// icc.rs 的 ICC 直写路径都在闩内调用）。
pub(crate) fn cache_final_ramp(device_id: &str, ramp: &[u16; 768]) {
    FINAL_RAMPS
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(device_id.to_string(), *ramp);
}

/// 运行时回读某显示器的当前 gamma ramp（768 个 u16，R/G/B 各 256）。
/// 不依赖调试记录是否开启；读取失败返回 Err —— 调用方不得把失败当成「一致」。
pub(crate) fn read_gamma_ramp(device_id: &str) -> Result<[u16; 768], String> {
    use windows::Win32::Graphics::Gdi::{CreateDCW, DeleteDC};
    use windows::Win32::UI::ColorSystem::GetDeviceGammaRamp;
    let wide: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let hdc = CreateDCW(windows::core::PCWSTR(wide.as_ptr()), None, None, None);
        if hdc.is_invalid() {
            return Err(format!("CreateDCW failed for {device_id}"));
        }
        let mut values = [0u16; 768];
        let ok = GetDeviceGammaRamp(hdc, values.as_mut_ptr() as *mut _);
        let _ = DeleteDC(hdc);
        if ok.as_bool() {
            Ok(values)
        } else {
            Err(format!("GetDeviceGammaRamp failed for {device_id}"))
        }
    }
}

/// 恢复写入的失败形态：代号在门闩内复核时已变化（所有权作废）或真实写入失败。
#[derive(Debug)]
pub(crate) enum RecoveryWriteError {
    GenerationChanged,
    WriteFailed(String),
}

/// 恢复调度专用写入：同一门闩内完成「复核代号 → 写入缓存的最终 LUT」，消除
/// 检查与写入之间被外部写入插队的竞态。不递增代号、不重跑方案、不切分辨率、
/// 不重新注册 ICC、不重复 DVC。调用方（watcher 线程）不得持有状态锁。
pub(crate) fn recovery_write_ramp(
    device_id: &str,
    ramp: &[u16; 768],
    expected_generation: u64,
    backend: crate::color_backend::Backend,
) -> Result<(), RecoveryWriteError> {
    let _latch = COLOR_WRITE_LATCH.lock().unwrap();
    perform_recovery_write(expected_generation, backend, crate::process_watcher::apply_generation(),
        crate::color_backend::active_backend(device_id),
        || crate::color_backend::write_ramp_locked(device_id, ramp))
}

fn perform_recovery_write(
    expected_generation: u64, owned_backend: crate::color_backend::Backend,
    current_generation: u64, current_backend: crate::color_backend::Backend,
    write: impl FnOnce() -> Result<(), String>,
) -> Result<(), RecoveryWriteError> {
    if current_generation != expected_generation {
        return Err(RecoveryWriteError::GenerationChanged);
    }
    if current_backend != owned_backend {
        return Err(RecoveryWriteError::GenerationChanged);
    }
    write().map_err(RecoveryWriteError::WriteFailed)
}

#[cfg(test)]
mod backend_routing_tests {
    use super::*;
    use crate::color_backend::Backend;
    use std::cell::Cell;

    #[test]
    fn stale_generation_and_backend_never_reach_the_writer() {
        let writes = Cell::new(0);
        let first = perform_recovery_write(5, Backend::Nvidia, 6, Backend::Nvidia, || { writes.set(1); Ok(()) });
        assert!(matches!(first, Err(RecoveryWriteError::GenerationChanged)));
        let second = perform_recovery_write(5, Backend::Standard, 5, Backend::Nvidia, || { writes.set(1); Ok(()) });
        assert!(matches!(second, Err(RecoveryWriteError::GenerationChanged)));
        assert_eq!(writes.get(), 0);
    }

    #[test]
    fn current_owner_reaches_one_writer_and_preserves_driver_error() {
        let writes = Cell::new(0);
        let result = perform_recovery_write(5, Backend::Nvidia, 5, Backend::Nvidia, || {
            writes.set(writes.get() + 1); Err("NVAPI -104".into())
        });
        assert_eq!(writes.get(), 1);
        assert!(matches!(result, Err(RecoveryWriteError::WriteFailed(error)) if error == "NVAPI -104"));
    }

    #[test]
    fn shared_renderer_preserves_neutral_curve_and_distinct_vcgt_channels() {
        let neutral = NvidiaSettings::default();
        assert_eq!(compose_gamma_ramp(&neutral, None), crate::color_backend::identity_ramp());
        let base = std::array::from_fn(|channel| std::array::from_fn(|i| match channel {
            0 => (i * 128) as u16,
            1 => (i * 257) as u16,
            _ => ((i * 257 + 16384).min(65535)) as u16,
        }));
        let curve = compose_gamma_ramp(&neutral, Some(&base));
        assert_eq!(curve[128], 16384);
        assert_eq!(curve[256 + 128], 32896);
        assert_eq!(curve[512 + 128], 49280);
    }

    #[test]
    fn rgb_gain_does_not_modify_other_channels_and_gamma_keeps_endpoints() {
        let normal = compose_gamma_ramp(&NvidiaSettings::default(), None);
        let mut settings = NvidiaSettings::default();
        settings.rgb_r = 100;
        let gained = compose_gamma_ramp(&settings, None);
        assert_eq!(normal[256..], gained[256..]);
        assert!(gained[128] > normal[128]);
        settings.rgb_r = 0;
        settings.gamma = 2.0;
        let gamma = compose_gamma_ramp(&settings, None);
        assert_eq!(gamma[0], 0);
        assert_eq!(gamma[255], 65535);
        assert!(gamma[128] > normal[128]);
    }
}

/// CreateDCW + SetDeviceGammaRamp + 即时回读验证（验证日志仅在调试记录开启时落盘）。
pub(crate) fn write_gamma_ramp_raw(device_id: &str, ramp: &[u16; 768]) -> Result<(), String> {
    unsafe {
        let device_w: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();
        let hdc = windows::Win32::Graphics::Gdi::CreateDCW(
            windows::core::PCWSTR(device_w.as_ptr()),
            None,
            None,
            None,
        );
        if hdc.is_invalid() {
            return Err(format!("Failed to create DC for display: {device_id}"));
        }
        let result =
            windows::Win32::UI::ColorSystem::SetDeviceGammaRamp(hdc, ramp.as_ptr() as *const _);
        let _ = windows::Win32::Graphics::Gdi::DeleteDC(hdc);
        crate::debug_recorder::log(
            "gamma",
            format!(
                "device={device_id} SetDeviceGammaRamp 返回={}（TRUE≠生效，见 verify 行）",
                result.as_bool()
            ),
        );
        crate::debug_recorder::verify_ramp(device_id, ramp);
        if result.as_bool() {
            Ok(())
        } else {
            Err(format!("SetDeviceGammaRamp failed for {device_id}"))
        }
    }
}

#[allow(dead_code)]
fn get_settings_map() -> std::sync::MutexGuard<'static, Option<HashMap<String, NvidiaSettings>>> {
    DISPLAY_SETTINGS.lock().unwrap()
}

pub(crate) fn get_or_default_settings(device_id: &str) -> NvidiaSettings {
    let map = DISPLAY_SETTINGS.lock().unwrap();
    map.as_ref()
        .and_then(|m| m.get(device_id))
        .cloned()
        .unwrap_or_default()
}

fn update_settings(device_id: &str, f: impl FnOnce(&mut NvidiaSettings)) -> NvidiaSettings {
    let mut map = DISPLAY_SETTINGS.lock().unwrap();
    let m = map.get_or_insert_with(HashMap::new);
    let s = m
        .entry(device_id.to_string())
        .or_insert_with(NvidiaSettings::default);
    f(s);
    s.clone()
}

pub(crate) fn replace_settings(device_id: &str, settings: NvidiaSettings) {
    update_settings(device_id, |current| *current = settings);
}

pub(crate) fn icc_base_for_display(device_id: &str) -> Option<[[u16; 256]; 3]> {
    DISPLAY_ICC_RAMPS.lock().unwrap().as_ref().and_then(|m| m.get(device_id)).copied()
}

/// 由 icc.rs 调用，设置指定显示器的 ICC vcgt 基础 ramp
pub fn set_icc_base_ramp_for_display(device_id: &str, ramp: Option<[[u16; 256]; 3]>) {
    let mut map = DISPLAY_ICC_RAMPS.lock().unwrap();
    let m = map.get_or_insert_with(HashMap::new);
    match ramp {
        Some(r) => {
            m.insert(device_id.to_string(), r);
        }
        None => {
            m.remove(device_id);
        }
    }
}

/// 兼容旧调用：设置主显示器的 ICC base ramp
#[allow(dead_code)]
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
const NVAPI_ID_GET_ASSOCIATED_DISPLAY_HANDLE: u32 = 0x35C29134;
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
/// NVAPI 的 NvDisplayHandle 是不透明指针（x64 上 8 字节）。曾误声明为 u32，
/// 驱动会往 4 字节的栈槽写 8 字节，是未定义行为，且句柄高 32 位被截断。
type NvDisplayHandle = *mut std::ffi::c_void;
type NvEnumDisplayHandle =
    unsafe extern "C" fn(this_enum: i32, p_nv_disp_handle: *mut NvDisplayHandle) -> i32;
type NvGetAssociatedDisplayHandle = unsafe extern "C" fn(
    sz_display_name: *const std::ffi::c_char,
    p_nv_disp_handle: *mut NvDisplayHandle,
) -> i32;
type NvSetDvcLevel =
    unsafe extern "C" fn(h_nv_disp: NvDisplayHandle, output_id: u32, level: i32) -> i32;
type NvSetDvcLevelEx = unsafe extern "C" fn(
    h_nv_disp: NvDisplayHandle,
    output_id: u32,
    p_dvc_info: *mut NvDvcInfoEx,
) -> i32;
type NvGetDvcInfoEx = unsafe extern "C" fn(
    h_nv_disp: NvDisplayHandle,
    output_id: u32,
    p_dvc_info: *mut NvDvcInfoEx,
) -> i32;

/// 取得指定显示器的 NVAPI display handle。
///
/// 优先用 `NvAPI_GetAssociatedNvidiaDisplayHandle` 按 Windows 设备名（`\\.\DISPLAYn`）精确匹配。
/// 不能把 `\\.\DISPLAYn` 里的编号当成 `NvAPI_EnumNvidiaDisplayHandle` 的索引 —— 后者只枚举
/// 已连接的 NVIDIA 输出，两套编号没有对应关系。主屏是 `\\.\DISPLAY2` 时按编号取索引 1 会拿到
/// NVAPI_END_ENUMERATION(-7)，数字振动等 NVAPI 调用直接全部失效。
/// 按名字取不到时回退到第一个 NVIDIA 显示器，保证单显示器场景可用。
fn nvapi_load_for_display(
    device_id: Option<&str>,
) -> Result<
    (
        windows::Win32::Foundation::HMODULE,
        NvDisplayHandle,
        NvQueryInterface,
    ),
    String,
> {
    unsafe {
        let lib =
            windows::Win32::System::LibraryLoader::LoadLibraryW(windows::core::w!("nvapi64.dll"))
                .map_err(|e| format!("nvapi64.dll not found: {}", e))?;

        let query_ptr = windows::Win32::System::LibraryLoader::GetProcAddress(
            lib,
            windows::core::s!("nvapi_QueryInterface"),
        )
        .ok_or("nvapi_QueryInterface not found")?;
        let query_fn: NvQueryInterface = std::mem::transmute(query_ptr);

        let init_ptr = query_fn(NVAPI_ID_INITIALIZE);
        if init_ptr.is_null() {
            let _ = windows::Win32::Foundation::FreeLibrary(lib);
            return Err("NvAPI_Initialize not found".into());
        }
        let init_fn: NvInitialize = std::mem::transmute(init_ptr);
        let init_status = init_fn();
        if init_status != 0 {
            let _ = windows::Win32::Foundation::FreeLibrary(lib);
            return Err(format!("NvAPI_Initialize failed: status={}", init_status));
        }

        // 1) 指定了显示器 → 必须按设备名精确取到句柄，取不到就报错。
        //    不回退到"第一个 NVIDIA 显示器"：调用方点名了某台屏，悄悄改另一台
        //    比明确失败更糟（虚拟显示器、Intel 输出的屏都会走到这里）。
        let assoc_ptr = query_fn(NVAPI_ID_GET_ASSOCIATED_DISPLAY_HANDLE);
        if !assoc_ptr.is_null() {
            if let Some(name) = device_id {
                let c_name = match std::ffi::CString::new(name) {
                    Ok(c) => c,
                    Err(_) => {
                        let _ = windows::Win32::Foundation::FreeLibrary(lib);
                        return Err(format!("invalid display name: {}", name));
                    }
                };
                let assoc_fn: NvGetAssociatedDisplayHandle = std::mem::transmute(assoc_ptr);
                let mut handle: NvDisplayHandle = std::ptr::null_mut();
                let status = assoc_fn(c_name.as_ptr(), &mut handle);
                if status == 0 {
                    return Ok((lib, handle, query_fn));
                }
                let _ = windows::Win32::Foundation::FreeLibrary(lib);
                return Err(format!(
                    "display {} is not driven by NVIDIA (GetAssociatedNvidiaDisplayHandle status={})",
                    name, status
                ));
            }
        }

        // 2) 没指定显示器，或老驱动缺 GetAssociated 入口 → 用第一个 NVIDIA 显示器兜底
        let enum_ptr = query_fn(NVAPI_ID_ENUM_DISPLAY_HANDLE);
        if enum_ptr.is_null() {
            let _ = windows::Win32::Foundation::FreeLibrary(lib);
            return Err("NvAPI_EnumNvidiaDisplayHandle not found".into());
        }
        let enum_fn: NvEnumDisplayHandle = std::mem::transmute(enum_ptr);
        let mut handle: NvDisplayHandle = std::ptr::null_mut();
        let status = enum_fn(0, &mut handle);
        if status != 0 {
            let _ = windows::Win32::Foundation::FreeLibrary(lib);
            return Err(format!(
                "no NVIDIA display found (EnumNvidiaDisplayHandle status={})",
                status
            ));
        }

        Ok((lib, handle, query_fn))
    }
}

/// 读取驱动的 DVC 信息（min/max/default/current）
fn nvapi_get_dvc_info(device_id: Option<&str>) -> Result<(i32, i32, i32, i32), String> {
    unsafe {
        let (lib, handle, query_fn) = nvapi_load_for_display(device_id)?;

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
            Ok((
                info.min_level,
                info.max_level,
                info.default_level,
                info.current_level,
            ))
        } else {
            Err(format!(
                "GetDVCInfoEx failed: status={} (display={})",
                status,
                device_id.unwrap_or("<primary>")
            ))
        }
    }
}

/// UI 0..100 → 驱动标度 [min, max]，与 `sync_dvc_from_driver` 的反向换算对称。
///
/// NVIDIA 目前报的就是 0..100，换算等价于恒等；但读取路径一直在做归一化，
/// 写入不做就是不对称的 —— 哪天驱动改了标度（或接 AMD 的 ADL，它的范围由驱动给），
/// 写入会静默失真而不报错。
pub(crate) fn ui_to_driver_level(ui: i32, min: i32, max: i32) -> i32 {
    let ui = ui.clamp(0, 100);
    if max > min {
        min + (ui * (max - min)) / 100
    } else {
        ui
    }
}

/// 驱动标度 → UI 0..100，`ui_to_driver_level` 的反向换算。
/// 拿不到有效范围时退回 50（面板默认位置），而不是 0 —— 0 是"完全去色"。
fn driver_to_ui_level(value: i32, min: i32, max: i32) -> i32 {
    let range = max - min;
    if range > 0 {
        ((value - min) * 100 / range).clamp(0, 100)
    } else {
        50
    }
}

/// `ui_level` 是 UI 标度的 0..100，换算成驱动标度后写入。
fn nvapi_set_dvc(ui_level: i32, device_id: Option<&str>) -> Result<(), String> {
    unsafe {
        let (lib, handle, query_fn) = nvapi_load_for_display(device_id)?;

        // 先尝试 SetDVCLevelEx（新接口，直接传结构体，范围与面板一致）
        let set_ex_ptr = query_fn(NVAPI_ID_SET_DVC_LEVEL_EX);
        let (status, api) = if !set_ex_ptr.is_null() {
            // 先读取当前 info 获取 min/max
            let get_ptr = query_fn(NVAPI_ID_GET_DVC_INFO_EX);
            let mut info = NvDvcInfoEx {
                version: (std::mem::size_of::<NvDvcInfoEx>() as u32) | 0x10000,
                current_level: 0, // 等 min/max 确定后再按驱动标度换算
                min_level: 0,
                max_level: 100,
                default_level: 50,
            };
            if !get_ptr.is_null() {
                let get_fn: NvGetDvcInfoEx = std::mem::transmute(get_ptr);
                let mut cur = NvDvcInfoEx {
                    version: (std::mem::size_of::<NvDvcInfoEx>() as u32) | 0x10000,
                    current_level: 0,
                    min_level: 0,
                    max_level: 100,
                    default_level: 50,
                };
                if get_fn(handle, 0, &mut cur) == 0 {
                    info.min_level = cur.min_level;
                    info.max_level = cur.max_level;
                    info.default_level = cur.default_level;
                }
            }
            info.current_level = ui_to_driver_level(ui_level, info.min_level, info.max_level);
            let set_ex_fn: NvSetDvcLevelEx = std::mem::transmute(set_ex_ptr);
            (set_ex_fn(handle, 0, &mut info), "SetDVCLevelEx")
        } else {
            // 回退到旧接口
            let set_ptr = query_fn(NVAPI_ID_SET_DVC_LEVEL);
            if set_ptr.is_null() {
                let _ = windows::Win32::Foundation::FreeLibrary(lib);
                return Err("NvAPI_SetDVCLevel not found".into());
            }
            // 旧接口标度是 0..63，且与 Ex 的 0..100 不是线性对应（本机实测 0→Ex50、
            // 63→Ex100，即旧接口只能加饱和、不能降），没法从 UI 值精确还原。
            // 只有缺 SetDVCLevelEx 的老驱动才会走到这里，按比例缩放兜底。
            let legacy_level = (ui_level.clamp(0, 100) * 63) / 100;
            let set_fn: NvSetDvcLevel = std::mem::transmute(set_ptr);
            (set_fn(handle, 0, legacy_level), "SetDVCLevel")
        };

        let _ = windows::Win32::Foundation::FreeLibrary(lib);
        if status == 0 {
            Ok(())
        } else {
            Err(format!(
                "{} failed: status={} (display={}, ui_level={})",
                api,
                status,
                device_id.unwrap_or("<primary>"),
                ui_level
            ))
        }
    }
}

/// 亮度/对比度与伽马独立计算，与 NVIDIA 面板行为一致：
/// - 亮度/对比度：线性偏移/缩放
/// - 伽马：独立幂函数，不与亮度/对比度耦合
fn calculate_lut(brightness: f64, contrast: f64, gamma: f64) -> [u16; 256] {
    let contrast = (contrast.clamp(0.0, 1.0) - 0.5) * 2.0; // -1..1
    let brightness = (brightness.clamp(0.0, 1.0) - 0.5) * 2.0; // -1..1

    // 亮度/对比度线性变换（不含伽马）
    let offset = if contrast > 0.0 {
        contrast * -25.4
    } else {
        contrast * -32.0
    };
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

/// RGB 增益 -100..+100 → 通道 scale（约 0.52..1.48，兼顾驱动可接受范围）
fn rgb_gain_to_scale(gain: i32) -> f64 {
    1.0 + (gain.clamp(-100, 100) as f64 / 100.0) * 0.48
}

/// 按当前设置合成并写入 gamma ramp（已持闩版本；调用方必须先 `begin_external_write`）。
/// 写入成功后缓存最终 LUT（含 ICC vcgt 与 RGB 增益）作为恢复调度的期望曲线；
/// 写入失败不缓存 —— 初次 apply 失败的显示器不会有恢复任务。
pub(crate) fn build_gamma_ramp(device_id: &str) -> [u16; 768] {
    let s = get_or_default_settings(device_id);
    let base = icc_base_for_display(device_id);
    let ramp = compose_gamma_ramp(&s, base.as_ref());
    crate::debug_recorder::log("gamma", format!("device={device_id} 亮度={} 对比度={} 伽马={} RGB=({},{},{}) icc_base={} 中间调R={}",
        s.brightness, s.contrast, s.gamma, s.rgb_r, s.rgb_g, s.rgb_b, base.is_some(), ramp[128] >> 8));
    ramp
}

/// Shared numeric renderer: A/B/default preparation all use the same mapping.
pub(crate) fn compose_gamma_ramp(s: &NvidiaSettings, icc_base: Option<&[[u16; 256]; 3]>) -> [u16; 768] {
    // UI range -> WindowsDisplayAPI range
    let b = (s.brightness as f64 + 125.0) / 250.0;
    let c = (s.contrast as f64 + 82.0) / 164.0;
    let g = s.gamma.clamp(0.4, 2.8);

    let lut = calculate_lut(b, c, g);
    let scales = [
        rgb_gain_to_scale(s.rgb_r),
        rgb_gain_to_scale(s.rgb_g),
        rgb_gain_to_scale(s.rgb_b),
    ];

    // 在 ICC vcgt 基础上叠加调节：把 lut 作为索引映射应用到 icc_base，再乘通道增益
    let mut ramp = [0u16; 768];
    for i in 0..256 {
        for (ch, offset) in [(0usize, 0usize), (1, 256), (2, 512)] {
            let base_val = if let Some(base) = icc_base {
                let idx = (lut[i] as usize * 255 / 65535).min(255);
                base[ch][idx]
            } else {
                lut[i]
            };
            let scaled = (base_val as f64 * scales[ch]).round().clamp(0.0, 65535.0) as u16;
            ramp[offset + i] = scaled;
        }
    }
    ramp
}

pub(crate) fn apply_gamma_ramp_locked(device_id: &str) -> Result<(), String> {
    let ramp = build_gamma_ramp(device_id);
    let result = crate::color_backend::write_ramp_locked(device_id, &ramp);
    if result.is_ok() {
        cache_final_ramp(device_id, &ramp);
    }
    result
}

// ─── 滑块命令：公开持闩入口 + 私有已持闩 helper ───────────────────────────────
//
// 整组方案应用（tray::apply_color_config_from）持闩调用 *_locked helper，
// 不经公开入口重取闩（Mutex 不可重入）。

pub(crate) fn set_brightness_locked(did: &str, value: i32) -> Result<(), String> {
    update_and_apply(did, |s| s.brightness = value.clamp(-125, 125))
}

pub(crate) fn set_contrast_locked(did: &str, value: i32) -> Result<(), String> {
    update_and_apply(did, |s| s.contrast = value.clamp(-82, 82))
}

pub(crate) fn set_gamma_locked(did: &str, value: f64) -> Result<(), String> {
    if !value.is_finite() { return Err("Gamma must be finite".into()); }
    update_and_apply(did, |s| s.gamma = value.clamp(0.4, 2.8))
}

pub(crate) fn set_rgb_gain_locked(did: &str, r: i32, g: i32, b: i32) -> Result<(), String> {
    update_and_apply(did, |s| {
        s.rgb_r = r.clamp(-100, 100);
        s.rgb_g = g.clamp(-100, 100);
        s.rgb_b = b.clamp(-100, 100);
    })
}

fn update_and_apply(did: &str, update: impl FnOnce(&mut NvidiaSettings)) -> Result<(), String> {
    let previous = get_or_default_settings(did);
    update_settings(did, update);
    let result = apply_gamma_ramp_locked(did);
    if result.is_err() { replace_settings(did, previous); }
    result
}

#[tauri::command]
pub async fn set_nvidia_brightness(device_id: Option<String>, value: i32) -> Result<(), String> {
    let did = resolve_display_id(device_id);
    tauri::async_runtime::spawn_blocking(move || {
        let _latch = begin_external_write();
        set_brightness_locked(&did, value)
    }).await.map_err(|e| format!("Brightness task failed: {e}"))?
}

#[tauri::command]
pub async fn set_nvidia_contrast(device_id: Option<String>, value: i32) -> Result<(), String> {
    let did = resolve_display_id(device_id);
    tauri::async_runtime::spawn_blocking(move || {
        let _latch = begin_external_write();
        set_contrast_locked(&did, value)
    }).await.map_err(|e| format!("Contrast task failed: {e}"))?
}

#[tauri::command]
pub async fn set_nvidia_gamma(device_id: Option<String>, value: f64) -> Result<(), String> {
    let did = resolve_display_id(device_id);
    tauri::async_runtime::spawn_blocking(move || {
        let _latch = begin_external_write();
        set_gamma_locked(&did, value)
    }).await.map_err(|e| format!("Gamma task failed: {e}"))?
}

/// 设置 RGB 增益（内部标度 -100..+100，0 = 无偏色）
#[tauri::command]
pub async fn set_nvidia_rgb_gain(
    device_id: Option<String>,
    r: i32,
    g: i32,
    b: i32,
) -> Result<(), String> {
    let did = resolve_display_id(device_id);
    tauri::async_runtime::spawn_blocking(move || {
        let _latch = begin_external_write();
        set_rgb_gain_locked(&did, r, g, b)
    }).await.map_err(|e| format!("RGB task failed: {e}"))?
}

// ─── 数字振动后端分派 ─────────────────────────────────────────────────────────
//
// NVIDIA 叫 Digital Vibrance，AMD 叫 Saturation，都是同一个旋钮。前端命令名沿用
// `*_nvidia_*` 且签名不变；这里按"NVIDIA 优先，NVIDIA 明确不在时转 AMD ADLX"分派。
// "明确不在"指没有驱动、初始化失败、或这台显示器不由 NVIDIA 输出 —— 混合输出机器上
// 每台屏各自归属，所以判断是按显示器做的，不是按机器。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DvcVendor {
    Nvidia,
    Amd,
}

impl DvcVendor {
    fn as_str(self) -> &'static str {
        match self {
            DvcVendor::Nvidia => "nvidia",
            DvcVendor::Amd => "amd",
        }
    }
}

fn nvidia_absent(err: &str) -> bool {
    err.contains("nvapi64.dll not found")
        || err.contains("NvAPI_Initialize failed")
        || err.contains("is not driven by NVIDIA")
        || err.contains("no NVIDIA display found")
}

/// `(min, max, default, current)` 及命中的厂商。
fn dvc_get_info(did: &str) -> Result<(DvcVendor, (i32, i32, i32, i32)), String> {
    match nvapi_get_dvc_info(Some(did)) {
        Ok(v) => Ok((DvcVendor::Nvidia, v)),
        Err(nv) if nvidia_absent(&nv) => crate::amd::get_saturation_info(did)
            .map(|v| (DvcVendor::Amd, v))
            .map_err(|amd| format!("NVIDIA: {} | AMD: {}", nv, amd)),
        Err(e) => Err(e),
    }
}

/// DVC 写入的已持闩版本（调用方必须先 `begin_external_write`）。
pub(crate) fn dvc_set_locked(did: &str, ui_level: i32) -> Result<(), String> {
    let outcome = match nvapi_set_dvc(ui_level, Some(did)) {
        Ok(()) => Ok(()),
        Err(nv) if nvidia_absent(&nv) => crate::amd::set_saturation_ui(did, ui_level)
            .map_err(|amd| format!("NVIDIA: {} | AMD: {}", nv, amd)),
        Err(e) => Err(e),
    };
    // 数字振动是独立于 gamma LUT 的另一条管线，记录它才能在调试报告里
    // 对比「gamma 死了 DVC 活着」这类关键症状
    crate::debug_recorder::log(
        "dvc",
        format!(
            "device={did} 目标={ui_level} 结果={}{}",
            if outcome.is_ok() { "成功" } else { "失败" },
            outcome
                .as_ref()
                .err()
                .map(|e| format!("：{e}"))
                .unwrap_or_default()
        ),
    );
    outcome
}

#[tauri::command]
pub async fn set_nvidia_digital_vibrance(device_id: Option<String>, value: i32) -> Result<(), String> {
    let did = resolve_display_id(device_id);
    tauri::async_runtime::spawn_blocking(move || {
        let _latch = begin_external_write();
        let value = value.clamp(0, 100);
        dvc_set_locked(&did, value)?;
        update_settings(&did, |s| s.digital_vibrance = value);
        Ok(())
    }).await.map_err(|e| format!("Digital vibrance task failed: {e}"))?
}

/// 解析 device_id，fallback 到主显示器
pub(crate) fn resolve_display_id(device_id: Option<String>) -> String {
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
    match dvc_get_info(&did) {
        Ok((_, (min, max, default, current))) => {
            let ui_value = driver_to_ui_level(current, min, max);
            eprintln!(
                "[DVC] display={} min={} max={} default={} current={} => ui={}",
                did, min, max, default, current, ui_value
            );
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
    match dvc_get_info(&did) {
        Ok((_, (min, max, default, _))) => driver_to_ui_level(default, min, max),
        Err(_) => 50,
    }
}

#[tauri::command]
pub fn get_nvidia_settings(device_id: Option<String>) -> Result<NvidiaSettings, String> {
    let did = resolve_display_id(device_id);
    Ok(get_or_default_settings(&did))
}

/// 数字振动的可用性与驱动值域。
#[derive(Debug, Serialize)]
pub struct DvcCapability {
    pub supported: bool,
    /// 命中的后端："nvidia" / "amd"；不支持时为 None。前端据此决定标签文案
    pub vendor: Option<String>,
    /// 不支持时给用户看的一句话
    pub reason: Option<String>,
    /// 驱动实际标度（UI 始终是 0..100），仅供诊断展示
    pub driver_min: i32,
    pub driver_max: i32,
    /// 驱动默认值换算到 UI 标度的结果
    pub default_ui_value: i32,
}

/// 探测当前显示器能不能调数字振动/饱和度，以及由哪家驱动接管。
///
/// 前端据此决定滑块是否可交互与标签文案：Intel 机器上两家驱动都不在，
/// 让用户每拉一次滑块弹一次错误只是噪音 —— 直接禁用并说明原因才对。
#[tauri::command]
pub fn get_dvc_capability(device_id: Option<String>) -> DvcCapability {
    let did = resolve_display_id(device_id);
    match dvc_get_info(&did) {
        Ok((vendor, (min, max, default, _))) => DvcCapability {
            supported: true,
            vendor: Some(vendor.as_str().to_string()),
            reason: None,
            driver_min: min,
            driver_max: max,
            default_ui_value: driver_to_ui_level(default, min, max),
        },
        Err(e) => {
            eprintln!("[DVC] capability probe failed for {}: {}", did, e);
            DvcCapability {
                supported: false,
                vendor: None,
                reason: Some(humanize_dvc_error(&e)),
                driver_min: 0,
                driver_max: 100,
                default_ui_value: 50,
            }
        }
    }
}

/// 把驱动层的原始失败翻译成用户能看懂的话。分派失败时原始串形如
/// `NVIDIA: ... | AMD: ...`，两边都要看。原始串仍会进 stderr，诊断时看日志。
fn humanize_dvc_error(err: &str) -> String {
    let no_nvidia_driver =
        err.contains("nvapi64.dll not found") || err.contains("NvAPI_Initialize failed");
    let no_amd_driver = err.contains("amdadlx64.dll not found");
    let not_nvidia_display =
        err.contains("is not driven by NVIDIA") || err.contains("no NVIDIA display found");

    if no_nvidia_driver && no_amd_driver {
        "未检测到 NVIDIA 或 AMD 显卡驱动，数字振动 / 饱和度不可用".to_string()
    } else if err.contains("saturation not supported") || err.contains("custom color not supported")
    {
        "当前 AMD 驱动或显示器不支持饱和度调节（可尝试更新 Adrenalin 驱动、关闭 HDR）".to_string()
    } else if err.contains("not found among") || err.contains("cannot identify") {
        "无法在 AMD 驱动中定位当前显示器，饱和度调节不可用".to_string()
    } else if err.contains("ADLXInitialize failed") {
        "AMD ADLX 初始化失败，饱和度调节不可用（驱动可能过旧）".to_string()
    } else if not_nvidia_display && no_amd_driver {
        "当前显示器不由 NVIDIA 显卡输出，无法调节数字振动".to_string()
    } else {
        format!("数字振动 / 饱和度不可用：{}", err)
    }
}

#[cfg(test)]
mod recovery_snapshot_tests {
    use super::with_write_snapshot;
    use crate::color_recovery::RecoveryOwnership;
    use std::cell::Cell;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{mpsc, Mutex, TryLockError};
    use std::thread;
    use std::time::Duration;

    const TARGET: &str = "test-display-written-by-apply";
    const WATCHER_LUT: [u16; 768] = [12345; 768];
    const MANUAL_LUT: [u16; 768] = [54321; 768];
    const WAIT: Duration = Duration::from_secs(10);

    #[test]
    fn snapshot_precedes_manual_generation_and_manual_write_invalidates_owner() {
        let latch = Mutex::new(());
        let generation = AtomicU64::new(0);
        let final_lut = Mutex::new([0; 768]);
        let manual_was_blocked = Cell::new(false);
        let (start_tx, start_rx) = mpsc::channel();
        let (probe_tx, probe_rx) = mpsc::channel();
        let (advanced_tx, advanced_rx) = mpsc::channel();
        let (finish_tx, finish_rx) = mpsc::channel();

        thread::scope(|scope| {
            let (latch, generation, final_lut) = (&latch, &generation, &final_lut);
            let manual = scope.spawn(move || {
                start_rx.recv_timeout(WAIT).unwrap();
                // Probe at the snapshot boundary, not at a timing-dependent sleep.
                let guard = match latch.try_lock() {
                    Ok(guard) => {
                        generation.fetch_add(1, Ordering::SeqCst);
                        probe_tx.send(false).unwrap();
                        guard
                    }
                    Err(TryLockError::WouldBlock) => {
                        probe_tx.send(true).unwrap();
                        let guard = latch.lock().unwrap();
                        generation.fetch_add(1, Ordering::SeqCst);
                        guard
                    }
                    Err(TryLockError::Poisoned(err)) => panic!("{err}"),
                };
                // Model the real vulnerable interval: new generation, old LUT.
                advanced_tx.send(()).unwrap();
                finish_rx.recv_timeout(WAIT).unwrap();
                *final_lut.lock().unwrap() = MANUAL_LUT;
                drop(guard);
            });

            let owner = with_write_snapshot(
                || {
                    let guard = latch.lock().unwrap();
                    generation.fetch_add(1, Ordering::SeqCst);
                    guard
                },
                || {
                    *final_lut.lock().unwrap() = WATCHER_LUT;
                    Ok(TARGET.to_string())
                },
                || {
                    start_tx.send(()).unwrap();
                    manual_was_blocked.set(probe_rx.recv_timeout(WAIT).unwrap());
                    generation.load(Ordering::SeqCst)
                },
                |device_id, generation| {
                    Some(RecoveryOwnership {
                        backend: crate::color_backend::Backend::Standard,
                        rule_id: "rule".into(),
                        activation_seq: 1,
                        device_id: device_id.into(),
                        pnp_id: "test-pnp".into(),
                        generation,
                        expected_lut: *final_lut.lock().unwrap(),
                    })
                },
            )
            .unwrap()
            .unwrap();

            advanced_rx.recv_timeout(WAIT).unwrap();
            let during_manual_generation = generation.load(Ordering::SeqCst);
            let during_manual_lut = *final_lut.lock().unwrap();
            // Always release/join the writer before assertions, including on RED.
            finish_tx.send(()).unwrap();
            manual.join().unwrap();

            assert_eq!(
                owner.generation, 1,
                "must not adopt the manual write's generation"
            );
            assert!(
                manual_was_blocked.get(),
                "snapshot must retain the write latch"
            );
            assert_eq!(owner.device_id, TARGET);
            assert_eq!(owner.expected_lut, WATCHER_LUT);
            assert_eq!(during_manual_lut, WATCHER_LUT);
            assert_eq!(during_manual_generation, 2);
            assert_ne!(
                owner.generation, during_manual_generation,
                "owner must already be invalid while manual LUT write is pending"
            );
            assert_eq!(*final_lut.lock().unwrap(), MANUAL_LUT);
        });
    }

    #[test]
    fn failed_apply_never_captures_even_when_an_old_lut_exists() {
        let latch = Mutex::new(());
        let old_lut = WATCHER_LUT;
        let captured = Cell::new(false);
        let result = with_write_snapshot(
            || latch.lock().unwrap(),
            || Err("initial apply failed".to_string()),
            || panic!("generation must not be sampled after failed apply"),
            |_, generation| {
                captured.set(true);
                Some((generation, old_lut))
            },
        );
        assert_eq!(result, Err("initial apply failed".to_string()));
        assert!(!captured.get());
        assert!(latch.try_lock().is_ok(), "failure must release the latch");
    }

    #[test]
    fn successful_apply_without_snapshot_is_not_an_apply_error() {
        let latch = Mutex::new(());
        let result = with_write_snapshot(
            || latch.lock().unwrap(),
            || Ok(TARGET.to_string()),
            || 7,
            |device_id, generation| {
                assert_eq!(device_id, TARGET);
                assert_eq!(generation, 7);
                None::<RecoveryOwnership>
            },
        );
        assert!(matches!(result, Ok(None)));
        assert!(latch.try_lock().is_ok());
    }
}
