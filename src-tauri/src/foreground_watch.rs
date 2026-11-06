//! 前台窗口监听（EVENT_SYSTEM_FOREGROUND，WinEvent）。
//!
//! 为什么不用轮询：前台切换是低频事件，`SetWinEventHook` 的 OUTOFCONTEXT 回调
//! 由系统投递到注册线程的消息循环，空闲时零开销——符合「不开定时全进程枚举」
//! 的约束。
//!
//! 线程模型（依据 SetWinEventHook / UnhookWinEvent 官方文档）：
//! - `WINEVENT_OUTOFCONTEXT` 的回调经由**注册线程**的消息循环投递，因此本模块
//!   用专用线程 "foreground-watch" 注册 hook 后跑 `GetMessageW` 消息循环；
//! - `UnhookWinEvent` 从非注册线程调用会直接失败，所以 `shutdown_runtime` 只向
//!   该线程投递 `WM_QUIT`，线程在消息循环退出后**同线程**自行卸载 hook。
//!
//! 回调约束（按计划）：只把 HWND/PID 信号投递给进程监听线程——不读 LUT、
//! 不做进程快照、不调用任何颜色写入；真正的目标进程匹配与恢复窗口调度都在
//! watcher 线程完成。
//!
//! 失败边界：hook 注册失败只记日志并放弃，保留 WMI 监听与启动窗口，不影响
//! 原有监听。
//!
//! 签名核对：SetWinEventHook / UnhookWinEvent / HWINEVENTHOOK / WINEVENTPROC
//! 在 windows 0.58 中位于 `Win32::UI::Accessibility`（需 `Win32_UI_Accessibility`
//! feature）；`EVENT_SYSTEM_FOREGROUND`(u32=3)、`WINEVENT_OUTOFCONTEXT`(u32=0)、
//! `GetWindowThreadProcessId`、`PostThreadMessageW`、`GetMessageW`、`WM_QUIT`
//! 位于 `Win32::UI::WindowsAndMessaging`（既有 feature），均对照本地 crate 源码
//! 与 Microsoft Learn 文档核实。

use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, GetWindowThreadProcessId, PeekMessageW, PostThreadMessageW,
    TranslateMessage, EVENT_SYSTEM_FOREGROUND, MSG, PM_NOREMOVE, WINEVENT_OUTOFCONTEXT, WM_QUIT,
};

/// hook 句柄（注册线程写入；卸载后清零）
static HOOK_HANDLE: AtomicIsize = AtomicIsize::new(0);
/// hook 线程的 Win32 线程 id（0 = 未运行）
static THREAD_ID: AtomicU32 = AtomicU32::new(0);

/// setup 阶段调用：起 hook 线程。任一环节失败只记日志，不影响应用其余功能。
pub fn start() {
    if THREAD_ID.load(Ordering::SeqCst) != 0 {
        return; // 已在运行
    }
    if std::thread::Builder::new()
        .name("foreground-watch".into())
        .spawn(foreground_watch_thread)
        .is_err()
    {
        eprintln!("[foreground-watch] spawn failed, foreground events disabled");
    }
}

fn foreground_watch_thread() {
    unsafe {
        let hook: HWINEVENTHOOK = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None, // OUTOFCONTEXT：回调在本进程代码里，hmod 传 NULL
            Some(foreground_event_proc),
            0, // 所有进程
            0, // 所有线程
            WINEVENT_OUTOFCONTEXT,
        );
        if hook.is_invalid() {
            eprintln!(
                "[foreground-watch] SetWinEventHook failed (winerr={:?}), foreground events disabled",
                windows::Win32::Foundation::GetLastError()
            );
            return;
        }
        HOOK_HANDLE.store(hook.0 as isize, Ordering::SeqCst);
        // PeekMessage(NOREMOVE) 强制为本线程创建消息队列：没有队列时
        // PostThreadMessageW(WM_QUIT)（shutdown 卸载路径）会失败
        let mut peek = MSG::default();
        let _ = PeekMessageW(&mut peek, None, 0, 0, PM_NOREMOVE);
        THREAD_ID.store(GetCurrentThreadId(), Ordering::SeqCst);
        eprintln!("[foreground-watch] EVENT_SYSTEM_FOREGROUND hook registered");

        // OUTOFCONTEXT 回调经由本线程的消息循环投递：跑到 WM_QUIT 为止
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // WM_QUIT（shutdown_runtime 投递）到达 → 同线程卸载 hook
        // （UnhookWinEvent 跨线程调用会失败，官方文档明示）
        let stored = HOOK_HANDLE.swap(0, Ordering::SeqCst);
        if stored != 0 {
            let _ = UnhookWinEvent(HWINEVENTHOOK(stored as *mut _));
            eprintln!("[foreground-watch] hook unregistered on owning thread");
        }
        THREAD_ID.store(0, Ordering::SeqCst);
    }
}

/// shutdown_runtime 调用：向 hook 线程投递 WM_QUIT，由该线程在同线程卸载 hook。
/// 线程尚未注册完成时 THREAD_ID 仍为 0 → 无操作（hook 未建立，无需卸载）。
pub fn shutdown() {
    let tid = THREAD_ID.load(Ordering::SeqCst);
    if tid != 0 {
        let result = unsafe { PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0)) };
        if let Err(e) = result {
            eprintln!("[foreground-watch] PostThreadMessageW(WM_QUIT) failed: {e}");
        }
    }
}

/// OBJID_WINDOW（0）：WinEvent 回调里顶层窗口的 id_object 值
const OBJID_WINDOW_VALUE: i32 = 0x0000_0000;

/// WinEvent 回调：只投递 HWND/PID 信号。id_object 过滤到顶层窗口
/// （OBJID_WINDOW = 0），pid 解析失败直接忽略。
unsafe extern "system" fn foreground_event_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    id_object: i32,
    _id_child: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    if id_object != OBJID_WINDOW_VALUE {
        return;
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == 0 {
        return;
    }
    crate::process_watcher::notify_color_env_changed(
        crate::process_watcher::ColorEnvEvent::Foreground {
            pid,
            hwnd: hwnd.0 as isize,
        },
    );
}
