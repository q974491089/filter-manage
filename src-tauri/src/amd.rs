//! AMD 显示器饱和度控制 —— 通过 ADLX（`amdadlx64.dll`）。
//!
//! 为什么是 ADLX 而不是老的 ADL（`atiadlxx.dll`）：RX 7900 XT 实测
//! `ADL_Display_ColorCaps_Get` 报 caps=0x30，只剩色温可调，饱和度/亮度/对比度/色调
//! 全部"不支持"—— RDNA3 起颜色控制已经整体迁到 ADLX。同一台机器上 ADLX 报饱和度
//! 范围 [0,200]、当前 100，接口完整可用。
//!
//! ADLX 是 COM 风格的 C 接口：对象首字段是虚表指针，方法按 SDK 头文件顺序排列。
//! 本文件不引入 ADLX SDK，直接按槽位索引调用。槽位取自官方头文件，v1.0 与 v1.5
//! 布局一致，并已用诊断脚本在真机验证（`tools/gpu-color-diag.bat` 第 4 节）。
//!
//! 生命周期：`ADLXInitialize` 只做一次，`IADLXSystem` / `IADLXDisplayServices` 进程内
//! 常驻，退出时由 [`shutdown`] 统一 Release + `ADLXTerminate`。显示器列表每次调用
//! 重新枚举，热插拔安全。

use std::ffi::{c_char, c_void, CStr};
use std::ptr::{null, null_mut};
use std::sync::Mutex;

use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

// ─── ADLX 常量（来自 ADLXDefines.h / ADLXVersion.h）────────────────────────────

/// ADLX_MAKE_FULL_VER(1, 5, 0, 124)：本文件依据的 SDK 版本
const ADLX_SDK_VERSION: u64 = (1u64 << 48) | (5u64 << 32) | (0u64 << 16) | 124;

const ADLX_OK: i32 = 0;
const ADLX_BAD_VER: i32 = 5;
const ADLX_NOT_SUPPORTED: i32 = 12;

const RESULT_NAMES: [&str; 19] = [
    "OK", "ALREADY_ENABLED", "ALREADY_INITIALIZED", "FAIL", "INVALID_ARGS", "BAD_VER",
    "UNKNOWN_INTERFACE", "TERMINATED", "ADL_INIT_ERROR", "NOT_FOUND", "INVALID_OBJECT",
    "ORPHAN_OBJECTS", "NOT_SUPPORTED", "PENDING_OPERATION", "GPU_INACTIVE", "GPU_IN_USE",
    "TIMEOUT_OPERATION", "NOT_ACTIVE", "RESET_NEEDED",
];

fn result_name(r: i32) -> String {
    match usize::try_from(r).ok().and_then(|i| RESULT_NAMES.get(i)) {
        Some(n) => format!("{} (ADLX_{})", r, n),
        None => r.to_string(),
    }
}

// 虚表槽位。IADLXSystem 没有 Acquire/Release，其余接口前三槽固定是
// Acquire / Release / QueryInterface。
const SYS_GET_DISPLAYS_SERVICES: usize = 3;
const DS_GET_DISPLAYS: usize = 4;
const DS_GET_CUSTOM_COLOR: usize = 16;
const LIST_SIZE: usize = 3;
const LIST_AT_DISPLAY: usize = 11;
const DISP_NAME: usize = 6;
const CC_IS_SATURATION_SUPPORTED: usize = 7;
const CC_GET_SATURATION_RANGE: usize = 8;
const CC_GET_SATURATION: usize = 9;
const CC_SET_SATURATION: usize = 10;
const IFACE_RELEASE: usize = 1;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct AdlxIntRange {
    min: i32,
    max: i32,
    step: i32,
}

// 入口函数是 __cdecl，虚表方法是 __stdcall；x64 上二者 ABI 相同，
// 分开写只是为了表意。adlx_bool 是 1 字节，用 u8 接，不能用 Rust bool。
type FnQueryFullVersion = unsafe extern "C" fn(*mut u64) -> i32;
type FnInitialize = unsafe extern "C" fn(u64, *mut *mut c_void) -> i32;
type FnTerminate = unsafe extern "C" fn() -> i32;

