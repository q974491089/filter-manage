//! NVIDIA 私有输出颜色接口。只承载已有 Gamma LUT，不计算 sRGB 色域压缩。
//!
//! Display ID 映射来自 NVIDIA 官方 nvapi.h / nvapi_interface.h：
//! NvAPI_DISP_GetDisplayIdByDisplayName(const char *, NvU32 *)，ID 0xAE457190。
//! CSC 为未公开接口；调用方必须做 SDR/设备身份校验，且不能把返回成功理解成
//! 面板实际生效。这里独立实现 ABI 与曲线插值，不包含上游 GPL 实现代码。

use serde::{Deserialize, Serialize};
use std::ffi::{c_char, c_void, CString};
use std::mem::{align_of, size_of};
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{
    GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32,
};

const NVAPI_INITIALIZE: u32 = 0x0150_E828;
const NVAPI_DISPLAY_ID_BY_NAME: u32 = 0xAE45_7190;
const NVAPI_GET_CSC: u32 = 0x8159_E87A;
const NVAPI_SET_CSC: u32 = 0xFCAB_D23A;
const CURVE_POINTS: usize = 1024;
const CHANNELS: usize = 3;
const CURVE_VALUES: usize = CURVE_POINTS * CHANNELS;
const BUFFER_VALUES: usize = CURVE_VALUES * 2;
const CSC_V1_VERSION: u32 = 0x0001_007C;
const CSC_V2_VERSION: u32 = 0x0002_00A0;

type QueryInterface = unsafe extern "C" fn(u32) -> *mut c_void;
type Initialize = unsafe extern "C" fn() -> i32;
type DisplayIdByName = unsafe extern "C" fn(*const c_char, *mut u32) -> i32;
type CscCall = unsafe extern "C" fn(u32, *mut c_void) -> i32;

/// V1 不含指针，可保存到本机恢复记录；恢复前再次校验结构及 idle 状态。
#[repr(C)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct CscV1 {
    version: u32,
    content_color_space: u32,
    monitor_color_space: u32,
    unknown1: u32,
    unknown2: u32,
    use_matrix1: u32,
    matrix1: [f32; 12],
    use_matrix2: u32,
    matrix2: [f32; 12],
}

impl CscV1 {
    fn empty() -> Self {
        Self {
            version: CSC_V1_VERSION,
            content_color_space: 2,
            monitor_color_space: 0,
            unknown1: 0,
            unknown2: 0,
            use_matrix1: 0,
            matrix1: [0.0; 12],
            use_matrix2: 0,
            matrix2: [0.0; 12],
        }
    }

    fn validate(&self) -> Result<(), String> {
        if self.version != CSC_V1_VERSION {
            return Err(format!(
                "unexpected NVIDIA CSC V1 version: {:#x}",
                self.version
            ));
        }
        if self.use_matrix1 > 1 || self.use_matrix2 > 1 {
            return Err("unexpected NVIDIA CSC matrix enable flags".to_string());
        }
        if !(1..=12).contains(&self.content_color_space) || self.monitor_color_space > 12 {
            return Err("unexpected NVIDIA CSC color-space identifiers".to_string());
        }
        if !self
            .matrix1
            .iter()
            .chain(&self.matrix2)
            .all(|value| value.is_finite())
        {
            return Err("NVIDIA CSC contains non-finite matrix values".to_string());
        }
        Ok(())
    }

    fn is_idle(&self) -> bool {
        self.use_matrix1 == 0
            && self.use_matrix2 == 0
            && (self.monitor_color_space == 0
                || (self.content_color_space == 12 && self.monitor_color_space == 12))
    }

    fn matches_ramp_pipeline(&self) -> bool {
        self.content_color_space == 2
            && self.monitor_color_space == 2
            && self.use_matrix1 == 1
            && self.use_matrix2 == 0
            && self
                .matrix1
                .iter()
                .zip(identity_matrix())
                .all(|(actual, desired)| (*actual - desired).abs() <= 0.000001)
    }
}

