//! 独立的「写入能力」探针 v2。
//!
//! 它存在的唯一理由：`icc.rs` 写完 `SetDeviceGammaRamp` 只把返回值丢进 `eprintln!`，
//! 而 GUI release 版没有控制台 —— 于是「写入被拒」在用户那边表现为"点了没反应"，
//! 我们这边什么都收不到。
//!
//! 这个工具**调用和 app 完全相同的代码**（`filter-manage-platform` 就是 app 依赖的那个
//! crate，`parse_vcgt` / `calculate_lut` / `compose_ramp` 都是同一份），所以它测出来的
//! 结果就是 app 会得到的结果。唯一额外做的一件事是**写完回读比对** ——
//! MSDN 明确写了 `SetDeviceGammaRamp` 会对"会让屏幕不可读"的 ramp **静默失败**
//! （返回 TRUE 但根本没设置），只看返回值是抓不到这种失败的。
//!
//! v2 新增（2026-10-01，用户实机出现「API 成功 + 回读一致 + 屏幕零变化」后）：
//! 回读一致只证明驱动收下了曲线，不证明画面用了它。所以补四块信息：
//! ① 环境段（OS build / 显卡驱动版本 / HDR 高级颜色 / 可疑 LUT 抢占进程，可选手动结束）；
//! ② 持久性采样：写入独特曲线后 20 秒内每 500ms 回读，区分「没人动它」和「被程序抢回去」；
//! ③ 肉眼确认：6 秒内每 300ms 重写一条重度变暗曲线，把用户的肉眼观察记进报告；
//! ④ 综合结论：把「驱动收下但画面不用」这类管线断路明确指出来。

use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use filter_manage_platform as plat;

const SETTLE: Duration = Duration::from_millis(900);
/// 持久性采样：总时长 / 间隔
const PERSIST_SECS: u64 = 20;
const PERSIST_STEP_MS: u64 = 500;
/// 肉眼确认：保持时长 / 重写间隔
const HOLD_SECS: u64 = 6;
const HOLD_STEP_MS: u64 = 300;

/// 可能参与色彩 / LUT 管理的进程名片段（小写、子串匹配）。
/// 刻意不含 NVDisplay.Container——它在任何 N 卡机器上常驻，列它等于每台机器都喊（信号归零）。
const CLAIMER_TERMS: &[&str] = &[
    // MSI 全家桶（True Color 会改写 LUT；DCv2 是 Dragon Center 的主进程名——实机报告核出来的）
    "msi",
    "dragon center",
    "creator center",
    "truecolor",
    "true color",
    "dcv2",
    // 火绒（本案实锤：运行期间拦截 SetDeviceGammaRamp；
    // "hips" 同时命中 HipsDaemon 和 HipsTray）
    "hips",
    "usysdiag",
    "wsctrl",
    // 第三方校准 / 屏幕工具
    "displaycal",
    "flux",
    "twinkletray",
    "monitorian",
    "onscreen control",
    "controlmymonitor",
];

struct TestCase {
    label: String,
    note: String,
    ramp: Vec<u16>,
}

/// app 的预设（只取探针需要的字段；字段名对不上会落到默认值 —— 所以报告里会把读到的值打出来）
#[derive(serde::Deserialize, Default)]
struct Preset {
    #[serde(default)]
    name: String,
    #[serde(default)]
    brightness: i32,
    #[serde(default)]
    contrast: i32,
    #[serde(default)]
    gamma: f64,
    #[serde(default)]
    rgb_r: i32,
    #[serde(default)]
    rgb_g: i32,
    #[serde(default)]
    rgb_b: i32,
}

#[derive(serde::Deserialize, Default)]
struct AppStore {
    #[serde(default)]
    presets: Vec<Preset>,
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

fn app_json_path() -> PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    PathBuf::from(appdata).join("filter-manage").join("app.json")
}

fn timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // 只需要一个稳定的文件名后缀，不做日历换算
    format!("{secs}")
}

/// 带提示读一行输入；输入流关闭（被重定向/无终端）时返回 None，绝不卡死
fn ask(prompt: &str) -> Option<String> {
    print!("{prompt}");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).unwrap_or(0) == 0 {
        return None;
    }
    Some(line.trim().to_string())
}

// ─── 注册表读取（OS / 显卡驱动）──────────────────────────────────────────────

fn reg_sz(path: &str, name: &str) -> Option<String> {
    use winreg::enums::*;
    let key = winreg::RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(path, KEY_READ)
        .ok()?;
    key.get_value(name).ok()
}

fn reg_u32(path: &str, name: &str) -> Option<u32> {
    use winreg::enums::*;
    let key = winreg::RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(path, KEY_READ)
        .ok()?;
    key.get_value(name).ok()
}