type MRelease = unsafe extern "system" fn(*mut c_void) -> i32;
type MGetObj = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32;
type MGetObjFor = unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> i32;
type MSize = unsafe extern "system" fn(*mut c_void) -> u32;
type MAt = unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32;
type MName = unsafe extern "system" fn(*mut c_void, *mut *const c_char) -> i32;
type MGetBool = unsafe extern "system" fn(*mut c_void, *mut u8) -> i32;
type MGetRange = unsafe extern "system" fn(*mut c_void, *mut AdlxIntRange) -> i32;
type MGetInt = unsafe extern "system" fn(*mut c_void, *mut i32) -> i32;
type MSetInt = unsafe extern "system" fn(*mut c_void, i32) -> i32;

// ─── 虚表访问与 RAII ─────────────────────────────────────────────────────────

/// 读对象虚表第 `index` 槽并转成函数指针类型 `T`。
///
/// 调用方保证 `obj` 是有效的 ADLX 对象、`T` 是与该槽签名一致的函数指针类型。
unsafe fn vslot<T: Copy>(obj: *mut c_void, index: usize) -> Result<T, String> {
    if obj.is_null() {
        return Err("ADLX: null object".into());
    }
    let vtbl = *(obj as *const *const *const c_void);
    if vtbl.is_null() {
        return Err("ADLX: null vtable".into());
    }
    let f = *vtbl.add(index);
    if f.is_null() {
        return Err(format!("ADLX: vtable slot {} is null", index));
    }
    Ok(std::mem::transmute_copy::<*const c_void, T>(&f))
}

fn check(status: i32, what: &str) -> Result<(), String> {
    if status == ADLX_OK {
        Ok(())
    } else {
        Err(format!("ADLX {} failed: {}", what, result_name(status)))
    }
}

/// 持有一个引用计数的 ADLX 对象，离开作用域自动 Release。
struct AdlxObj(*mut c_void);

impl Drop for AdlxObj {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }
        unsafe {
            if let Ok(rel) = vslot::<MRelease>(self.0, IFACE_RELEASE) {
                rel(self.0);
            }
        }
    }
}

// ─── 进程级运行时 ────────────────────────────────────────────────────────────

struct Runtime {
    /// 永不 FreeLibrary：ADLX 内部有工作线程，进程退出前卸载模块不安全
    _lib: HMODULE,
    /// ADLX 拥有，不需要 Release；Terminate 时由它回收
    _system: *mut c_void,
    /// 进程内常驻，退出时 Release
    display_services: *mut c_void,
    terminate: FnTerminate,
}

// 裸指针只在持有 RUNTIME 锁时使用，跨线程访问由 Mutex 串行化。
unsafe impl Send for Runtime {}

static RUNTIME: Mutex<Option<Runtime>> = Mutex::new(None);

unsafe fn init_runtime() -> Result<Runtime, String> {
    let lib = LoadLibraryW(windows::core::w!("amdadlx64.dll"))
        .map_err(|e| format!("amdadlx64.dll not found: {}", e))?;

    let fail = |lib: HMODULE, msg: String| -> Result<Runtime, String> {
        let _ = FreeLibrary(lib);
        Err(msg)
    };

    let init_ptr = match GetProcAddress(lib, windows::core::s!("ADLXInitialize")) {
        Some(p) => p,
        None => return fail(lib, "ADLXInitialize not found".into()),
    };
    let term_ptr = match GetProcAddress(lib, windows::core::s!("ADLXTerminate")) {
        Some(p) => p,
        None => return fail(lib, "ADLXTerminate not found".into()),
    };
    let init: FnInitialize = std::mem::transmute(init_ptr);
    let terminate: FnTerminate = std::mem::transmute(term_ptr);

    // 版本协商：先按 SDK 版本；驱动更老会报 BAD_VER，此时退回驱动自报的版本。
    // 本文件用到的槽位在 1.0 ~ 1.5 之间布局一致，降级是安全的。
    let mut system: *mut c_void = null_mut();
    let mut status = init(ADLX_SDK_VERSION, &mut system);
    if status == ADLX_BAD_VER {
        if let Some(qv_ptr) = GetProcAddress(lib, windows::core::s!("ADLXQueryFullVersion")) {
            let query: FnQueryFullVersion = std::mem::transmute(qv_ptr);
            let mut runtime_ver = 0u64;
            if query(&mut runtime_ver) == ADLX_OK && runtime_ver != 0 {
                eprintln!(
                    "[ADLX] SDK version rejected, retrying with driver version {:#x}",
                    runtime_ver
                );
                status = init(runtime_ver, &mut system);
            }
        }
    }
    if status != ADLX_OK || system.is_null() {
        return fail(lib, format!("ADLXInitialize failed: {}", result_name(status)));
    }

    let get_ds: MGetObj = match vslot(system, SYS_GET_DISPLAYS_SERVICES) {
        Ok(f) => f,
        Err(e) => {
            terminate();
            return fail(lib, e);
        }
    };
    let mut ds: *mut c_void = null_mut();
    let st = get_ds(system, &mut ds);
    if st != ADLX_OK || ds.is_null() {
        terminate();
        return fail(lib, format!("ADLX GetDisplaysServices failed: {}", result_name(st)));
    }

    eprintln!("[ADLX] initialized");
    Ok(Runtime {
        _lib: lib,
        _system: system,
        display_services: ds,
        terminate,
    })
}

