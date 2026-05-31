// windows_impl.rs — Windows 平台特定实现

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::thread::{self, JoinHandle, ThreadId};

use tauri::Manager;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, POINT, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CallWindowProcW, DefWindowProcW, DispatchMessageW, GetCursorPos,
    GetMessageW, GetWindowLongPtrW, GetWindowRect, PostThreadMessageW, SetWindowLongPtrW,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
    GWLP_WNDPROC, HTCAPTION, HTCLOSE, HTMAXBUTTON, HTMINBUTTON,
    MSG, WH_MOUSE_LL, WM_LBUTTONUP, WM_NCHITTEST, WM_QUIT,
};

use crate::commands::TitlebarRegion;

// ── 窗口子类化 ─────────────────────────────────

static WINDOW_STATES: LazyLock<Mutex<HashMap<isize, WindowState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 主窗口 HWND，用于拖拽合并时判定鼠标是否位于主窗口上方
static MAIN_WINDOW_HWND: LazyLock<Mutex<Option<isize>>> =
    LazyLock::new(|| Mutex::new(None));

#[derive(Clone)]
struct WindowState {
    original_proc: isize,
    regions: Vec<(i32, i32, i32, i32, String)>,
}

pub unsafe fn install_subclass(hwnd_raw: isize) {
    // 记录主窗口 HWND（install_subclass 仅在 setup 中对 main 窗口调用）
    MAIN_WINDOW_HWND.lock().unwrap().replace(hwnd_raw);

    let hwnd = HWND(hwnd_raw as *mut _);
    let original = GetWindowLongPtrW(hwnd, GWLP_WNDPROC);

    WINDOW_STATES.lock().unwrap().insert(
        hwnd_raw,
        WindowState { original_proc: original, regions: Vec::new() },
    );

    SetWindowLongPtrW(hwnd, GWLP_WNDPROC, subclass_proc_addr());
    log::info!("[titlebar] 子类化已安装: 0x{:x}", hwnd_raw);
}

pub fn set_regions(hwnd_raw: isize, regions: Vec<TitlebarRegion>) {
    let converted: Vec<_> = regions.into_iter()
        .map(|r| (r.x, r.y, r.width, r.height, r.kind))
        .collect();
    if let Ok(mut m) = WINDOW_STATES.lock() {
        if let Some(s) = m.get_mut(&hwnd_raw) {
            s.regions = converted;
        }
    }
}

