// 管理员运行 + 静默/计划任务自启 的 Windows 平台实现。
//
// 背景见 .docs/prd/2026-09-24-silent-autostart-admin-run.md。
// 两种开机自启机制互斥，由 reconcile_autostart() 根据 (autostart, run_as_admin)
// 与当前是否已提权来切换、并清理另一种，保证不会开机启动两次：
//   - 非管理员：tauri-plugin-autostart 写 HKCU Run 项（带 --silent 参数）
//   - 管理员  ：Windows 计划任务（登录触发 + 最高权限 + 交互令牌），可静默提权且无 UAC

use std::io::Write;
use std::os::windows::process::CommandExt;
use std::process::Command;

use tauri::AppHandle;

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{
    GetTokenInformation, TokenElevation, TokenElevationType, TokenElevationTypeLimited,
    TOKEN_ELEVATION, TOKEN_ELEVATION_TYPE, TOKEN_INFORMATION_CLASS, TOKEN_QUERY,
};
use windows::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, WaitForSingleObject, PROCESS_SYNCHRONIZE,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};
use winreg::RegKey;

/// 计划任务名（无空格，便于 schtasks 传参）
const TASK_NAME: &str = "FilterManageAutostart";
/// 静默启动标记：带此参数启动时不弹主窗口，仅进托盘（开机自启使用）
pub const SILENT_ARG: &str = "--silent";
/// 自提权重启时附带旧进程 PID（形如 `--relaunch-wait=1234`），新实例据此等旧进程退出
const RELAUNCH_WAIT_ARG: &str = "--relaunch-wait=";
/// 隐藏子进程控制台窗口，避免 schtasks 命令行闪现
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// tauri-plugin-autostart 写开机自启项的注册表位置（HKCU）
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// 最高权限计划任务只能在已提权时创建/删除；未提权时返回这条错误，而不是假装成功
const NEED_ELEVATION_MSG: &str =
    "需要管理员权限才能修改开机任务，请允许 UAC 以管理员身份打开应用后再试";

/// 读取当前进程令牌的一项信息
fn token_info<T: Default>(class: TOKEN_INFORMATION_CLASS) -> Option<T> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).ok()?;
        let mut value = T::default();
        let mut ret_len = 0u32;
        let ok = GetTokenInformation(
            token,
            class,
            Some(&mut value as *mut T as *mut core::ffi::c_void),
            std::mem::size_of::<T>() as u32,
            &mut ret_len,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok.then_some(value)
    }
}

/// 当前进程是否以管理员（提升的令牌）运行
pub fn is_elevated() -> bool {
    token_info::<TOKEN_ELEVATION>(TokenElevation).is_some_and(|e| e.TokenIsElevated != 0)
}

/// 当前账户能否「以本人身份」提权：已提权，或开着 UAC 的管理员（受限的拆分令牌）。
/// 标准用户在 UAC 里填的是另一个管理员账户的凭据，提权后的进程属于那个账户，
/// 读写的是它的 %APPDATA% 和计划任务，所以不支持。
pub fn can_elevate() -> bool {
    is_elevated()
        || token_info::<TOKEN_ELEVATION_TYPE>(TokenElevationType)
            == Some(TokenElevationTypeLimited)
}

/// 以管理员权限重新启动自身。成功时直接结束当前进程（不返回，避免与提权实例并存）；
/// 用户取消 UAC 或失败时返回 Err。
/// `keep_silent` 为 false 时去掉 --silent：在设置里开启时用户正看着界面，新实例必须显示窗口。
pub fn relaunch_elevated(app: &AppHandle, keep_silent: bool) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("获取自身路径失败: {e}"))?
        .to_string_lossy()
        .to_string();
    let exe_h = HSTRING::from(exe.as_str());

    // 转发除 argv[0] 外的启动参数，并附带本进程 PID：
    // 新实例要等本进程完全退出后再初始化，否则单实例插件会把它当成「第二个实例」直接退出
    let mut args: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with(RELAUNCH_WAIT_ARG) && (keep_silent || a != SILENT_ARG))
        .collect();
    args.push(format!("{RELAUNCH_WAIT_ARG}{}", std::process::id()));
    let params = args.join(" ");
    let params_h = HSTRING::from(params.as_str());
    let verb_h = HSTRING::from("runas");

    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb_h.as_ptr()),
            PCWSTR(exe_h.as_ptr()),
            PCWSTR(params_h.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };

    // ShellExecuteW 返回值 <= 32 表示失败（含用户取消 UAC → SE_ERR_ACCESSDENIED）
    if (result.0 as isize) > 32 {
        // 先移除托盘图标、隐藏窗口，避免退出后残留幽灵托盘图标
        app.cleanup_before_exit();
        std::process::exit(0);
    } else {
        Err("用户取消授权或提权失败".to_string())
    }
}