/// OS 版本串，如 "Windows 11 24H2 (build 26100.2894)"
fn os_info() -> String {
    const CV: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let product = reg_sz(CV, "ProductName").unwrap_or_else(|| "?".into());
    let display = reg_sz(CV, "DisplayVersion").unwrap_or_default();
    let build = reg_sz(CV, "CurrentBuildNumber").unwrap_or_default();
    let ubr = reg_u32(CV, "UBR").unwrap_or(0);
    if display.is_empty() {
        format!("{product} (build {build}.{ubr})")
    } else {
        format!("{product} {display} (build {build}.{ubr})")
    }
}

/// 显卡驱动列表（显示类 GUID 下 0000..0009），NVIDIA 的版本号顺手翻译成 561.03 这种格式
fn gpu_drivers(out: &mut String) {
    const CLASS: &str = r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";
    let mut found = 0;
    for idx in 0..10 {
        let sub = format!("{CLASS}\\{idx:04}");
        let Some(desc) = reg_sz(&sub, "DriverDesc") else {
            continue;
        };
        found += 1;
        let raw_version = reg_sz(&sub, "DriverVersion").unwrap_or_default();
        let date = reg_sz(&sub, "DriverDate").unwrap_or_default();
        let translated = if desc.to_lowercase().contains("nvidia") {
            // "32.0.15.6103" → 去点拼 "320156103" 取后 5 位 "56103" → "561.03"
            let digits: String = raw_version.split('.').collect();
            translate_nvidia_version(&digits)
                .map(|v| format!("{raw_version}（≈{v}）"))
                .unwrap_or_else(|| raw_version.clone())
        } else {
            raw_version.clone()
        };
        out.push_str(&format!("    {desc}  版本={translated}  日期={date}\n"));
    }
    if found == 0 {
        out.push_str("    （注册表里读不到任何显卡驱动记录）\n");
    }
}

fn translate_nvidia_version(digits: &str) -> Option<String> {
    let last5: String = digits
        .chars()
        .rev()
        .take(5)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if last5.len() < 5 || !last5.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let value: u32 = last5.parse().ok()?;
    if value == 0 {
        return None;
    }
    let (hi, lo) = last5.split_at(3);
    Some(format!("{}.{}", hi, lo))
}

// ─── HDR / 高级颜色状态（DisplayConfig）─────────────────────────────────────

#[cfg(windows)]
mod advanced_color {
    // DisplayConfig 三个函数直接走 user32 原始 FFI（和共享 crate 的动态取函数风格一致，
    // 避开 windows crate 各版本函数签名的差异）；结构体和常量仍然用 windows crate 的
    // 权威定义 —— 手抄 DisplayConfig 的布局必错。
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

    pub struct Entry {
        pub gdi_device: String,
        pub ok: bool,
        pub supported: bool,
        pub enabled: bool,
        pub force_disabled: bool,
        pub output_technology: i32,
    }

    fn output_tech_name(tech: i32) -> String {
        let name = match tech {
            1 => "VGA(HD15)",
            5 => "DVI",
            6 => "HDMI",
            7 => "LVDS(内屏)",
            10 => "DisplayPort(外接)",
            11 => "DisplayPort(内屏)",
            15 => "Miracast",
            16 => "间接有线",
            17 => "间接虚拟",
            _ => "其他/未知",
        };
        format!("{}({tech})", name)
    }

    /// 枚举所有活动路径，逐条给出 GDI 设备名 + HDR（高级颜色）状态。
    /// 单条查询失败时该条 ok=false，报告里如实写「查不到」，绝不猜。
    pub fn states() -> Result<Vec<Entry>, String> {
        unsafe {
            let (mut num_paths, mut num_modes) = (0u32, 0u32);
            let rc =
                GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS.0 as i32, &mut num_paths, &mut num_modes);
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

            let mut entries = Vec::new();
            for path in &paths[..num_paths as usize] {
                // 源名：拿 GDI 设备名（\\.\DISPLAYn），和 app 枚举出的显示器对上号
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

                // HDR 高级颜色状态挂在 target 上
                let mut aci: DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO = std::mem::zeroed();
                aci.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO;
                aci.header.size = std::mem::size_of::<DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO>() as u32;
                aci.header.adapterId = path.targetInfo.adapterId;
                aci.header.id = path.targetInfo.id;
                let (ok, supported, enabled, force_disabled) =
                    if DisplayConfigGetDeviceInfo(&mut aci.header) == 0 {
                        // windows 0.58 把 value 放在匿名 union 里；低三位分别是
                        // supported / enabled / forceDisabled
                        let bits = aci.Anonymous.value;
                        (true, bits & 0x1 != 0, bits & 0x2 != 0, bits & 0x4 != 0)
                    } else {
                        (false, false, false, false)
                    };

                entries.push(Entry {
                    gdi_device: gdi,
                    ok,
                    supported,
                    enabled,
                    force_disabled,
                    output_technology: path.targetInfo.outputTechnology.0,
                });
            }
            Ok(entries)
        }
    }

    pub fn tech(entry: &Entry) -> String {
        output_tech_name(entry.output_technology)
    }
}

// ─── 可疑进程检测 / 结束 ─────────────────────────────────────────────────────

