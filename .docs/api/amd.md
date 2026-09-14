# AMD 饱和度后端（ADLX）

> 对应源码：`src-tauri/src/amd.rs`。本模块**没有独立的 Tauri 命令**，由 `nvidia.rs` 的
> 分派层调用，前端仍走 `set_nvidia_digital_vibrance` / `get_dvc_capability` 等原有命令，
> 详见 [nvidia.md](./nvidia.md)。

## 为什么是 ADLX 而不是 ADL

| 接口 | DLL | RX 7900 XT 实测 |
|---|---|---|
| ADL（旧） | `atiadlxx.dll` | `ADL_Display_ColorCaps_Get` 报 caps=0x30，只剩色温；饱和度/亮度/对比度/色调全部不支持 |
| ADLX（新） | `amdadlx64.dll` | 饱和度范围 [0,200]、当前 100，接口完整可用 |

RDNA3 起颜色控制已整体迁到 ADLX，ADL 那条路在新卡上是死的。数据来源：
`tools/gpu-color-diag.bat` 第 3、4 节在用户机器上的输出（2026-09-13）。

## 调用方式

ADLX 是 COM 风格的 C 接口：对象首字段是虚表指针，方法按 SDK 头文件顺序排列。
本模块不引入 ADLX SDK，直接按槽位索引调用（`vslot`）。槽位取自官方头文件
（GPUOpen-LibrariesAndSDKs/ADLX，`SDK/Include/*.h`），v1.0 与 v1.5 布局一致。

用到的槽位：

| 接口 | 槽位 | 方法 |
|---|---|---|
| IADLXSystem | 3 | GetDisplaysServices（System 没有 Acquire/Release） |
| IADLXDisplayServices | 4 / 16 | GetDisplays / GetCustomColor |
| IADLXDisplayList | 3 / 11 | Size / At_DisplayList |
| IADLXDisplay | 6 | Name（EDID 型号名） |
| IADLXDisplayCustomColor | 7 / 8 / 9 / 10 | IsSaturationSupported / GetSaturationRange / GetSaturation / SetSaturation |
| 所有接口 | 1 | Release |

注意 `adlx_bool` 是 1 字节，用 `u8` 接，不能用 Rust `bool`。

## 生命周期

- `ADLXInitialize` 只做一次，`IADLXSystem` / `IADLXDisplayServices` 进程内常驻（`static RUNTIME`）
- 版本协商：先按 SDK 1.5.0.124；驱动更老报 `BAD_VER` 时退回驱动自报的版本
- 显示器列表每次调用重新枚举，热插拔安全
- 应用退出时 `amd::shutdown()`（挂在 `RunEvent::Exit`）Release 常驻对象并 `ADLXTerminate`，避免 `ORPHAN_OBJECTS`
- 永不 `FreeLibrary`：ADLX 内部有工作线程

## 显示器定位

Windows 侧 `\\.\DISPLAYn` → `icc::get_display_monitors` 取 `pnp_id` →
`icc::get_monitor_name_from_edid` 取 EDID 型号名（如 `VG27AQML1A`）→
与 ADLX `IADLXDisplay::Name` 逐个比对（两边读的是同一个 EDID 0xFC 描述符）。

- 型号名相同 → 命中
- Windows 侧取不到型号名且 ADLX 只有一台 → 用那一台兜底
- 型号名对不上 → **报错，不回退**（Intel 核显 + AMD 独显混合输出时，那台屏本来就不归 AMD 管）
- 已知局限：两台同型号显示器会命中第一台

## 标度

UI 固定 0..100；驱动值域由 `GetSaturationRange` 给出（实测 [0,200] step=1），
写入前经 `nvidia::ui_to_driver_level` 换算并对齐 step。默认值取值域中点
（ADLX 没有"默认值"查询接口，饱和度以中点为中性）。

## 对外函数

```rust
/// (min, max, default, current)，与 nvapi_get_dvc_info 同形
pub(crate) fn get_saturation_info(device_id: &str) -> Result<(i32, i32, i32, i32), String>

/// 按 UI 标度 0..100 写入
pub(crate) fn set_saturation_ui(device_id: &str, ui_level: i32) -> Result<(), String>

/// 应用退出时调用
pub fn shutdown()
```

错误串前缀固定为 `ADLX: ...` / `ADLX <方法> failed: <code> (ADLX_<NAME>)`，
`nvidia::humanize_dvc_error` 据此翻译成用户可读文案。

---

**更新于**：2026-09-13 — 新建。基于 RX 7900 XT + Adrenalin（ADLX 1.5.0.124）真机诊断数据实现