/// 在持锁状态下使用运行时；首次调用时惰性初始化。初始化失败不缓存，下次再试
/// （LoadLibrary 失败很快，而用户可能中途装好驱动）。
fn with_runtime<R>(f: impl FnOnce(&Runtime) -> Result<R, String>) -> Result<R, String> {
    let mut guard = RUNTIME.lock().map_err(|_| "ADLX runtime mutex poisoned".to_string())?;
    if guard.is_none() {
        *guard = Some(unsafe { init_runtime()? });
    }
    f(guard.as_ref().expect("runtime just initialized"))
}

/// 应用退出时调用：Release 常驻对象并 `ADLXTerminate`，避免 ORPHAN_OBJECTS。
pub fn shutdown() {
    let Ok(mut guard) = RUNTIME.lock() else { return };
    if let Some(rt) = guard.take() {
        unsafe {
            if let Ok(rel) = vslot::<MRelease>(rt.display_services, IFACE_RELEASE) {
                rel(rt.display_services);
            }
            let st = (rt.terminate)();
            eprintln!("[ADLX] terminated: {}", result_name(st));
        }
    }
}

// ─── 显示器定位 ──────────────────────────────────────────────────────────────

/// Windows 设备名（`\\.\DISPLAYn`）→ EDID 型号名，如 "VG27AQML1A"。
/// ADLX 的 `IADLXDisplay::Name` 读的是同一个 EDID 描述符，两边可以直接比。
fn windows_monitor_name(device_id: &str) -> Option<String> {
    let monitors = crate::icc::get_display_monitors().ok()?;
    let m = monitors.into_iter().find(|m| m.device_id == device_id)?;
    crate::icc::get_monitor_name_from_edid(&m.pnp_id)
}

unsafe fn display_name(disp: &AdlxObj) -> Option<String> {
    let name_fn: MName = vslot(disp.0, DISP_NAME).ok()?;
    let mut p: *const c_char = null();
    if name_fn(disp.0, &mut p) != ADLX_OK || p.is_null() {
        return None;
    }
    // 字符串归 ADLX 所有，仅在 display 对象存活期间有效，立刻拷走
    Some(CStr::from_ptr(p).to_string_lossy().trim().to_string())
}

