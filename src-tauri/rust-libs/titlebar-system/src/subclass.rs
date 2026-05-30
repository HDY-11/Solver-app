// subclass.rs — 窗口子类化，拦截 WM_NCHITTEST

use std::collections::HashMap;
use std::sync::Mutex;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, GetWindowLongPtrW, SetWindowLongPtrW,
    GWLP_WNDPROC, HTCAPTION, HTCLOSE, HTMAXBUTTON, HTMINBUTTON,
    WM_NCHITTEST, WNDPROC,
};

use crate::TitlebarRegion;

struct WindowState {
    original_proc: WNDPROC,
    regions: Vec<TitlebarRegion>,
}

static WINDOW_STATES: Mutex<Option<HashMap<isize, WindowState>>> = Mutex::new(None);

/// 安装窗口子类化
///
/// # Safety
/// hwnd 必须是有效窗口句柄
pub unsafe fn install(hwnd: HWND) -> Result<(), String> {
    let hwnd_key = hwnd.0 as isize;

    // 保存原始窗口过程
    let original = GetWindowLongPtrW(hwnd, GWLP_WNDPROC);
    if original == 0 {
        return Err("GetWindowLongPtrW failed".into());
    }
    // 在 windows 0.62 中，GetWindowLongPtrW 返回 isize，需要转换为 WNDPROC
    let original_proc: WNDPROC = unsafe { std::mem::transmute(original) };

    let mut states = WINDOW_STATES.lock().map_err(|e| e.to_string())?;
    if states.is_none() {
        *states = Some(HashMap::new());
    }
    states.as_mut().unwrap().insert(
        hwnd_key,
        WindowState { original_proc, regions: Vec::new() },
    );

    // 设置新窗口过程
    SetWindowLongPtrW(hwnd, GWLP_WNDPROC, subclass_proc as usize as isize);

    log::info!("[titlebar] 窗口子类化已安装: hwnd=0x{:x}", hwnd_key);
    Ok(())
}

/// 更新窗口的自定义区域配置
pub fn set_regions(hwnd: HWND, regions: Vec<TitlebarRegion>) {
    let hwnd_key = hwnd.0 as isize;
    if let Ok(mut states) = WINDOW_STATES.lock() {
        if let Some(ref mut map) = *states {
            if let Some(state) = map.get_mut(&hwnd_key) {
                state.regions = regions;
            }
        }
    }
}

/// 自定义窗口过程
unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let hwnd_key = hwnd.0 as isize;

    // 获取窗口状态
    let (original_proc, regions) = {
        let states = match WINDOW_STATES.lock() {
            Ok(s) => s,
            Err(_) => return LRESULT(0),
        };
        let empty = Vec::new();
        let default_proc = unsafe { std::mem::zeroed::<WNDPROC>() };
        match states.as_ref() {
            Some(map) => {
                let state = map.get(&hwnd_key);
                match state {
                    Some(s) => (Some(s.original_proc), s.regions.clone()),
                    None => (None, empty),
                }
            }
            None => (None, empty),
        }
    };

    if msg == WM_NCHITTEST {
        // lparam: LOWORD = x, HIWORD = y (屏幕坐标)
        let mx = (lparam.0 as u32 & 0xFFFF) as i32;
        let my = ((lparam.0 as u32 >> 16) & 0xFFFF) as i32;

        for region in &regions {
            if mx >= region.x && mx <= region.x + region.width
                && my >= region.y && my <= region.y + region.height
            {
                let hit = match region.kind {
                    crate::RegionKind::Caption => HTCAPTION,
                    crate::RegionKind::MaxButton => HTMAXBUTTON,
                    crate::RegionKind::MinButton => HTMINBUTTON,
                    crate::RegionKind::CloseButton => HTCLOSE,
                };
                return LRESULT(hit.0 as isize);
            }
        }
    }

    // 调用原始窗口过程
    match original_proc {
        Some(proc) => CallWindowProcW(Some(proc), hwnd, msg, wparam, lparam),
        None => LRESULT(0),
    }
}