/// 若本进程是 relaunch_elevated 拉起的提权实例，先等旧进程退出（最多 5 秒）。
/// 须在 tauri::Builder 之前调用：旧进程还活着时，单实例插件会判定已有实例并让新实例退出，
/// 结果两个进程都没了。
pub fn wait_for_relaunch_parent() {
    let pid = std::env::args()
        .find_map(|a| a.strip_prefix(RELAUNCH_WAIT_ARG)?.parse::<u32>().ok());
    let Some(pid) = pid else { return };
    unsafe {
        if let Ok(handle) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            let _ = WaitForSingleObject(handle, 5000);
            let _ = CloseHandle(handle);
        }
    }
}

fn schtasks() -> Command {
    // 用 System32 下的绝对路径：按裸名查找时会先搜程序所在目录，而按用户安装时那个目录普通权限就能写
    let mut buf = [0u16; 260];
    let len = unsafe { GetSystemDirectoryW(Some(&mut buf)) } as usize;
    let dir = if len > 0 && len < buf.len() {
        String::from_utf16_lossy(&buf[..len])
    } else {
        r"C:\Windows\System32".to_string()
    };
    let mut c = Command::new(format!(r"{dir}\schtasks.exe"));
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

fn current_user() -> String {
    let user = std::env::var("USERNAME").unwrap_or_default();
    let domain = std::env::var("USERDOMAIN")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_default();
    if domain.is_empty() {
        user
    } else {
        format!("{domain}\\{user}")
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Priority 用 5：计划任务默认的 7 对应「低于正常」进程优先级，4~6 才是正常优先级
fn build_task_xml(exe: &str, user: &str) -> String {
    let exe = xml_escape(exe);
    let user = xml_escape(user);
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Filter Manage 开机静默自启（管理员）</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>false</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>5</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
      <Arguments>{arg}</Arguments>
    </Exec>
  </Actions>
</Task>
"#,
        user = user,
        exe = exe,
        arg = SILENT_ARG,
    )
}
/// 创建/更新计划任务（登录触发 + 最高权限）。需当前进程已提权。
pub fn create_scheduled_task() -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("获取自身路径失败: {e}"))?
        .to_string_lossy()
        .to_string();
    let xml = build_task_xml(&exe, &current_user());

    // Task Scheduler XML 需 UTF-16LE + BOM
    let mut bytes: Vec<u8> = vec![0xFF, 0xFE];
    for u in xml.encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    // 唯一文件名 + 独占创建：提权的 schtasks 会读这个文件，固定路径可能被同用户的其他进程抢先放入或替换
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let tmp = std::env::temp_dir().join(format!(
        "filter-manage-task-{}-{nanos}.xml",
        std::process::id()
    ));
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .and_then(|mut f| f.write_all(&bytes))
        .map_err(|e| format!("写任务 XML 失败: {e}"))?;

    let status = schtasks()
        .args(["/Create", "/TN", TASK_NAME, "/XML"])
        .arg(&tmp)
        .arg("/F")
        .status()
        .map_err(|e| format!("执行 schtasks 失败: {e}"))?;

    let _ = std::fs::remove_file(&tmp);

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "创建计划任务失败（schtasks 退出码 {:?}），通常是权限不足",
            status.code()
        ))
    }
}

