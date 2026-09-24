// 管理员运行 + 静默/计划任务自启 的 Windows 平台实现。
//
// 背景见 .docs/prd/2026-09-24-silent-autostart-admin-run.md。
// 两种开机自启机制互斥，由 reconcile_autostart() 根据 (autostart, run_as_admin)
// 与当前是否已提权来切换、并清理另一种，保证不会开机启动两次：
//   - 非管理员：tauri-plugin-autostart 写 HKCU Run 项（带 --silent 参数）
//   - 管理员  ：Windows 计划任务（登录触发 + 最高权限 + 交互令牌），可静默提权且无 UAC

use std::os::windows::process::CommandExt;
use std::process::Command;

use tauri::AppHandle;

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// 计划任务名（无空格，便于 schtasks 传参）
const TASK_NAME: &str = "FilterManageAutostart";
/// 静默启动标记：带此参数启动时不弹主窗口，仅进托盘（开机自启使用）
pub const SILENT_ARG: &str = "--silent";
/// 隐藏子进程控制台窗口，避免 schtasks 命令行闪现
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 当前进程是否以管理员（提升的令牌）运行
pub fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut ret_len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut core::ffi::c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    }
}
/// 以管理员权限重新启动自身（转发当前启动参数，如 --silent）。
/// 成功时直接结束当前进程（不返回，避免与提权实例并存）；用户取消 UAC 或失败时返回 Err。
pub fn relaunch_elevated() -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("获取自身路径失败: {e}"))?
        .to_string_lossy()
        .to_string();
    let exe_h = HSTRING::from(exe.as_str());

    // 转发除 argv[0] 外的启动参数（如 --silent）
    let params = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
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
        std::process::exit(0);
    } else {
        Err("用户取消授权或提权失败".to_string())
    }
}

fn schtasks() -> Command {
    let mut c = Command::new("schtasks");
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
    <Priority>7</Priority>
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
    let tmp = std::env::temp_dir().join("filter-manage-task.xml");
    std::fs::write(&tmp, &bytes).map_err(|e| format!("写任务 XML 失败: {e}"))?;

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

/// 删除计划任务；任务不存在时不报错（best-effort 清理）。
pub fn delete_scheduled_task() {
    let _ = schtasks()
        .args(["/Delete", "/TN", TASK_NAME, "/F"])
        .status();
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
            // 计划任务负责自启 → 移除注册表 Run 项
            let _ = launcher.disable();
            // 仅在已提权时能创建最高权限任务；未提权则等提权后由 setup() 再次建立
            if is_elevated() {
                create_scheduled_task()?;
            }
        }
        (true, false) => {
            delete_scheduled_task();
            launcher
                .enable()
                .map_err(|e| format!("启用注册表自启失败: {e}"))?;
        }
        (false, _) => {
            delete_scheduled_task();
            let _ = launcher.disable();
        }
    }
    Ok(())
}

// ─── Tauri 命令 ───────────────────────────────────────────────────────────────

/// 当前是否以管理员运行
#[tauri::command]
pub fn is_running_as_admin() -> bool {
    is_elevated()
}

/// 设置「以管理员身份运行」。持久化开关后：
///   - 开启且当前未提权 → 以管理员重启（触发一次 UAC）；提权实例的 setup() 建立计划任务
///   - 开启且已提权     → 立即按自启开关建立/清理机制
///   - 关闭             → 删除计划任务并按需回退到注册表自启（下次普通启动即为非管理员）
#[tauri::command]
pub fn set_run_as_admin(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = crate::config::get_app_settings()?;
    settings.run_as_admin = enabled;
    crate::config::save_app_settings(settings.clone())?;

    if enabled {
        if is_elevated() {
            reconcile_autostart(&app, settings.autostart, true)
        } else {
            match relaunch_elevated() {
                Ok(()) => Ok(()), // 成功即 exit，不会返回
                Err(e) => {
                    // 用户取消 UAC → 回滚开关，避免下次启动反复弹 UAC
                    let mut rollback = crate::config::get_app_settings()?;
                    rollback.run_as_admin = false;
                    let _ = crate::config::save_app_settings(rollback);
                    Err(e)
                }
            }
        }
    } else {
        reconcile_autostart(&app, settings.autostart, false)
    }
}