/// V1 prefix followed by V2 pointers. On x64 the first pointer starts at 128.
#[repr(C)]
struct CscV2 {
    prefix: CscV1,
    degamma: *mut f32,
    regamma: *mut f32,
    buffer: *mut f32,
    buffer_size: i32,
}

impl CscV2 {
    fn with_buffer(buffer: &mut [f32; BUFFER_VALUES]) -> Self {
        Self {
            prefix: CscV1 {
                version: CSC_V2_VERSION,
                content_color_space: 2,
                monitor_color_space: 2,
                use_matrix1: 1,
                matrix1: identity_matrix(),
                ..CscV1::empty()
            },
            degamma: buffer.as_mut_ptr(),
            // The offset is measured in f32 entries; buffer_size is in bytes.
            regamma: unsafe { buffer.as_mut_ptr().add(CURVE_VALUES) },
            buffer: buffer.as_mut_ptr(),
            buffer_size: (BUFFER_VALUES * size_of::<f32>()) as i32,
        }
    }
}

fn identity_matrix() -> [f32; 12] {
    [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]
}

struct Module(HMODULE);

impl Drop for Module {
    fn drop(&mut self) {
        // The owning reference is obtained by LoadLibraryExW, not GetModuleHandle.
        if let Err(error) = unsafe { FreeLibrary(self.0) } {
            eprintln!("[nvidia-csc] FreeLibrary failed: {error}");
        }
    }
}

struct Api {
    _module: Module,
    display_id_by_name: DisplayIdByName,
    get_csc: CscCall,
    set_csc: CscCall,
}

impl Api {
    fn load() -> Result<Self, String> {
        validate_layout()?;
        // Only search the Windows system directory, never the cwd or ICC folder.
        let module = Module(
            unsafe {
                LoadLibraryExW(
                    windows::core::w!("nvapi64.dll"),
                    None,
                    LOAD_LIBRARY_SEARCH_SYSTEM32,
                )
            }
            .map_err(|error| format!("cannot load system nvapi64.dll: {error}"))?,
        );
        let query_pointer =
            unsafe { GetProcAddress(module.0, windows::core::s!("nvapi_QueryInterface")) }
                .ok_or_else(|| "nvapi_QueryInterface is unavailable".to_string())?;
        let query: QueryInterface = unsafe { std::mem::transmute(query_pointer) };

        let initialize: Initialize =
            unsafe { std::mem::transmute(entry(query, NVAPI_INITIALIZE, "NvAPI_Initialize")?) };
        check_status(unsafe { initialize() }, "NvAPI_Initialize", "<driver>")?;
        let display_id_by_name: DisplayIdByName = unsafe {
            std::mem::transmute(entry(
                query,
                NVAPI_DISPLAY_ID_BY_NAME,
                "NvAPI_DISP_GetDisplayIdByDisplayName",
            )?)
        };
        let get_csc: CscCall = unsafe {
            std::mem::transmute(entry(
                query,
                NVAPI_GET_CSC,
                "NvAPI_GPU_GetColorSpaceConversion",
            )?)
        };
        let set_csc: CscCall = unsafe {
            std::mem::transmute(entry(
                query,
                NVAPI_SET_CSC,
                "NvAPI_GPU_SetColorSpaceConversion",
            )?)
        };
        Ok(Self {
            _module: module,
            display_id_by_name,
            get_csc,
            set_csc,
        })
    }

    fn display_id(&self, device_id: &str) -> Result<u32, String> {
        if device_id.is_empty() || !device_id.is_ascii() {
            return Err("NVIDIA display name must be a nonempty ASCII device name".to_string());
        }
        let name = CString::new(device_id)
            .map_err(|_| "NVIDIA display name contains an embedded NUL".to_string())?;
        let mut display_id = 0;
        check_status(
            unsafe { (self.display_id_by_name)(name.as_ptr(), &mut display_id) },
            "NvAPI_DISP_GetDisplayIdByDisplayName",
            device_id,
        )?;
        if display_id == 0 {
            return Err(format!(
                "NvAPI_DISP_GetDisplayIdByDisplayName returned displayId=0 (display={device_id})"
            ));
        }
        Ok(display_id)
    }

