// windows_impl.rs — Windows 平台特定实现

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::thread::{self, JoinHandle, ThreadId};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, POINT};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CallWindowProcW, DefWindowProcW, DispatchMessageW, GetCursorPos,
    GetMessageW, GetWindowLongPtrW, PostThreadMessageW, SetWindowLongPtrW,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WindowFromPoint,
    GWLP_WNDPROC, HTCAPTION, HTCLOSE, HTMAXBUTTON, HTMINBUTTON,
    MSG, WH_MOUSE_LL, WM_LBUTTONUP, WM_NCHITTEST, WM_QUIT,
};

use crate::commands::TitlebarRegion;

// ── 窗口子类化 ─────────────────────────────────

static WINDOW_STATES: LazyLock<Mutex<HashMap<isize, WindowState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Clone)]
struct WindowState {
    original_proc: isize,
    regions: Vec<(i32, i32, i32, i32, String)>,
}

pub unsafe fn install_subclass(hwnd_raw: isize) {
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

pub fn start_drag(_path: String, _label: String) {
    let mut guard = HOOK_THREAD.lock().unwrap();
    if guard.is_some() { return; }

    let handle = thread::spawn(|| unsafe { hook_thread() });
    *guard = Some((handle.thread().id(), handle));
}

pub fn stop_drag() {
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
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _under = WindowFromPoint(pt);

        // 通过事件系统通知前端合并
        if let Some(handle) = event_system::GLOBAL_APPHANDLE.get() {
            let payload = serde_json::json!({
                "screenX": pt.x,
                "screenY": pt.y,
            });
            let _ = tauri::Emitter::emit(handle, "drag-release", payload);
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
