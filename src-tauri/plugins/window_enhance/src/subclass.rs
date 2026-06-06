//! subclass.rs — 窗口子类化（无副作用函数 + 类型特化窗口过程）
//!
//! ## 职责
//! - [`get_original_proc`]：获取目标窗口的原始窗口过程地址
//! - [`handle_nchittest`]：**纯函数** — 给定区域列表和鼠标坐标，返回命中测试结果
//! - [`main_window_proc`]：主窗口专用过程 — 仅处理 `WM_NCHITTEST`
//! - [`detached_window_proc`]：分离窗口专用过程 — 处理 `WM_NCHITTEST` + 拖拽合并检测
//!
//! ## 设计原则
//!
//! 不同类型的窗口使用**不同的窗口过程**，而非在一个过程中用标记位分支。
//! 窗口过程在 [`WindowManager::register`](super::manager::WindowManager::register)
//! 的 hook 闭包中通过 match [`WindowKind`](super::state::WindowKind) 选择安装。
//!
//! 拖拽合并检测不是独立模块——它是 `detached_window_proc` 的内聚行为。

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, POINT, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW, GetClientRect, GetCursorPos,
    GetWindowLongPtrW,
    GWLP_WNDPROC, HTCAPTION, HTCLOSE, HTMAXBUTTON, HTMINBUTTON,
    WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_NCHITTEST,
};

use event_system::emit;

use super::manager::WindowManager;

// ScreenToClient 在 windows crate 0.62 中需要 Win32_Graphics_Gdi feature，
// 为避免额外依赖，直接通过 FFI 声明（link to user32.dll）。
#[cfg(target_os = "windows")]
unsafe extern "system" {
    unsafe fn ScreenToClient(hWnd: HWND, lpPoint: *mut POINT) -> i32;
}

// ═══════════════════════════════════════════════════════════════════
// 常量
// ═══════════════════════════════════════════════════════════════════

/// Nav 区域在窗口客户区中的 CSS 像素范围（从窗口顶部算起）
const NAV_TOP_CSS_PX: f64 = 60.0;
const NAV_BOTTOM_CSS_PX: f64 = 124.0;

// ═══════════════════════════════════════════════════════════════════
// 子类化安装
// ═══════════════════════════════════════════════════════════════════

/// 获取目标窗口的原始窗口过程地址。
///
/// 此函数仅调用 `GetWindowLongPtrW(hwnd, GWLP_WNDPROC)`，
/// 不执行 `SetWindowLongPtrW`（子类化安装由 [`WindowManager::register`] 在锁内完成）。
///
/// 返回原始窗口过程地址，用于消息转发。
pub unsafe fn get_original_proc(hwnd: HWND) -> isize {
    GetWindowLongPtrW(hwnd, GWLP_WNDPROC)
}

// ═══════════════════════════════════════════════════════════════════
// 命中测试 handler（纯函数，两种窗口过程共享）
// ═══════════════════════════════════════════════════════════════════

/// 处理 `WM_NCHITTEST` — **纯函数**。
///
/// 不访问任何全局状态。调用方需要传入区域列表和鼠标坐标。
pub fn handle_nchittest(
    lparam: LPARAM,
    regions: &[(i32, i32, i32, i32, String)],
) -> Option<LRESULT> {
    if regions.is_empty() {
        return None;
    }

    let mx = ((lparam.0 as u64) & 0xFFFF) as i32;
    let my = (((lparam.0 as u64) >> 16) & 0xFFFF) as i32;

    for (rx, ry, rw, rh, ref kind) in regions {
        if mx >= *rx && mx <= *rx + *rw && my >= *ry && my <= *ry + *rh {
            let hit = match kind.as_str() {
                "maxbutton" => HTMAXBUTTON,
                "minbutton" => HTMINBUTTON,
                "closebutton" => HTCLOSE,
                _ => HTCAPTION,
            };
            return Some(LRESULT(hit as isize));
        }
    }

    None
}

// ═══════════════════════════════════════════════════════════════════
// main_window_proc — 主窗口专用
// ═══════════════════════════════════════════════════════════════════