    fn get_v1(&self, device_id: &str, display_id: u32) -> Result<CscV1, String> {
        let mut state = CscV1::empty();
        check_status(
            unsafe { (self.get_csc)(display_id, (&mut state as *mut CscV1).cast()) },
            "NvAPI_GPU_GetColorSpaceConversion(V1)",
            device_id,
        )?;
        state.validate()?;
        Ok(state)
    }
}

fn entry(query: QueryInterface, id: u32, name: &str) -> Result<*mut c_void, String> {
    let pointer = unsafe { query(id) };
    if pointer.is_null() {
        Err(format!("{name} is unavailable (interface={id:#010x})"))
    } else {
        Ok(pointer)
    }
}

fn check_status(status: i32, api: &str, device_id: &str) -> Result<(), String> {
    if status == 0 {
        Ok(())
    } else {
        let detail = match status {
            -104 => "NVAPI_NOT_SUPPORTED",
            -9 => "NVAPI_INCOMPATIBLE_STRUCT_VERSION",
            -8 => "NVAPI_INVALID_HANDLE",
            -7 => "NVAPI_END_ENUMERATION",
            -6 => "NVAPI_NVIDIA_DEVICE_NOT_FOUND",
            -5 => "NVAPI_INVALID_ARGUMENT",
            -4 => "NVAPI_API_NOT_INITIALIZED",
            -3 => "NVAPI_NO_IMPLEMENTATION",
            -2 => "NVAPI_LIBRARY_NOT_FOUND",
            _ => "driver error",
        };
        Err(format!(
            "{api} failed: status={status} ({detail}), display={device_id}"
        ))
    }
}