fn same_monitor(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// 在 ADLX 显示器列表里找到 Windows 设备 `device_id` 对应的那台。
///
/// 匹配规则：EDID 型号名相同即命中。Windows 侧取不到型号名且 ADLX 只有一台显示器时，
/// 用那一台兜底。型号名对不上则报错 —— 宁可失败，也不悄悄改到别的屏
/// （Intel 核显 + AMD 独显的混合输出机器上会遇到）。
///
/// 已知局限：两台同型号显示器会命中第一台。
unsafe fn find_display(rt: &Runtime, device_id: &str) -> Result<AdlxObj, String> {
    let get_displays: MGetObj = vslot(rt.display_services, DS_GET_DISPLAYS)?;
    let mut list_ptr: *mut c_void = null_mut();
    check(get_displays(rt.display_services, &mut list_ptr), "GetDisplays")?;
    if list_ptr.is_null() {
        return Err("ADLX GetDisplays returned null".into());
    }
    let list = AdlxObj(list_ptr);

    let size_fn: MSize = vslot(list.0, LIST_SIZE)?;
    let at_fn: MAt = vslot(list.0, LIST_AT_DISPLAY)?;
    let n = size_fn(list.0);
    if n == 0 {
        return Err("ADLX: no displays enumerated".into());
    }

    let want = windows_monitor_name(device_id);
    let mut seen: Vec<String> = Vec::with_capacity(n as usize);
    let mut sole: Option<AdlxObj> = None;

    for i in 0..n {
        let mut p: *mut c_void = null_mut();
        if at_fn(list.0, i, &mut p) != ADLX_OK || p.is_null() {
            continue;
        }
        let disp = AdlxObj(p);
        let name = display_name(&disp).unwrap_or_default();
        if let Some(w) = &want {
            if same_monitor(&name, w) {
                return Ok(disp);
            }
        }
        seen.push(name);
        if n == 1 {
            sole = Some(disp);
        }
    }

    match (want, sole) {
        (None, Some(d)) => Ok(d),
        (Some(w), _) => Err(format!(
            "ADLX: display {} ({}) not found among [{}]",
            device_id,
            w,
            seen.join(", ")
        )),
        (None, None) => Err(format!(
            "ADLX: cannot identify {} (no EDID name) among {} displays [{}]",
            device_id,
            n,
            seen.join(", ")
        )),
    }
}

unsafe fn custom_color(rt: &Runtime, disp: &AdlxObj) -> Result<AdlxObj, String> {
    let f: MGetObjFor = vslot(rt.display_services, DS_GET_CUSTOM_COLOR)?;
    let mut p: *mut c_void = null_mut();
    let st = f(rt.display_services, disp.0, &mut p);
    if st == ADLX_NOT_SUPPORTED {
        return Err("ADLX: custom color not supported on this display".into());
    }
    check(st, "GetCustomColor")?;
    if p.is_null() {
        return Err("ADLX GetCustomColor returned null".into());
    }
    Ok(AdlxObj(p))
}

/// 饱和度的值域与当前值。中性点/默认值取值域中点 —— ADLX 没有"默认值"查询接口，
/// 而饱和度本身以中点为中性（实测 [0,200] 中性 100）。
struct SaturationState {
    range: AdlxIntRange,
    current: i32,
}

unsafe fn read_saturation(cc: &AdlxObj) -> Result<SaturationState, String> {
    let is_supported: MGetBool = vslot(cc.0, CC_IS_SATURATION_SUPPORTED)?;
    let mut sup = 0u8;
    check(is_supported(cc.0, &mut sup), "IsSaturationSupported")?;
    if sup == 0 {
        return Err("ADLX: saturation not supported on this display".into());
    }

    let get_range: MGetRange = vslot(cc.0, CC_GET_SATURATION_RANGE)?;
    let mut range = AdlxIntRange::default();
    check(get_range(cc.0, &mut range), "GetSaturationRange")?;
    if range.max <= range.min {
        return Err(format!(
            "ADLX: invalid saturation range [{}, {}]",
            range.min, range.max
        ));
    }

    let get_cur: MGetInt = vslot(cc.0, CC_GET_SATURATION)?;
    let mut current = 0i32;
    check(get_cur(cc.0, &mut current), "GetSaturation")?;

    Ok(SaturationState { range, current })
}

fn midpoint(r: &AdlxIntRange) -> i32 {
    r.min + (r.max - r.min) / 2
}

/// 把驱动标度的值对齐到 step 网格并夹在值域内。
fn snap(value: i32, r: &AdlxIntRange) -> i32 {
    let v = value.clamp(r.min, r.max);
    if r.step > 1 {
        r.min + ((v - r.min) / r.step) * r.step
    } else {
        v
    }
}

// ─── 对外接口（供 nvidia.rs 的分派层调用）──────────────────────────────────────

/// 读指定显示器的饱和度信息：`(min, max, default, current)`，与
/// `nvidia::nvapi_get_dvc_info` 同形，便于分派层统一处理。
pub(crate) fn get_saturation_info(device_id: &str) -> Result<(i32, i32, i32, i32), String> {
    with_runtime(|rt| unsafe {
        let disp = find_display(rt, device_id)?;
        let cc = custom_color(rt, &disp)?;
        let s = read_saturation(&cc)?;
        Ok((s.range.min, s.range.max, midpoint(&s.range), s.current))
    })
}

/// 按 UI 标度（0..100）设置饱和度：内部读驱动值域、换算、对齐 step 后写入。
pub(crate) fn set_saturation_ui(device_id: &str, ui_level: i32) -> Result<(), String> {
    with_runtime(|rt| unsafe {
        let disp = find_display(rt, device_id)?;
        let cc = custom_color(rt, &disp)?;
        let s = read_saturation(&cc)?;
        let target = snap(
            crate::nvidia::ui_to_driver_level(ui_level, s.range.min, s.range.max),
            &s.range,
        );
        let set: MSetInt = vslot(cc.0, CC_SET_SATURATION)?;
        check(set(cc.0, target), "SetSaturation").map_err(|e| {
            format!("{} (display={}, ui_level={}, target={})", e, device_id, ui_level, target)
        })
    })
}