/// 主窗口的窗口过程。
///
/// 仅处理 `WM_NCHITTEST`（自定义标题栏按钮命中测试）。
/// 其他消息全部转发给原始窗口过程。
pub(crate) unsafe extern "system" fn main_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let hwnd_raw = hwnd.0 as isize;
    let manager = WindowManager::global();

    // 获取状态快照（try_lock 防嵌套消息死锁）
    let (original_proc, regions) = match manager.try_lock_windows() {
        Some(windows) => windows
            .get(&hwnd_raw)
            .map(|s| (s.original_proc, s.regions.clone()))
            .unwrap_or((0, Vec::new())),
        None => (0, Vec::new()),
    };

    // 仅处理命中测试
    if msg == WM_NCHITTEST {
        if let Some(result) = handle_nchittest(lparam, &regions) {
            return result;
        }
    }

    // 转发
    forward_message(original_proc, hwnd, msg, wparam, lparam)
}

// ═══════════════════════════════════════════════════════════════════
// detached_window_proc — 分离窗口专用
// ═══════════════════════════════════════════════════════════════════

/// 分离窗口的窗口过程。
///
/// 处理两类消息：
/// 1. `WM_NCHITTEST` — 自定义标题栏按钮命中测试（同主窗口）
/// 2. `WM_ENTERSIZEMOVE` / `WM_EXITSIZEMOVE` — 拖拽合并检测：
///    拖拽结束时检查光标是否落在主窗口 Nav 区域，若是则发射 `drag-release` 事件。
pub(crate) unsafe extern "system" fn detached_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let hwnd_raw = hwnd.0 as isize;
    let manager = WindowManager::global();

    // 获取状态快照
    let (original_proc, regions) = match manager.try_lock_windows() {
        Some(windows) => windows
            .get(&hwnd_raw)
            .map(|s| (s.original_proc, s.regions.clone()))
            .unwrap_or((0, Vec::new())),
        None => (0, Vec::new()),
    };

    // 1) 命中测试（纯函数）
    if msg == WM_NCHITTEST {
        if let Some(result) = handle_nchittest(lparam, &regions) {
            return result;
        }
    }

    // 2) 拖拽合并检测（分离窗口内聚行为）
    if msg == WM_ENTERSIZEMOVE {
        log::debug!("[window_enhance] 分离窗口开始移动 0x{:x}", hwnd_raw);
    }

    if msg == WM_EXITSIZEMOVE {
        log::debug!("[window_enhance] 分离窗口结束移动 0x{:x}", hwnd_raw);
        try_merge_on_drag_end(manager);
    }

    // 转发
    forward_message(original_proc, hwnd, msg, wparam, lparam)
}

// ═══════════════════════════════════════════════════════════════════
// 消息转发
// ═══════════════════════════════════════════════════════════════════

