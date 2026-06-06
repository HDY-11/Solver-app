//! platform.rs — 平台抽象层
//!
//! ## 架构角色
//!
//! 封装平台相关的系统调用（`GetCursorPos`、`ScreenToClient`、`GetClientRect`），
//! 提供平台无关的函数签名。上层 trait 实现者通过此模块获取窗口上下文信息，
//! 无需直接依赖 `windows` crate。
//!
//! ## 扩展
//!
//! 新增平台时，在此文件添加对应的 `#[cfg]` 模块即可，不影响其他层。

// ═══════════════════════════════════════════════════════════════════
// 平台无关类型
// ═══════════════════════════════════════════════════════════════════

/// 屏幕坐标点（物理像素）
#[derive(Debug, Clone, Copy, Default)]
pub struct ScreenPoint {
    pub x: i32,
    pub y: i32,
}

/// 矩形尺寸（物理像素）
#[derive(Debug, Clone, Copy, Default)]
pub struct RectSize {
    pub width: i32,
    pub height: i32,
}

// ═══════════════════════════════════════════════════════════════════
// Windows 平台实现
// ═══════════════════════════════════════════════════════════════════

#[cfg(target_os = "windows")]
mod windows_impl {
    use windows::Win32::Foundation::{HWND, POINT, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, GetCursorPos};

    // 直接链接 user32.dll，避免引入 Win32_Graphics_Gdi feature
    unsafe extern "system" {
        unsafe fn ScreenToClient(hWnd: HWND, lpPoint: *mut POINT) -> i32;
    }

    /// 获取当前光标屏幕坐标。
    ///
    /// 包装 `GetCursorPos`。失败时返回 `None`。
    pub fn cursor_position() -> Option<super::ScreenPoint> {
        let mut pt = POINT::default();
        let ok = unsafe { GetCursorPos(&mut pt) };
        if ok.is_ok() {
            Some(super::ScreenPoint { x: pt.x, y: pt.y })
        } else {
            None
        }
    }

    /// 将屏幕坐标转换为指定窗口的客户区坐标。
    ///
    /// 包装 `ScreenToClient`。即使转换失败，坐标保持不变（安全降级）。
    pub fn screen_to_client(hwnd_raw: isize, point: super::ScreenPoint) -> super::ScreenPoint {
        let mut pt = POINT { x: point.x, y: point.y };
        unsafe {
            let _ = ScreenToClient(HWND(hwnd_raw as *mut _), &mut pt);
        }
        super::ScreenPoint { x: pt.x, y: pt.y }
    }

    /// 获取窗口客户区尺寸（物理像素）。
    ///
    /// 包装 `GetClientRect`。
    pub fn client_rect(hwnd_raw: isize) -> super::RectSize {
        let mut rect = RECT::default();
        unsafe {
            let _ = GetClientRect(HWND(hwnd_raw as *mut _), &mut rect);
        }
        super::RectSize {
            width: rect.right,
            height: rect.bottom,
        }
    }

    /// 从 `WM_NCHITTEST` 的 `lParam` 中提取屏幕坐标。
    ///
    /// `lParam` 低 16 位 = X，高 16 位 = Y。
    /// 使用 `u64` 中间类型避免 `isize` 符号扩展问题。
    pub fn unpack_nchittest_lparam(lparam: isize) -> super::ScreenPoint {
        let lp = lparam as u64;
        super::ScreenPoint {
            x: (lp & 0xFFFF) as i32,
            y: ((lp >> 16) & 0xFFFF) as i32,
        }
    }
}

#[cfg(target_os = "windows")]
pub use windows_impl::*;

// ═══════════════════════════════════════════════════════════════════
// 非 Windows 平台 — no-op 占位
// ═══════════════════════════════════════════════════════════════════

#[cfg(not(target_os = "windows"))]
mod noop_impl {
    use super::{RectSize, ScreenPoint};

    pub fn cursor_position() -> Option<ScreenPoint> {
        None
    }

    pub fn screen_to_client(_hwnd_raw: isize, point: ScreenPoint) -> ScreenPoint {
        point
    }

    pub fn client_rect(_hwnd_raw: isize) -> RectSize {
        RectSize::default()
    }

    pub fn unpack_nchittest_lparam(_lparam: isize) -> ScreenPoint {
        ScreenPoint::default()
    }
}

#[cfg(not(target_os = "windows"))]
pub use noop_impl::*;
