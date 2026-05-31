// tauri-plugin-titlebar — 自定义标题栏系统
//
// 通过窗口子类化拦截 WM_NCHITTEST，使自定义 UI 按钮
// 被 Windows 识别为原生标题栏元素，触发贴靠布局弹出菜单。
// 同时提供全局鼠标钩子支持跨窗口拖拽检测。

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager,
};

pub mod commands;
#[cfg(target_os = "windows")]
pub mod windows_impl;

pub fn init() -> TauriPlugin<tauri::Wry> {
    Builder::new("titlebar")
        .invoke_handler(tauri::generate_handler![
            commands::update_regions,
            commands::start_drag_track,
            commands::stop_drag_track,
        ])
        .setup(|app, _api| {
            #[cfg(target_os = "windows")]
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(hwnd) = window.hwnd() {
                    let hwnd_raw = hwnd.0 as isize;
                    unsafe { windows_impl::install_subclass(hwnd_raw); }
                    log::info!("[titlebar] 子类化已安装: 0x{:x}", hwnd_raw);
                }
            }
            Ok(())
        })
        .build()
}