/// 用共享 crate 的进程枚举（和 app 同一份实现），按 CLAIMER_TERMS 子串匹配
fn find_claimers() -> Result<Vec<plat::ProcessInfo>, String> {
    let terms: Vec<String> = CLAIMER_TERMS.iter().map(|s| s.to_string()).collect();
    plat::list_processes(&terms)
}

fn kill_claimers(list: &[plat::ProcessInfo], out: &mut String) {
    for p in list {
        let result = std::process::Command::new("taskkill")
            .args(["/F", "/PID", &p.pid.to_string()])
            .output();
        match result {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&o.stderr).trim().to_string();
                out.push_str(&format!(
                    "    结束 {} (pid {}) → {}\n",
                    p.name,
                    p.pid,
                    if stdout.is_empty() { stderr } else { stdout }
                ));
            }
            Err(error) => out.push_str(&format!("    结束 {} 失败：{error}\n", p.name)),
        }
    }
    // 结束后复扫：还在的说明是服务型进程（会被 SCM 拉回来），报告里点名
    match find_claimers() {
        Ok(remaining) if !remaining.is_empty() => {
            out.push_str("    结束后仍存活（服务型，会被系统自动拉起，需要停服务/关自启）：\n");
            for p in &remaining {
                out.push_str(&format!("      {} (pid {})\n", p.name, p.pid));
            }
        }
        Ok(_) => out.push_str("    结束后复扫：全部已退出。\n"),
        Err(error) => out.push_str(&format!("    结束后复扫失败：{error}\n")),
    }
}

// ─── 探针主体 ────────────────────────────────────────────────────────────────

/// 先把 app 的预设读出来 —— 这些才是用户真正会应用的曲线
fn load_presets(out: &mut String) -> Vec<Preset> {
    let path = app_json_path();
    out.push_str(&format!("app.json = {}\n", path.display()));
    let Ok(text) = std::fs::read_to_string(&path) else {
        out.push_str("  （读不到 —— 跳过预设曲线，只测内置曲线）\n");
        return Vec::new();
    };
    match serde_json::from_str::<AppStore>(&text) {
        Ok(store) => {
            out.push_str(&format!("  读到 {} 个预设\n", store.presets.len()));
            store.presets
        }
        Err(error) => {
            out.push_str(&format!("  解析失败（{error}）—— 跳过预设曲线\n"));
            Vec::new()
        }
    }
}

fn build_cases(presets: &[Preset], icc: Option<&[u8]>, icc_base: Option<&[[u16; 256]; 3]>) -> Vec<TestCase> {
    let mut cases = Vec::new();

    // ① 恒等：如果连这条都写不进去，问题一定在驱动/设备层，与曲线内容无关
    cases.push(TestCase {
        label: "identity（恒等）".into(),
        note: "基准：连这条都失败 = 驱动/设备层写不进，与曲线内容无关".into(),
        ramp: plat::identity_ramp(),
    });

    // ② app 全中性
    cases.push(TestCase {
        label: "app 全中性（亮度0 对比度0 伽马1.0 RGB0）".into(),
        note: "app 合成公式在中性档位下应接近恒等".into(),
        ramp: plat::app_ramp(0, 0, 1.0, [0, 0, 0], None).to_vec(),
    });

    // ③ 用户真实预设（有的话）
    for preset in presets.iter().take(6) {
        let name = if preset.name.is_empty() {
            "(未命名)".to_string()
        } else {
            preset.name.clone()
        };
        let ramp = plat::app_ramp(
            preset.brightness,
            preset.contrast,
            preset.gamma,
            [preset.rgb_r, preset.rgb_g, preset.rgb_b],
            icc_base,
        );
        cases.push(TestCase {
            label: format!("app 预设「{name}」"),
            note: format!(
                "读到的档位：亮度={} 对比度={} 伽马={} RGB={}/{}/{}",
                preset.brightness, preset.contrast, preset.gamma,
                preset.rgb_r, preset.rgb_g, preset.rgb_b
            ),
            ramp: ramp.to_vec(),
        });
    }

    // ④ 拉满：高位大段截断在 65535 —— 最可能被驱动拒的那条
    cases.push(TestCase {
        label: "app 拉满（亮度+125 对比度+82 伽马0.4 RGB+100）".into(),
        note: "高位大段截断在 65535，是最可能被驱动拒绝的形状".into(),
        ramp: plat::app_ramp(125, 82, 0.4, [100, 100, 100], None).to_vec(),
    });

    // ⑤ 伽马 2.4：app 的伽马语义是反的（gamma>1 = 变亮，见交接文档 §6.5），
    //    中间调 128 → ≈191，肉眼必见。用肉眼确认写入有没有真的落到屏上。
    cases.push(TestCase {
        label: "app 伽马 2.4（画面应明显变亮）".into(),
        note: "请看着屏幕：这条如果 API 成功、回读也一致，画面就应该明显变亮（app 伽马>1=变亮）".into(),
        ramp: plat::app_ramp(0, 0, 2.4, [0, 0, 0], None).to_vec(),
    });

    // ⑥ 用户拖进来的那个 ICC 的 vcgt
    if let Some(data) = icc {
        let info = plat::vcgt::describe_vcgt(data);
        match plat::vcgt::parse_vcgt(data) {
            Some(ramp768) => {
                let flat: Vec<u16> = (0..768).map(|i| ramp768[i / 256][i % 256]).collect();
                cases.push(TestCase {
                    label: "你拖进来的 ICC 的 vcgt".into(),
                    note: format!(
                        "vcgt={} / app 会应用={} / {}",
                        info.kind, info.app_would_apply, info.reason
                    ),
                    ramp: flat,
                });
            }
            None => {
                cases.push(TestCase {
                    label: "你拖进来的 ICC（app 判定不会写 LUT）".into(),
                    note: format!(
                        "vcgt={} → 原因：{}。**这就是「点了没反应」的解释：app 根本没去动 LUT**",
                        info.kind, info.reason
                    ),
                    ramp: plat::identity_ramp(),
                });
            }
        }
    }

    cases
}

