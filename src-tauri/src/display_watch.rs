//! 显示器拓扑监听（系统事件驱动，无轮询）。
//!
//! Windows 在显示拓扑/模式变化时向所有顶层窗口广播两条消息：
//! - `WM_DISPLAYCHANGE`：分辨率 / 色深 / 显示模式变化。Win+P 投影切换、
//!   关闭显示器、热插拔线缆最终都体现为拓扑变化并触发它；
//! - `WM_DEVICECHANGE (DBT_DEVNODES_CHANGED)`：设备节点增删（显示器热插拔）。
//!
//! Tauri 2 没有把这两条消息透出给 JS（只透出 ScaleChanged），所以这里在主窗口
//! 上装一层 `SetWindowSubclass` 子类过程：收到消息只置「脏」标志并唤醒 worker
//! 线程；worker 经稳定窗口（跳过 Win+P / 热插拔过程的连发中间态）后重新枚举
//! 活动显示器，与上次不同才 emit `monitors-changed` 给前端。空闲时 worker
//! 阻塞在 Condvar 上，进程零轮询开销。
//!
//! 说明：`WM_DEVICECHANGE` 对任何设备变化都响（如 U 盘插拔），靠「枚举 +
//! 比对一次」过滤噪音——这只在事件到达时发生一次，不是周期轮询。
//!
//! 退出清理：子类过程随进程销毁，无需 RemoveWindowSubclass。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

use crate::icc::{self, DisplayMonitor};

const WM_DISPLAYCHANGE: u32 = 0x007E;
const WM_DEVICECHANGE: u32 = 0x0219;
const DBT_DEVNODES_CHANGED: usize = 0x0007;
/// 子类链里标识本模块的 id，值随意、非 0 即可
const SUBCLASS_ID: usize = 0x464D_5245;

/// 检测到变化后的静默稳定窗口：Win+P / 热插拔过程会连发多条消息，
/// 等消息静默这段时间再枚举，避免把瞬时中间态推给前端（标签闪动、选中误重置）。
const STABLE_WINDOW: Duration = Duration::from_millis(800);

static TOPOLOGY_DIRTY: AtomicBool = AtomicBool::new(false);
static TOPOLOGY_MTX: Mutex<()> = Mutex::new(());
static TOPOLOGY_CV: Condvar = Condvar::new();

type SubclassProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM, usize, usize) -> LRESULT;
type FnSetWindowSubclass = unsafe extern "system" fn(HWND, SubclassProc, usize, usize) -> BOOL;
type FnDefSubclassProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

/// 子类过程里要调的 DefSubclassProc 由 comctl32 动态加载而来，先存后用
static DEF_SUBCLASS_PROC: OnceLock<FnDefSubclassProc> = OnceLock::new();

unsafe extern "system" fn topology_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    _ref_data: usize,
) -> LRESULT {
    if msg == WM_DISPLAYCHANGE || (msg == WM_DEVICECHANGE && wparam.0 == DBT_DEVNODES_CHANGED) {
        TOPOLOGY_DIRTY.store(true, Ordering::SeqCst);
        let guard = TOPOLOGY_MTX.lock().unwrap();
        TOPOLOGY_CV.notify_all();
        drop(guard);
    }
    let def = DEF_SUBCLASS_PROC.get().expect("DefSubclassProc not loaded");
    def(hwnd, msg, wparam, lparam)
}

/// setup 阶段（主线程 = 窗口所属线程）调用：装子类 + 起 worker。
/// 任一环节失败只记日志并放弃，不影响应用其余功能。
pub fn start(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        eprintln!("[display-watch] main window not found, disabled");
        return;
    };
    // tauri 依赖树的 windows 与本项目直接依赖的 windows 版本不同，
    // hwnd() 返回的是 tauri 侧类型，取原始指针换成本项目的 HWND
    let hwnd_raw = match window.hwnd() {
        Ok(h) => h.0,
        Err(e) => {
            eprintln!("[display-watch] hwnd unavailable: {e}, disabled");
            return;
        }
    };
    let hwnd = HWND(hwnd_raw);

    unsafe {
        // comctl32 进程内常驻（webview 本就依赖它），这里不 FreeLibrary
        let Ok(comctl32) = LoadLibraryW(windows::core::w!("comctl32.dll")) else {
            eprintln!("[display-watch] comctl32.dll load failed, disabled");
            return;
        };
        let Some(set_proc) = GetProcAddress(comctl32, windows::core::s!("SetWindowSubclass")) else {
            eprintln!("[display-watch] SetWindowSubclass not found, disabled");
            return;
        };
        let Some(def_proc) = GetProcAddress(comctl32, windows::core::s!("DefSubclassProc")) else {
            eprintln!("[display-watch] DefSubclassProc not found, disabled");
            return;
        };
        let set_subclass: FnSetWindowSubclass = std::mem::transmute(set_proc);
        let def: FnDefSubclassProc = std::mem::transmute(def_proc);
        if DEF_SUBCLASS_PROC.set(def).is_err() {
            return;
        }
        if !set_subclass(hwnd, topology_subclass_proc, SUBCLASS_ID, 0).as_bool() {
            eprintln!("[display-watch] SetWindowSubclass failed, disabled");
            return;
        }
    }

    spawn_worker(app.clone());
}

fn spawn_worker(app: AppHandle) {
    std::thread::Builder::new()
        .name("display-watch".into())
        .spawn(move || {
            let mut last: Option<Vec<DisplayMonitor>> = None;
            loop {
                // 阻塞等事件信号（无事件零开销）
                {
                    let mut guard = TOPOLOGY_MTX.lock().unwrap();
                    while !TOPOLOGY_DIRTY.load(Ordering::SeqCst) {
                        guard = TOPOLOGY_CV.wait(guard).unwrap();
                    }
                    TOPOLOGY_DIRTY.store(false, Ordering::SeqCst);
                }
                // 稳定窗口：连发风暴期间反复顺延，直到静默 STABLE_WINDOW 再枚举
                loop {
                    std::thread::sleep(STABLE_WINDOW);
                    if !TOPOLOGY_DIRTY.swap(false, Ordering::SeqCst) {
                        break;
                    }
                }

                let current = icc::enumerate_display_monitors();
                if last.as_ref() == Some(&current) {
                    // 消息到了但显示器列表没变（如无关设备触发的 DBT_DEVNODES_CHANGED）
                    continue;
                }
                if let Err(e) = app.emit("monitors-changed", &current) {
                    eprintln!("[display-watch] emit failed: {e}");
                    continue;
                }
                eprintln!(
                    "[display-watch] topology changed: {} monitor(s)",
                    current.len()
                );
                last = Some(current);
            }
        })
        .expect("Failed to spawn display-watch thread");
}
