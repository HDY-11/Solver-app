// commands.rs — 前端可调用的 Tauri 命令

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct TitlebarRegion {
    pub kind: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// 更新当前窗口的自定义标题栏区域（前端 ResizeObserver 触发）
#[tauri::command]
pub fn update_regions(hwnd_raw: String, regions: Vec<TitlebarRegion>) {
    #[cfg(target_os = "windows")]
    if let Ok(raw) = hwnd_raw.parse::<isize>() {
        crate::windows_impl::set_regions(raw, regions);
    }
}

/// 启动全局拖拽追踪（标签拖拽开始）
#[tauri::command]
pub fn start_drag_track(tab_path: String, tab_label: String, device_pixel_ratio: f64) {
    eprintln!("[titlebar] start_drag_track: path={}, label={}, dpr={}", tab_path, tab_label, device_pixel_ratio);
    #[cfg(target_os = "windows")]
    crate::windows_impl::start_drag(tab_path, tab_label, device_pixel_ratio);
}

/// 停止全局拖拽追踪
#[tauri::command]
pub fn stop_drag_track() {
    eprintln!("[titlebar] stop_drag_track");
    #[cfg(target_os = "windows")]
    crate::windows_impl::stop_drag();
}