/// 拿到那个「点了没反应」的 ICC 的路径。
///
/// 为什么这么啰嗦：这件事连续三份报告都缺，因为没有这个文件的报告基本没用
/// （不知道他点的是哪个 profile，就判不了"没有 vcgt 所以本来就不该有变化"）。
/// 所以这里交互追问，并且把跳过做成需要显式输入 s。
fn resolve_icc_path(args: &[String]) -> Option<String> {
    if let Some(raw) = args.iter().find(|a| !a.starts_with("--")) {
        let candidate = raw.trim().trim_matches('"').to_string();
        if PathBuf::from(&candidate).is_file() {
            return Some(candidate);
        }
        println!("[!] 命令行给的路径找不到文件：{candidate}");
    }

    println!();
    println!("================================================================");
    println!("  请把那个「点了没反应」的 ICC 文件拖到本窗口里，然后按回车。");
    println!("  （Windows 控制台支持把文件直接拖进来，路径会自动填上）");
    println!();
    println!("  也可以直接把 .icc 文件拖到 run.bat 上重新运行。");
    println!("  确实拿不到就输入 s 跳过 —— 但缺这个文件的报告基本定位不了问题。");
    println!("================================================================");

    loop {
        let Some(text) = ask("> ") else {
            println!("（没有读到输入，跳过）");
            return None;
        };
        let text = text.trim_matches('"').trim().to_string();
        if text.eq_ignore_ascii_case("s") {
            println!("[!] 已跳过 —— 报告里不会有那个 ICC 的判定，定位能力会明显下降。");
            return None;
        }
        if text.is_empty() {
            continue;
        }
        if PathBuf::from(&text).is_file() {
            println!("[✓] 已读到：{text}");
            return Some(text);
        }
        println!("[×] 找不到这个文件：{text}");
        println!("    重新拖一次，或输入 s 跳过。");
    }
}

/// 一条明显区别于恒等（×1.0）和常见校准曲线（×0.5~×0.9）的**提亮**曲线：
/// 幂 0.55，中间调 128 → ≈174（高字节）。
/// 刻意选亮不选暗：Windows 对「会让屏幕不可读」的过暗 ramp 会启发式拒绝
/// （实机验证 ×0.30 深暗被驱动拒掉），亮曲线从不触发。
fn distinct_ramp() -> Vec<u16> {
    (0..3)
        .flat_map(|_| {
            (0..=255u32).map(|v| {
                ((v as f64 / 255.0).powf(0.55) * 65535.0).round() as u16
            })
        })
        .collect()
}

/// 持久性采样的判定
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PersistVerdict {
    /// 首次写入就没落住，采样没意义
    Skipped,
    /// 20 秒内始终是我们写入的曲线
    Stable,
    /// 期间被拉回测试前曲线或变成第三种形状
    Grabbed,
}

