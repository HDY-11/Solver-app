// drag_tracker.rs — 全局鼠标钩子，追踪跨窗口拖拽

use std::sync::Mutex;
use std::thread;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM, POINT};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetCursorPos, GetMessageW, PostQuitMessage,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WindowFromPoint,
    MSG, MSLLHOOKSTRUCT, WH_MOUSE_LL, WM_LBUTTONUP,
};

static DRAG_STATE: Mutex<Option<DragState>> = Mutex::new(None);

struct DragState {
    hook_thread: Option<thread::JoinHandle<()>>,
}

/// 启动拖拽追踪
pub fn start(_tab_path: String, _tab_label: String) {
    let mut state = DRAG_STATE.lock().unwrap();
    if state.is_some() {
        return;
    }

    let handle = thread::spawn(move || {
        hook_thread();
    });

    *state = Some(DragState {
        hook_thread: Some(handle),
    });
}

/// 停止拖拽追踪
pub fn stop() {
    if let Ok(mut state) = DRAG_STATE.lock() {
        if let Some(_s) = state.take() {
            unsafe { PostQuitMessage(0) };
        }
    }
}

fn hook_thread() {
    let hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(hook_proc), None, 0) };
    let Ok(hook) = hook else {
        log::error!("[drag_tracker] 安装全局鼠标钩子失败");
        return;
    };

    log::info!("[drag_tracker] 开始追踪拖拽");

    let mut msg = MSG::default();
    loop {
        let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if ret.0 <= 0 {
            break;
        }
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    unsafe { let _ = UnhookWindowsHookEx(hook); }
    log::info!("[drag_tracker] 停止追踪拖拽");
}

unsafe extern "system" fn hook_proc(
    code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if code >= 0 {
        let info = &*(l_param.0 as *const MSLLHOOKSTRUCT);
        if w_param.0 == WM_LBUTTONUP as usize {
            let mut pt = POINT::default();
            unsafe { GetCursorPos(&mut pt) };
            let hwnd_under = unsafe { WindowFromPoint(pt) };
            log::debug!("[drag_tracker] 左键释放: ({}, {}), under=0x{:x}",
                pt.x, pt.y, hwnd_under.0 as usize);

            unsafe { PostQuitMessage(0) };
        }
    }
    unsafe { CallNextHookEx(None, code, w_param, l_param) }
}