fn validate_layout() -> Result<(), String> {
    if size_of::<usize>() != 8
        || size_of::<CscV1>() != 124
        || size_of::<CscV2>() != 160
        || align_of::<CscV2>() != 8
    {
        return Err("NVIDIA CSC private ABI requires the verified Windows x64 layout".to_string());
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct Probe {
    pub(crate) display_id: u32,
    pub(crate) active_external_transform: bool,
}

/// Idle-only backup. Never treats a V1 snapshot as a backup of V2 gamma buffers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OriginalState {
    display_id: u32,
    state: CscV1,
}

pub(crate) fn probe(device_id: &str) -> Result<Probe, String> {
    let api = Api::load()?;
    let display_id = api.display_id(device_id)?;
    let state = api.get_v1(device_id, display_id)?;
    Ok(Probe {
        display_id,
        active_external_transform: !state.is_idle(),
    })
}

pub(crate) fn capture_idle(device_id: &str) -> Result<OriginalState, String> {
    let api = Api::load()?;
    let display_id = api.display_id(device_id)?;
    let state = api.get_v1(device_id, display_id)?;
    if !state.is_idle() {
        return Err(format!(
            "display={device_id} 已有无法完整备份的 NVIDIA 颜色变换（content={} monitor={} matrix1={} matrix2={}）；请检查 NVIDIA 控制面板或现有调色程序的设置，再尝试专用模式",
            state.content_color_space, state.monitor_color_space, state.use_matrix1, state.use_matrix2
        ));
    }
    Ok(OriginalState { display_id, state })
}

/// A live-session guard for changes to mode/matrix only. This deliberately does
/// not prove LUT ownership; a different application can use the same matrix.
/// Only call after this session has captured idle and successfully written its
/// own ramp. Crash cleanup must use is_current_ours instead.
pub(crate) fn matches_owned_pipeline(device_id: &str) -> Result<bool, String> {
    let api = Api::load()?;
    let display_id = api.display_id(device_id)?;
    Ok(api.get_v1(device_id, display_id)?.matches_ramp_pipeline())
}

/// Writes one complete snapshot of the already-composed legacy LUT.
pub(crate) fn write_ramp(device_id: &str, ramp: &[u16; 768]) -> Result<(), String> {
    let api = Api::load()?;
    let display_id = api.display_id(device_id)?;
    let mut buffer = make_curve_buffer(ramp);
    let mut payload = CscV2::with_buffer(&mut buffer);
    // buffer remains owned and pinned by Box throughout the synchronous driver call.
    check_status(
        unsafe { (api.set_csc)(display_id, (&mut payload as *mut CscV2).cast()) },
        "NvAPI_GPU_SetColorSpaceConversion(V2)",
        device_id,
    )
}

/// The caller must verify ownership before invoking restore; this function also
/// checks that the current device name still maps to the original NVIDIA output.
pub(crate) fn restore(device_id: &str, original: &OriginalState) -> Result<(), String> {
    original.state.validate()?;
    if !original.state.is_idle() {
        return Err("refusing to restore a non-idle NVIDIA CSC V1 snapshot".to_string());
    }
    let api = Api::load()?;
    let display_id = api.display_id(device_id)?;
    if original.display_id != display_id {
        return Err(format!(
            "NVIDIA output identity changed: previous={} current={display_id}, display={device_id}",
            original.display_id
        ));
    }
    let mut state = original.state;
    check_status(
        unsafe { (api.set_csc)(display_id, (&mut state as *mut CscV1).cast()) },
        "NvAPI_GPU_SetColorSpaceConversion(restore V1)",
        device_id,
    )
}

/// Full V2 readback is optional in private drivers. An error means ownership is
/// unverified; callers must never convert that error into a positive match.
pub(crate) fn is_current_ours(device_id: &str, expected: &[u16; 768]) -> Result<bool, String> {
    let api = Api::load()?;
    let display_id = api.display_id(device_id)?;
    let mut buffer = Box::new([f32::NAN; BUFFER_VALUES]);
    let mut payload = CscV2::with_buffer(&mut buffer);
    check_status(
        unsafe { (api.get_csc)(display_id, (&mut payload as *mut CscV2).cast()) },
        "NvAPI_GPU_GetColorSpaceConversion(V2 ownership)",
        device_id,
    )?;
    if payload.prefix.version != CSC_V2_VERSION
        || payload.buffer_size != (BUFFER_VALUES * size_of::<f32>()) as i32
        || !buffer.iter().all(|value| value.is_finite())
    {
        return Err(format!(
            "NVIDIA driver did not return complete CSC V2 curves (version={:#x}, bufferBytes={}, finiteSamples={}/{BUFFER_VALUES}, display={device_id})",
            payload.prefix.version,
            payload.buffer_size,
            buffer.iter().filter(|value| value.is_finite()).count()
        ));
    }
    let wanted = make_curve_buffer(expected);
    Ok(payload.prefix.content_color_space == 2
        && payload.prefix.monitor_color_space == 2
        && payload.prefix.use_matrix1 == 1
        && payload.prefix.use_matrix2 == 0
        && payload
            .prefix
            .matrix1
            .iter()
            .zip(identity_matrix())
            .all(|(actual, desired)| (*actual - desired).abs() <= 0.000001)
        && buffer
            .iter()
            .zip(wanted.iter())
            .all(|(actual, desired)| (*actual - *desired).abs() <= 1.0 / 65535.0))
}

fn make_curve_buffer(ramp: &[u16; 768]) -> Box<[f32; BUFFER_VALUES]> {
    let mut buffer = Box::new([0.0; BUFFER_VALUES]);
    for index in 0..CURVE_POINTS {
        let position = index as f64 / (CURVE_POINTS - 1) as f64;
        for channel in 0..CHANNELS {
            buffer[index * CHANNELS + channel] = position as f32;
            buffer[CURVE_VALUES + index * CHANNELS + channel] =
                sample_legacy_channel(ramp, channel, position) as f32;
        }
    }
    buffer
}

fn sample_legacy_channel(ramp: &[u16; 768], channel: usize, position: f64) -> f64 {
    let legacy_index = position * 255.0;
    let lower = legacy_index.floor() as usize;
    let upper = (lower + 1).min(255);
    let fraction = legacy_index - lower as f64;
    let base = channel * 256;
    let lower_value = ramp[base + lower] as f64;
    let upper_value = ramp[base + upper] as f64;
    (lower_value + (upper_value - lower_value) * fraction) / 65535.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_ramp() -> [u16; 768] {
        std::array::from_fn(|index| ((index % 256) * 257) as u16)
    }

    #[test]
    fn linear_ramp_stays_linear_after_resampling() {
        let curves = make_curve_buffer(&identity_ramp());
        for index in 0..1024 {
            let expected = index as f32 / 1023.0;
            for channel in 0..3 {
                assert!((curves[index * 3 + channel] - expected).abs() < 0.000001);
                assert!((curves[3072 + index * 3 + channel] - expected).abs() < 0.000001);
            }
        }
    }

    #[test]
    fn channels_and_endpoints_are_not_mixed() {
        let mut ramp = identity_ramp();
        ramp[..256].fill(65535);
        ramp[256..512].fill(0);
        ramp[512..768].fill(32768);
        let curves = make_curve_buffer(&ramp);
        for index in [0, 1, 512, 1023] {
            assert_eq!(curves[3072 + index * 3], 1.0);
            assert_eq!(curves[3072 + index * 3 + 1], 0.0);
            assert!((curves[3072 + index * 3 + 2] - 32768.0 / 65535.0).abs() < 0.000001);
        }
        assert!(curves
            .iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value)));
    }

    #[test]
    fn sampled_ramp_preserves_every_legacy_knot() {
        let ramp = std::array::from_fn(|index| {
            let channel = index / 256;
            let sample = (index % 256) as f64 / 255.0;
            (sample.powf(0.8 + channel as f64 * 0.7) * 65535.0).round() as u16
        });
        for channel in 0..3 {
            for index in 0..256 {
                let value = sample_legacy_channel(&ramp, channel, index as f64 / 255.0);
                assert!((value - ramp[channel * 256 + index] as f64 / 65535.0).abs() < 0.00000001);
            }
        }
    }

    #[test]
    fn x64_layout_has_the_expected_pointer_offsets_and_buffer_length() {
        assert_eq!(size_of::<CscV1>(), 124);
        assert_eq!(size_of::<CscV2>(), 160);
        assert_eq!(std::mem::offset_of!(CscV2, degamma), 128);
        assert_eq!(std::mem::offset_of!(CscV2, regamma), 136);
        assert_eq!(std::mem::offset_of!(CscV2, buffer), 144);
        let mut buffer = make_curve_buffer(&identity_ramp());
        let payload = CscV2::with_buffer(&mut buffer);
        assert_eq!(payload.buffer_size, 0x6000);
        assert_eq!(
            unsafe { payload.regamma.offset_from(payload.degamma) },
            3072
        );
    }

    #[test]
    fn existing_transform_is_not_an_idle_backup_even_with_identity_matrix() {
        let mut state = CscV1::empty();
        assert!(state.is_idle());
        state.monitor_color_space = 2;
        state.use_matrix1 = 1;
        state.matrix1 = identity_matrix();
        assert!(!state.is_idle());
        state.matrix1[0] = f32::NAN;
        assert!(state.validate().is_err());
    }

    #[test]
    fn idle_serialization_roundtrip_is_pointer_free_and_validated() {
        let original = OriginalState {
            display_id: 123,
            state: CscV1::empty(),
        };
        let encoded = serde_json::to_string(&original).unwrap();
        let decoded: OriginalState = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.display_id, 123);
        assert!(decoded.state.validate().is_ok());
        assert!(decoded.state.is_idle());
        assert_eq!(size_of::<CscV1>(), 124);
    }

    #[test]
    fn pipeline_guard_detects_mode_and_matrix_changes_without_claiming_lut_ownership() {
        let mut state = CscV1 {
            monitor_color_space: 2,
            use_matrix1: 1,
            matrix1: identity_matrix(),
            ..CscV1::empty()
        };
        assert!(state.matches_ramp_pipeline());
        state.matrix1[3] = 0.05;
        assert!(!state.matches_ramp_pipeline());
        state.matrix1 = identity_matrix();
        state.content_color_space = 12;
        assert!(!state.matches_ramp_pipeline());
        state.content_color_space = 2;
        state.use_matrix2 = 1;
        assert!(!state.matches_ramp_pipeline());
    }
}