/// 持久性采样：写入独特曲线后 PERSIST_SECS 秒内每 PERSIST_STEP_MS 回读一次，
/// 判定 LUT 是「一直保持」「被拉回测试前曲线」还是「被改成第三种形状」。
fn persistence_test(
    display_id: &str,
    original: &[u16],
    out: &mut String,
    say: &dyn Fn(&str),
) -> PersistVerdict {
    out.push_str(&format!("\n  【{display_id}】\n"));
    out.push_str("    测试前曲线 R 采样 = ");
    out.push_str(&sample(original, 0));
    out.push_str("\n");

    let distinct = distinct_ramp();
    let land = plat::write_and_verify(display_id, &distinct);
    out.push_str(&format!(
        "    写入「提亮（幂0.55）」曲线：API={} 回读一致={} 高字节最大差={}\n",
        if land.api_ok { "成功" } else { "失败" },
        land.readback_ok,
        land.max_high_byte_diff
    ));
    if !land.applied {
        out.push_str(&format!("    判定：{}（后续采样没有意义，跳过）\n", land.verdict));
        return PersistVerdict::Skipped;
    }

    let rounds = (PERSIST_SECS * 1000 / PERSIST_STEP_MS) as usize;
    let mut kept = 0usize;
    let mut reverted = 0usize;
    let mut other = 0usize;
    let mut first_deviation: Option<f64> = None;
    let mut other_shape: Option<Vec<u16>> = None;

    say(&format!(
        "    正在追踪 LUT {PERSIST_SECS} 秒（每 {PERSIST_STEP_MS}ms 采样一次，期间尽量别动其它程序）……"
    ));
    for round in 0..=rounds {
        let (values, error) = plat::read_gamma_ramp(display_id);
        let at = round as f64 * PERSIST_STEP_MS as f64 / 1000.0;
        match values {
            Some(values) => {
                let matches_distinct = plat::max_high_byte_diff(&values, &distinct) == 0;
                let matches_original = plat::max_high_byte_diff(&values, original) == 0;
                if matches_distinct {
                    kept += 1;
                } else {
                    if first_deviation.is_none() {
                        first_deviation = Some(at);
                    }
                    if matches_original {
                        reverted += 1;
                    } else {
                        other += 1;
                        if other_shape.is_none() {
                            other_shape = Some(values);
                        }
                    }
                }
            }
            None => {
                out.push_str(&format!(
                    "    第 {at:.1}s 回读失败：{}\n",
                    error.unwrap_or_default()
                ));
            }
        }
        if round != rounds {
            std::thread::sleep(Duration::from_millis(PERSIST_STEP_MS));
            if round % 10 == 0 {
                print!(".");
                let _ = std::io::stdout().flush();
            }
        }
    }
    println!();

    let deviation_note = match first_deviation {
        Some(at) => format!("首次偏离在第 {at:.1}s"),
        None => "全程无偏离".into(),
    };
    out.push_str(&format!(
        "    采样结果：保持写入值 {kept} 次；变回测试前曲线 {reverted} 次；变成第三种形状 {other} 次（{deviation_note}）\n"
    ));
    if let Some(shape) = &other_shape {
        out.push_str("    第三种形状 R 采样 = ");
        out.push_str(&sample(shape, 0));
        out.push_str("  ← 这是抢占者自己的曲线\n");
    }

    if reverted > 0 || other > 0 {
        out.push_str("    判定：** LUT 被外部程序改了回去 —— 抢占者实锤。对照第 0 节的进程列表找它。**\n");
        PersistVerdict::Grabbed
    } else {
        out.push_str(&format!(
            "    判定：写入的曲线 {PERSIST_SECS} 秒内没人动 → 不是被抢占；如果屏幕仍然没变化，断的是「LUT → 画面」这一段。\n"
        ));
        PersistVerdict::Stable
    }
}