/// 将消息转发给原始窗口过程或默认过程。
unsafe fn forward_message(
    original_proc: isize,
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if original_proc != 0 {
        let proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT =
            std::mem::transmute(original_proc);
        CallWindowProcW(Some(proc), hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

// ═══════════════════════════════════════════════════════════════════
// 拖拽合并检测（detached_window_proc 的内聚行为）
// ═══════════════════════════════════════════════════════════════════

/// 拖拽结束后尝试触发合并。
///
/// 执行步骤：
/// 1. 获取当前光标屏幕坐标
/// 2. 从已注册窗口中扫描 [`WindowKind::Main`] 获取主窗口 HWND
/// 3. 获取 DPI 缩放比
/// 4. 将光标转换为主窗口客户区坐标
/// 5. 获取主窗口客户区尺寸
/// 6. 调用 [`check_cursor_in_nav`] 纯函数做命中测试
/// 7. 若命中 → 发射 `drag-release` 事件
fn try_merge_on_drag_end(manager: &WindowManager) {
    // 1. 获取光标屏幕坐标
    let mut cursor_pt = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut cursor_pt);
    }

    // 2. 扫描已注册窗口，找到主窗口 HWND
    let main_hwnd_raw = manager.find_main_hwnd();
    if main_hwnd_raw == 0 {
        log::warn!("[window_enhance] 未找到已注册的主窗口，跳过合并检测");
        return;
    }
    let main_hwnd = HWND(main_hwnd_raw as *mut _);

    // 3. 获取 DPI 缩放比
    let scale = manager.dpr();

    // 4. 将光标从屏幕坐标转换为主窗口客户区坐标
    unsafe {
        let _ = ScreenToClient(main_hwnd, &mut cursor_pt);
    }

    // 5. 获取主窗口客户区尺寸
    let mut client_rect = RECT::default();
    unsafe {
        let _ = GetClientRect(main_hwnd, &mut client_rect);
    }

    // 6. 纯函数命中测试
    let cursor_in_nav = check_cursor_in_nav(
        cursor_pt.x,
        cursor_pt.y,
        client_rect.right,
        scale,
    );

    log::debug!(
        "[window_enhance] 合并检测: scale={:.2} client_size=({},{}) nav_y=({},{}) cursor_client=({},{}) hit={}",
        scale, client_rect.right, client_rect.bottom,
        NAV_TOP_CSS_PX * scale, NAV_BOTTOM_CSS_PX * scale,
        cursor_pt.x, cursor_pt.y, cursor_in_nav,
    );

    if cursor_in_nav {
        log::info!("[window_enhance] 光标在主窗口 Nav 区域，发射 drag-release 事件");
        let payload = serde_json::json!({
            "screenX": cursor_pt.x,
            "screenY": cursor_pt.y,
        });
        emit!(dyn "drag-release": payload);
    }
}

/// 判断客户区坐标是否落在主窗口的 Nav 区域内 — **纯函数**。
///
/// 所有参数均显式传入，不依赖任何全局状态。
fn check_cursor_in_nav(
    cursor_x: i32,
    cursor_y: i32,
    client_width: i32,
    scale: f64,
) -> bool {
    let nav_top = (NAV_TOP_CSS_PX * scale) as i32;
    let nav_bottom = (NAV_BOTTOM_CSS_PX * scale) as i32;

    cursor_x >= 0
        && cursor_x <= client_width
        && cursor_y >= nav_top
        && cursor_y <= nav_bottom
}

// ═══════════════════════════════════════════════════════════════════
// 测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::{HTCAPTION, HTCLOSE, HTMAXBUTTON};

    #[test]
    fn nchittest_empty_regions_returns_none() {
        let regions: Vec<(i32, i32, i32, i32, String)> = vec![];
        let lparam = LPARAM((100i64 << 32) as isize | 50isize);
        assert!(handle_nchittest(lparam, &regions).is_none());
    }

    #[test]
    fn nchittest_hits_maxbutton() {
        let regions = vec![(10, 0, 40, 30, "maxbutton".into())];
        let lparam = LPARAM(((15i64) << 32) as isize | 30isize);
        let result = handle_nchittest(lparam, &regions);
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, HTMAXBUTTON as isize);
    }

    #[test]
    fn nchittest_hits_closebutton() {
        let regions = vec![(50, 0, 40, 30, "closebutton".into())];
        let lparam = LPARAM(((15i64) << 32) as isize | 70isize);
        let result = handle_nchittest(lparam, &regions);
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, HTCLOSE as isize);
    }

    #[test]
    fn nchittest_hits_caption_for_unknown_kind() {
        let regions = vec![(0, 0, 100, 30, "dragzone".into())];
        let lparam = LPARAM(((15i64) << 32) as isize | 50isize);
        let result = handle_nchittest(lparam, &regions);
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, HTCAPTION as isize);
    }

    #[test]
    fn nchittest_miss_returns_none() {
        let regions = vec![(10, 0, 40, 30, "maxbutton".into())];
        let lparam = LPARAM(((100i64) << 32) as isize | 100isize);
        assert!(handle_nchittest(lparam, &regions).is_none());
    }

    #[test]
    fn cursor_inside_nav_returns_true() {
        assert!(check_cursor_in_nav(50, 80, 800, 1.0));
    }

    #[test]
    fn cursor_below_nav_returns_false() {
        assert!(!check_cursor_in_nav(50, 200, 800, 1.0));
    }

    #[test]
    fn cursor_above_nav_returns_false() {
        assert!(!check_cursor_in_nav(50, 30, 800, 1.0));
    }

    #[test]
    fn nav_scales_with_dpr() {
        // DPR=2.0, nav 区域 y ∈ [120, 248]
        assert!(check_cursor_in_nav(50, 150, 800, 2.0));
        assert!(!check_cursor_in_nav(50, 100, 800, 2.0));
    }

    #[test]
    fn cursor_at_nav_boundary() {
        assert!(check_cursor_in_nav(0, 60, 800, 1.0));
        assert!(check_cursor_in_nav(0, 124, 800, 1.0));
    }
}
