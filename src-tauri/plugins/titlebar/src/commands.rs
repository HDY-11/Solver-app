//! commands.rs — 前端可调用的 Tauri 命令
//!
//! | 命令 | 用途 |
//! |------|------|
//! | `update_regions` | 更新当前窗口的自定义标题栏区域（前端 ResizeObserver 触发） |
//! | `register_detached` | 将当前窗口标记为分离窗口，启用拖拽合并检测 |

use serde::Deserialize;

/// 前端传入的自定义标题栏区域描述
///
/// 通过 `ResizeObserver` 实时上报各按钮的像素坐标，
/// 供子类过程在 `WM_NCHITTEST` 中做命中测试。
#[derive(Debug, Clone, Deserialize)]
pub struct TitlebarRegion {
    /// 区域类型：`"maxbutton"` / `"minbutton"` / `"closebutton"` / 其他（视为拖拽区）
    pub kind: String,
    /// 相对于窗口客户区左上角的 X 坐标（物理像素）
    pub x: i32,
    /// 相对于窗口客户区左上角的 Y 坐标（物理像素）
    pub y: i32,
    /// 区域宽度（物理像素）
    pub width: i32,
    /// 区域高度（物理像素）
    pub height: i32,
}

/// 更新当前窗口的自定义标题栏区域
///
/// 前端应在窗口大小变化时（ResizeObserver）调用此命令，
/// 确保命中测试始终使用最新的按钮位置。
#[tauri::command]
pub fn update_regions(hwnd_raw: String, regions: Vec<TitlebarRegion>) {
    #[cfg(target_os = "windows")]
    if let Ok(raw) = hwnd_raw.parse::<isize>() {
        crate::windows_impl::set_regions(raw, regions);
    }
}

/// 将当前窗口注册为「分离窗口」，启用拖拽合并检测
///
/// 分离窗口挂载后应立即调用。该命令会：
/// 1. 对该窗口安装子类化（若尚未安装）
/// 2. 标记 `is_detached = true`，使 `WM_EXITSIZEMOVE` 触发合并判断
///
/// # 前端调用示例
/// ```typescript
/// import { getCurrentWindow } from '@tauri-apps/api/window';
/// if (getCurrentWindow().label.startsWith('detached-')) {
///   invoke('register_detached');
/// }
/// ```
#[tauri::command]
pub fn register_detached(window: tauri::Window) {
    log::info!("[titlebar] register_detached: label={}", window.label());
    #[cfg(target_os = "windows")]
    if let Ok(hwnd) = window.hwnd() {
        let hwnd_raw = hwnd.0 as isize;
        unsafe { crate::windows_impl::register_detached(hwnd_raw); }
    }
}