/// 肉眼确认的观察结果：Some("1"/"2"/"3")；None = 没读到输入
fn visual_hold_test(
    display_id: &str,
    original: &[u16],
    out: &mut String,
    say: &dyn Fn(&str),
) -> Option<String> {
    out.push_str(&format!("\n  【{display_id}】\n"));
    say(&format!(
        "  ★ 接下来 {HOLD_SECS} 秒屏幕应该明显变亮（每 {HOLD_STEP_MS}ms 重写一次，防抢占）。请死死盯着屏幕！"
    ));
    let ready = ask("  按回车开始：").unwrap_or_default();
    out.push_str(&format!(
        "    用户确认开始：{}\n",
        if ready.is_empty() { "（未读到输入，直接开始）" } else { "回车" }
    ));

    // app 伽马 2.4 = 中间调 ×1.5 提亮（语义见交接文档 §6.5）；
    // 亮曲线不会触发 Windows「防不可读」启发式，必能落进驱动
    let bright = plat::app_ramp(0, 0, 2.4, [0, 0, 0], None).to_vec();
    let rounds = (HOLD_SECS * 1000 / HOLD_STEP_MS) as usize;
    let mut write_fail = 0usize;
    for round in 0..=rounds {
        if plat::write_gamma_ramp(display_id, &bright).is_err() {
            write_fail += 1;
        }
        if round != rounds {
            std::thread::sleep(Duration::from_millis(HOLD_STEP_MS));
        }
    }
    // 立刻恢复，别让用户多亮一秒
    let _ = plat::write_gamma_ramp(display_id, original);
    if write_fail > 0 {
        out.push_str(&format!("    保持期间写入失败 {write_fail} 次\n"));
    }

    let answer = ask("  这几秒你的屏幕：输入 1=明显变亮且稳定  2=闪烁/来回变  3=完全没变 > ");
    let verdict = match answer.as_deref() {
        Some("1") => "写入真实生效 —— 与「没反应」矛盾，可能此前没盯着屏幕，或问题只在 app 侧。请在 app 里重点一次 ICC 对照。",
        Some("2") => "** 有另一个程序在同一条 LUT 上和我们抢（每 300ms 都重写了还是抢得回来）——对照第 0 节进程列表结束它再测。**",
        Some("3") => "** 每 300ms 重写一次都无效 → 不是抢占，是这条 LUT 管线根本不参与画面输出。**\n      ** 候选：HDR 高级颜色开着（看第 0 节）、Windows 24H2+驱动组合的已知问题、驱动装坏。**",
        None | Some("") => "缺少肉眼证据（没读到输入），请重跑时盯紧第 6 节那段。",
        Some(other) => return {
            out.push_str(&format!("    用户观察：无法识别的输入「{other}」，请重跑并输入 1/2/3。\n"));
            None
        },
    };
    let shown = match &answer {
        Some(v) => v.clone(),
        None => "（没读到输入）".to_string(),
    };
    out.push_str(&format!("    用户观察：{shown}\n    判定：{verdict}\n"));
    answer
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    println!("================================================================");
    println!("  屏幕 LUT 写入能力探针 v2（gamma-debug）");
    println!("  本工具调用与 Filter-Manage 完全相同的代码");
    println!("  v2：环境/HDR/抢占进程 + 20 秒持久性采样 + 肉眼确认");
    println!("================================================================");

    let mut report = String::new();
    let say = |line: &str| {
        println!("{line}");
        let _ = std::io::stdout().flush();
    };

    report.push_str("================================================================\n");
    report.push_str("  屏幕 LUT 写入能力探针 v2（gamma-debug）\n");
    report.push_str("  本工具调用与 Filter-Manage 完全相同的代码\n");
    report.push_str("================================================================\n\n");

    // ── 0. 环境 ──
    report.push_str("---- 0. 环境（OS / 显卡驱动 / HDR / 可疑进程）----\n");
    report.push_str(&format!("  OS = {}\n", os_info()));
    report.push_str("  显卡驱动：\n");
    gpu_drivers(&mut report);

    let hdr_entries = match advanced_color::states() {
        Ok(entries) => {
            for e in &entries {
                if e.ok {
                    report.push_str(&format!(
                        "  HDR（高级颜色）[{}]：支持={} 已启用={} 被强制关闭={} 输出={}\n",
                        e.gdi_device,
                        e.supported,
                        e.enabled,
                        e.force_disabled,
                        advanced_color::tech(e)
                    ));
                } else {
                    report.push_str(&format!(
                        "  HDR（高级颜色）[{}]：单条查询失败（不影响其余测试）\n",
                        e.gdi_device
                    ));
                }
            }
            Some(entries)
        }
        Err(error) => {
            report.push_str(&format!("  HDR（高级颜色）：整体查询失败（{error}）\n"));
            None
        }
    };
    let hdr_on = hdr_entries
        .as_ref()
        .map(|list| list.iter().any(|e| e.ok && e.enabled))
        .unwrap_or(false);

    report.push_str("  可能参与色彩管理的进程（仅嫌疑，不一定是凶手）：\n");
    let claimers = match find_claimers() {
        Ok(list) if list.is_empty() => {
            report.push_str("    （无匹配进程）\n");
            Vec::new()
        }
        Ok(list) => {
            for p in &list {
                report.push_str(&format!("    {} (pid {})\n", p.name, p.pid));
            }
            list
        }
        Err(error) => {
            report.push_str(&format!("    （进程枚举失败：{error}）\n"));
            Vec::new()
        }
    };

    // 控制台同步关键环境 + HDR 大声提示
    say("");
    say(&format!("  OS = {}", os_info()));
    if hdr_on {
        say("  **************************************************");
        say("  ** HDR（高级颜色）已开启 —— 仅此一条就足以让所有 gamma 写入**");
        say("  ** 对画面完全无效（API 成功、回读一致也照样无效）。      **");
        say("  ** 先去 设置 → 系统 → 屏幕 → HDR 关掉，再重新跑本工具。  **");
        say("  **************************************************");
    }
    if !claimers.is_empty() {
        say("  检测到可能参与色彩管理的进程：");
        for p in &claimers {
            say(&format!("    {} (pid {})", p.name, p.pid));
        }
        let choice = ask("  输入 k 并回车 = 结束这些进程后继续（服务型的会被拉回来）；直接回车 = 不动它们 > ");
        match choice {
            Some(c) if c.eq_ignore_ascii_case("k") => {
                report.push_str("  用户选择：结束上述进程。taskkill 结果：\n");
                kill_claimers(&claimers, &mut report);
            }
            _ => {
                report.push_str("  用户选择：保留上述进程不动。\n");
            }
        }
    }
    report.push_str("\n");

    // ── ICC 文件 ──
    let icc_path = resolve_icc_path(&args);

    // ── 显示器 ──
    let displays = match plat::enumerate_displays() {
        Ok(list) => list,
        Err(error) => {
            say(&format!("枚举显示器失败：{error}"));
            report.push_str(&format!("\n枚举显示器失败：{error}\n"));
            finish(report);
            return;
        }
    };
    if displays.is_empty() {
        say("没有检测到活动的显示器。");
        report.push_str("\n没有检测到活动的显示器。\n");
        finish(report);
        return;
    }

    report.push_str("---- 1. 显示器 ----\n");
    for d in &displays {
        let mode = d
            .mode
            .as_ref()
            .map(|m| {
                format!(
                    "{}x{}@{}Hz {}bpp",
                    m.width, m.height, m.refresh_hz, m.bits_per_pixel
                )
            })
            .unwrap_or_else(|| "(取不到模式)".into());
        report.push_str(&format!(
            "  {}  主屏={}  适配器={}  模式={}\n      PnP={}\n",
            d.id, d.primary, d.adapter, mode, d.pnp_id
        ));
    }
    report.push_str("\n");

    // ── 2. ICC ──
    let mut icc_bytes: Option<Vec<u8>> = None;
    if let Some(path) = &icc_path {
        report.push_str("---- 2. 你拖进来的 ICC ----\n");
        match std::fs::read(path) {
            Ok(bytes) => {
                let info = plat::vcgt::describe_vcgt(&bytes);
                report.push_str(&format!("  文件 = {path}\n  大小 = {} 字节\n", bytes.len()));
                report.push_str(&format!(
                    "  vcgt = {}（声明长度 {} 字节）\n",
                    info.kind, info.declared_size
                ));
                if info.kind == "formula" {
                    for (i, ch) in ["R", "G", "B"].iter().enumerate() {
                        report.push_str(&format!(
                            "    {ch}: gamma={:.4} min={:.4} max={:.4}\n",
                            info.gamma[i], info.min[i], info.max[i]
                        ));
                    }
                } else if info.kind == "table" {
                    report.push_str(&format!(
                        "    channels={} entries={} entrySize={}\n",
                        info.channels, info.entry_count, info.entry_size
                    ));
                }
                report.push_str(&format!("  app 判定：{}\n\n", info.reason));
                icc_bytes = Some(bytes);
            }
            Err(error) => {
                report.push_str(&format!("  读不到 {path}：{error}\n\n"));
            }
        }
    }

    // ── 3. 预设 ──
    // 合成基准：app 是把滑块曲线叠在「当前 ICC 的 vcgt」上的。
    // 这里用拖进来的那个 ICC 当基准，所以预设曲线的形状贴近但不等于 app 当前状态。
    let icc_base = icc_bytes.as_deref().and_then(plat::vcgt::parse_vcgt);

    report.push_str("---- 3. app 预设（用来复现你真实的曲线）----\n");
    let presets = load_presets(&mut report);
    report.push_str(match icc_base {
        Some(_) => "  合成基准 = 你拖进来的那个 ICC 的 vcgt\n",
        None => "  合成基准 = 无 ICC 基础曲线（纯滑块合成）\n",
    });
    report.push_str("\n");

    // ── 写入测试 ──
    let cases = build_cases(&presets, icc_bytes.as_deref(), icc_base.as_ref());

    say("");
    say("================================================================");
    say("  即将开始写入测试。屏幕会在每条曲线之间变化，最后自动恢复原样。");
    say("  期间请不要关掉本窗口。");
    say("  按回车开始……");
    let _ = std::io::stdin().read_line(&mut String::new());

    report.push_str("---- 4. 写入扫描（每条都做：写 → 取错误码 → 回读 → 比对）----\n");

    // 各显示器汇总，供第 7 节综合结论使用
    let mut all_applied = true;
    let mut any_grabbed = false;
    let mut visual_answer: Option<String> = None;

    for display in &displays {
        report.push_str(&format!("\n  【{}】\n", display.id));

        let original = match plat::read_gamma_ramp(&display.id) {
            (Some(values), _) => values,
            (None, error) => {
                report.push_str(&format!(
                    "    ** 回读失败，无法测试写入：{} **\n",
                    error.unwrap_or_else(|| "未知原因".into())
                ));
                report.push_str(
                    "    （回读都不行 = 这条输出很可能根本没有可加载的 LUT，属于驱动/设备层问题）\n",
                );
                continue;
            }
        };
        report.push_str("    原 ramp R 采样 = ");
        report.push_str(&sample(&original, 0));
        report.push_str("\n");

        let mut api_fail = 0;
        let mut silent = 0;
        let mut applied = 0;

        for case in &cases {
            say(&format!("  [{}] 正在测：{}", display.id, case.label));
            let outcome = plat::write_and_verify(&display.id, &case.ramp);
            std::thread::sleep(SETTLE);

            if !outcome.api_ok {
                api_fail += 1;
            } else if !outcome.applied {
                silent += 1;
            } else {
                applied += 1;
            }

            report.push_str(&format!("    · {}\n", case.label));
            report.push_str(&format!("        {}\n", case.note));
            report.push_str(&format!(
                "        API={} winerr={} 回读一致={} 高字节最大差={}\n",
                if outcome.api_ok { "成功" } else { "失败" },
                outcome.winerr,
                outcome.readback_ok,
                outcome.max_high_byte_diff
            ));
            if let Some(error) = &outcome.api_error {
                report.push_str(&format!("        api_error = {error}\n"));
            }
            if let Some(error) = &outcome.readback_error {
                report.push_str(&format!("        readback_error = {error}\n"));
            }
            report.push_str(&format!("        判定：{}\n", outcome.verdict));
        }

        if applied != cases.len() {
            all_applied = false;
        }

        // ── 持久性采样 ──
        report.push_str("\n---- 5. 持久性采样（写入后追踪 20 秒，抓「谁改了它」）----");
        let persist = persistence_test(&display.id, &original, &mut report, &say);
        if persist == PersistVerdict::Grabbed {
            any_grabbed = true;
        }

        // ── 肉眼确认 ──
        report.push_str("\n---- 6. 肉眼确认（6 秒强制提亮，每 300ms 重写）----");
        visual_answer = visual_hold_test(&display.id, &original, &mut report, &say);

        // ── 恢复 ──
        let restore = plat::write_and_verify(&display.id, &original);
        report.push_str(&format!(
            "\n    恢复原 ramp：API={} 回读一致={} → {}\n",
            if restore.api_ok { "成功" } else { "失败" },
            restore.readback_ok,
            restore.verdict
        ));

        // ── 该显示器的扫描结论 ──
        report.push_str("\n    【第 4 节扫描结论】\n");
        if api_fail == cases.len() {
            report.push_str("      ** 全部曲线都写入失败 → 问题在驱动/设备层，与曲线内容无关。**\n");
            report.push_str("      ** 优先查显卡驱动是否装好（设备管理器有没有通用设备/感叹号），再干净重装驱动。**\n");
        } else if api_fail > 0 {
            report.push_str(&format!(
                "      ** {api_fail}/{} 条写入失败 → 与曲线内容相关：某些形状会被驱动拒绝。**\n",
                cases.len()
            ));
        } else if silent > 0 {
            report.push_str(&format!(
                "      ** API 全部返回成功，但有 {silent} 条回读不一致 → 存在「静默拒绝」。**\n"
            ));
            report.push_str("      ** 这正是 app 察觉不到的那类失败（它只看返回值），也是「点了没反应」的机制之一。**\n");
        } else {
            report.push_str(&format!(
                "      API 全部成功且回读一致（{applied}/{}）→ 驱动层 LUT 写入是通的。\n",
                cases.len()
            ));
            report.push_str("      「没反应」的原因在第 5/6/7 节：要么被抢占，要么这条 LUT 管线根本不参与画面输出。\n");
        }
    }

    // ── 7. 综合结论 ──
    report.push_str("\n---- 7. 综合结论 ----\n");
    if hdr_on {
        report.push_str("  ** HDR（高级颜色）已开启：仅此一条即可让所有 gamma LUT 写入对画面无效。**\n");
        report.push_str("  ** 先关闭 HDR（设置 → 系统 → 屏幕 → HDR），再重跑本工具。**\n");
    } else if any_grabbed {
        report.push_str("  ** LUT 在 20 秒采样内被改了回去 → 有程序在抢 LUT，它就是「没反应」的凶手。**\n");
        report.push_str("  ** 对照第 0 节进程列表结束/停用它的自启（MSI True Color 类软件的常见行为）。**\n");
    } else if visual_answer.as_deref() == Some("3") && all_applied {
        report.push_str("  ** 驱动收下了 LUT（回读一致）且 20 秒没人动它，但画面完全不用它 → 「LUT → 画面」管线断路。**\n");
        report.push_str("  ** 处置顺序：① 确认 HDR 已关（第 0 节）；② 用 DDU 干净重装显卡驱动；**\n");
        report.push_str("  ** ③ 若 OS 是 Windows 11 24H2，留意微软/NVIDIA 的已知 gamma 问题，换驱动版本试。**\n");
    } else if visual_answer.as_deref() == Some("2") {
        report.push_str("  ** 屏幕在持续重写下仍然闪烁 → 有程序抢 LUT。对照第 0 节进程列表处理。**\n");
    } else if visual_answer.as_deref() == Some("1") {
        report.push_str("  ** 肉眼确认写入生效 → 写入链路全通。问题应该在 app 侧（应用路径/反馈缺失），回到 app 排查。**\n");
    } else {
        report.push_str("  ? 缺少肉眼证据（第 6 节），请重跑一次并盯着屏幕那段 6 秒。\n");
    }

    report.push_str("\n（如果第 2 节显示「app 不会写 LUT」，那无论写入能力如何，点这个 ICC 都不会有变化。）\n");
    finish(report);
}

fn sample(ramp: &[u16], channel: usize) -> String {
    let points = [0usize, 1, 16, 64, 128, 192, 240, 254, 255];
    points
        .iter()
        .map(|p| format!("{p}:{}", ramp[channel * 256 + p] >> 8))
        .collect::<Vec<_>>()
        .join(" ")
}

fn finish(report: String) {
    println!("\n{report}");
    let path = desktop_dir().join(format!("gamma-debug-{}.txt", timestamp()));
    match std::fs::write(&path, &report) {
        Ok(()) => {
            println!("报告已保存到：{}", path.display());
            println!("请把这个 txt 发回给我。");
        }
        Err(error) => println!("报告写盘失败：{error}"),
    }
}