fn subclass_proc_addr() -> isize {
    let fp: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT = subclass_proc;
    fp as usize as isize
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let hwnd_raw = hwnd.0 as isize;

    // try_lock 防止嵌套消息死锁：如果已锁定（CallWindowProcW 嵌套回调），
    // 直接转发给原始过程，不做 hit-test 处理
    let (original, regions) = match WINDOW_STATES.try_lock() {
        Ok(states) => {
            let state = states.get(&hwnd_raw).cloned();
            state.map(|s| (s.original_proc, s.regions)).unwrap_or((0, Vec::new()))
        }
        Err(_) => (0, Vec::new()),
    };

    if msg == WM_NCHITTEST && !regions.is_empty() {
        let mx = ((lparam.0 as u64) & 0xFFFF) as i32;
        let my = (((lparam.0 as u64) >> 16) & 0xFFFF) as i32;  // ← mask 高位

        for (rx, ry, rw, rh, ref kind) in &regions {
            if mx >= *rx && mx <= *rx + *rw && my >= *ry && my <= *ry + *rh {
                let hit = match kind.as_str() {
                    "maxbutton" => HTMAXBUTTON,
                    "minbutton" => HTMINBUTTON,
                    "closebutton" => HTCLOSE,
                    _ => HTCAPTION,
                };
                return LRESULT(hit as isize);
            }
        }
    }

    if original != 0 {
        let proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT =
            std::mem::transmute(original);
        CallWindowProcW(Some(proc), hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

// ── 全局鼠标钩子 ─────────────────────────────

static HOOK_THREAD: Mutex<Option<(ThreadId, JoinHandle<()>)>> = Mutex::new(None);

/// 前端传入的设备像素比（devicePixelRatio），用于 CSS→物理像素坐标转换
static DPI_SCALE: LazyLock<Mutex<f64>> = LazyLock::new(|| Mutex::new(1.0));

pub fn start_drag(_path: String, _label: String, device_pixel_ratio: f64) {
    eprintln!("[titlebar::hook] start_drag 调用");

    // 记录前端传入的 DPI 缩放比
    if device_pixel_ratio > 0.0 {
        *DPI_SCALE.lock().unwrap() = device_pixel_ratio;
    }

    // 延迟初始化主窗口 HWND（setup 阶段可能早于窗口创建）
    {
        let mut guard = MAIN_WINDOW_HWND.lock().unwrap();
        if guard.is_none() {
            if let Some(handle) = event_system::GLOBAL_APPHANDLE.get() {
                if let Some(window) = handle.get_webview_window("main") {
                    if let Ok(hwnd) = window.hwnd() {
                        let raw = hwnd.0 as isize;
                        eprintln!("[titlebar::hook] 延迟记录主窗口 HWND: 0x{:x}", raw);
                        guard.replace(raw);
                    }
                }
            }
        }
    }

    let mut guard = HOOK_THREAD.lock().unwrap();
    if guard.is_some() {
        eprintln!("[titlebar::hook] 钩子已存在，跳过");
        return;
    }

    let handle = thread::spawn(|| unsafe { hook_thread() });
    *guard = Some((handle.thread().id(), handle));
}

pub fn stop_drag() {
    eprintln!("[titlebar::hook] stop_drag 调用");
    let mut guard = HOOK_THREAD.lock().unwrap();
    if let Some((tid, _handle)) = guard.take() {
        // 向钩子线程发送 WM_QUIT（而非主线程）
        let _ = unsafe { PostThreadMessageW(tid_to_u32(tid), WM_QUIT, WPARAM(0), LPARAM(0)) };
    }
}

#[cfg(target_os = "windows")]
fn tid_to_u32(tid: ThreadId) -> u32 {
    // ThreadId 内部是 u64，低 32 位是实际线程 ID
    unsafe { std::mem::transmute::<ThreadId, u64>(tid) as u32 }
}

unsafe fn hook_thread() {
    let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(hook_proc), None, 0);
    let Ok(_hook) = hook else { return; };
    log::info!("[titlebar] 拖拽追踪已启动");

    let mut msg = MSG::default();
    loop {
        let ret = GetMessageW(&mut msg, None, 0, 0);
        if ret.0 <= 0 { break; }
        let _ = TranslateMessage(&msg);
        let _ = DispatchMessageW(&msg);
    }

    let _ = UnhookWindowsHookEx(_hook);
    log::info!("[titlebar] 拖拽追踪已停止");
}

unsafe extern "system" fn hook_proc(
    code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if code >= 0 && w_param.0 == WM_LBUTTONUP as usize {
        eprintln!("[titlebar::hook] WM_LBUTTONUP 检测到");
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);

        // 获取主窗口 HWND（从 setup 阶段存储）
        let main_hwnd_raw = MAIN_WINDOW_HWND
            .lock().ok().and_then(|g| *g).unwrap_or(0);

        // 坐标比对：主窗口 Nav 区域 ±12px（CSS Grid row3: Y=72~112）
        let is_over_target = if main_hwnd_raw != 0 {
            let main_hwnd = HWND(main_hwnd_raw as *mut _);
            let mut rect = RECT::default();
            let _ = GetWindowRect(main_hwnd, &mut rect);
            let scale = *DPI_SCALE.lock().unwrap();
            let nav_top = rect.top + (60.0 * scale) as i32;
            let nav_bottom = rect.top + (124.0 * scale) as i32;
            let hit = pt.x >= rect.left && pt.x <= rect.right
                   && pt.y >= nav_top && pt.y <= nav_bottom;
            eprintln!(
                "[titlebar::hook] dpi_scale={:.2}  nav_zone=({},{},{},{})  cursor=({},{})  hit={}",
                scale, rect.left, nav_top, rect.right, nav_bottom, pt.x, pt.y, hit
            );
            hit
        } else {
            eprintln!("[titlebar::hook] 主窗口 HWND 未记录，无法判定");
            false
        };

        if is_over_target {
            eprintln!("[titlebar::hook] 光标在 Nav 区域上，发射 drag-release");
            if let Some(handle) = event_system::GLOBAL_APPHANDLE.get() {
                let payload = serde_json::json!({
                    "screenX": pt.x,
                    "screenY": pt.y,
                });
                let _ = tauri::Emitter::emit(handle, "drag-release", payload);
            }
        } else {
            eprintln!("[titlebar::hook] 光标不在 Nav 区域内，忽略");
        }

        // 停止钩子消息泵
        let _ = PostThreadMessageW(
            tid_to_u32(thread::current().id()),
            WM_QUIT,
            WPARAM(0),
            LPARAM(0),
        );
    }
    CallNextHookEx(None, code, w_param, l_param)
}