/// 删除计划任务；任务不存在视为成功。删除失败时返回 Err（常见原因是未提权删不掉最高权限任务），
/// 调用方不能吞掉：残留的任务会在登录时继续以管理员身份启动应用。
pub fn delete_scheduled_task() -> Result<(), String> {
    if !scheduled_task_exists() {
        return Ok(());
    }
    let deleted = schtasks()
        .args(["/Delete", "/TN", TASK_NAME, "/F"])
        .status()
        .is_ok_and(|s| s.success());
    if deleted {
        Ok(())
    } else if is_elevated() {
        Err("删除开机计划任务失败".to_string())
    } else {
        Err(NEED_ELEVATION_MSG.to_string())
    }
}

/// 计划任务是否存在
pub fn scheduled_task_exists() -> bool {
    schtasks()
        .args(["/Query", "/TN", TASK_NAME])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// 根据目标 (autostart, run_as_admin) 与当前提权状态切换到正确的开机自启机制，
/// 并清理另一种，保证二者互斥、不会开机启动两次。
pub fn reconcile_autostart(
    app: &AppHandle,
    autostart: bool,
    run_as_admin: bool,
) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let launcher = app.autolaunch();

    match (autostart, run_as_admin) {
        (true, true) => {
            // 最高权限任务只能在已提权时创建；未提权直接报错，不能先删了 Run 项再假装成功
            if !is_elevated() {
                return Err(NEED_ELEVATION_MSG.to_string());
            }
            // 任务建成功后再移除注册表 Run 项，避免两种自启都没有
            create_scheduled_task()?;
            let _ = launcher.disable();
        }
        (true, false) => {
            delete_scheduled_task()?;
            launcher
                .enable()
                .map_err(|e| format!("启用注册表自启失败: {e}"))?;
        }
        (false, _) => {
            delete_scheduled_task()?;
            let _ = launcher.disable();
        }
    }
    Ok(())
}

/// 旧版本写入的注册表自启项不带 --silent，升级后开机仍会弹窗：给这类旧值补上参数。
/// 只在原值后追加，不改其中的 exe 路径（dev 构建、便携版不会把安装版的自启项改成指向自己），
/// 也不碰任务管理器「启动」页的启用/禁用状态。
pub fn add_silent_to_legacy_run_entry(app: &AppHandle) {
    let name = &app.package_info().name;
    let Ok(key) = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(RUN_KEY, KEY_READ | KEY_SET_VALUE)
    else {
        return;
    };
    if let Ok(value) = key.get_value::<String, _>(name) {
        if !value.contains(SILENT_ARG) {
            let _ = key.set_value(name, &format!("{} {SILENT_ARG}", value.trim_end()));
        }
    }
}

// ─── Tauri 命令 ───────────────────────────────────────────────────────────────

/// 当前是否以管理员运行
#[tauri::command]
pub fn is_running_as_admin() -> bool {
    is_elevated()
}

/// 设置「以管理员身份运行」：
///   - 开启且当前未提权 → 保存开关后以管理员重启（触发一次 UAC），提权实例的 setup() 建立计划任务；
///     用户取消 UAC 时回滚开关
///   - 已提权时开启 / 关闭 → 先切换开机自启机制，成功后再保存；失败时开关保持原状
#[tauri::command]
pub fn set_run_as_admin(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = crate::config::get_app_settings()?;

    if enabled && !is_elevated() {
        if !can_elevate() {
            return Err("当前 Windows 账户不是管理员，无法以管理员身份运行".to_string());
        }
        settings.run_as_admin = true;
        crate::config::save_app_settings(settings)?;
        // 用户正在操作界面，提权后的新实例要显示窗口，所以不保留 --silent
        return relaunch_elevated(&app, false).map_err(|e| {
            // 用户取消 UAC → 回滚开关，避免下次启动反复弹 UAC
            if let Ok(mut rollback) = crate::config::get_app_settings() {
                rollback.run_as_admin = false;
                let _ = crate::config::save_app_settings(rollback);
            }
            e
        });
    }

    reconcile_autostart(&app, settings.autostart, enabled)?;
    settings.run_as_admin = enabled;
    crate::config::save_app_settings(settings)
}
