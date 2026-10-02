// 管理员运行 + 计划任务自启 的 Windows 平台实现。
//
// 背景见 .docs/prd/2026-09-24-silent-autostart-admin-run.md。
// build.rs 给 exe 写入 requireAdministrator 清单：每次启动强制 UAC、进程必已提权，
// 因此开机自启只有一种可行机制——Windows 计划任务（登录触发 + 最高权限 + 交互令牌），
// 开机静默提权且无 UAC。注册表 Run 项在提权清单下开机必弹 UAC，只作为旧版残留清理。

use std::io::Write;
use std::os::windows::process::CommandExt;
use std::process::Command;

use tauri::AppHandle;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_INFORMATION_CLASS, TOKEN_QUERY,
};
use windows::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// 计划任务名（无空格，便于 schtasks 传参）
const TASK_NAME: &str = "FilterManageAutostart";
/// 静默启动标记：带此参数启动时不弹主窗口，仅进托盘（开机自启使用）
pub const SILENT_ARG: &str = "--silent";
/// 隐藏子进程控制台窗口，避免 schtasks 命令行闪现
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// 未提权时删不掉最高权限计划任务，返回这条错误，而不是假装成功
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
fn build_task_xml(exe: &str, user: &str, args: &str) -> String {
    let exe = xml_escape(exe);
    let user = xml_escape(user);
    // 无参数（开机显示主窗口）时省略 <Arguments> 元素
    let arguments = if args.is_empty() {
        String::new()
    } else {
        format!("\n      <Arguments>{}</Arguments>", xml_escape(args))
    };
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
      <Command>{exe}</Command>{arguments}
    </Exec>
  </Actions>
</Task>
"#,
        user = user,
        exe = exe,
        arguments = arguments,
    )
}
/// 创建/更新计划任务（登录触发 + 最高权限）。`silent` 决定启动参数是否带 `--silent`（仅进托盘）。
/// 需当前进程已提权。
pub fn create_scheduled_task(silent: bool) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("获取自身路径失败: {e}"))?
        .to_string_lossy()
        .to_string();
    let xml = build_task_xml(&exe, &current_user(), if silent { SILENT_ARG } else { "" });

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

/// 同步开机自启到目标状态：
///
///   - 开：按 `silent` 决定的启动参数覆盖注册计划任务（已存在也重注册，自愈参数/路径变更），
///     并清理旧版注册表自启项
///   - 关：删除计划任务与注册表自启项
///
/// 计划任务的创建/删除需要提权；requireAdministrator 清单保证正常情况下本进程已是管理员。
pub fn reconcile_autostart(app: &AppHandle, autostart: bool, silent: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let launcher = app.autolaunch();

    if autostart {
        // 覆盖注册（/F）：成本只是启动时多跑一次 schtasks，换来设置变更/升级换路径后的自愈
        create_scheduled_task(silent)?;
        // Run 项在 requireAdministrator 清单下会开机弹 UAC，必须清掉旧版残留
        let _ = launcher.disable();
    } else {
        delete_scheduled_task()?;
        let _ = launcher.disable();
    }
    Ok(())
}

// ─── Tauri 命令 ───────────────────────────────────────────────────────────────

/// 当前是否以管理员运行
#[tauri::command]
pub fn is_running_as_admin() -> bool {
    is_elevated()
}
