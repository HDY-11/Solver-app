//! # tauri-plugin-titlebar — 自定义标题栏系统
//!
//! ## 功能
//! - **窗口子类化**：拦截 `WM_NCHITTEST`，将前端自定义 UI 区域映射为原生标题栏按钮
//!   （最大化/最小化/关闭），同时支持贴靠布局弹出菜单。
//! - **拖拽合并检测**：分离窗口拖拽结束时，通过 `WM_EXITSIZEMOVE` 判断光标是否
//!   落在主窗口 Nav 区域，若是则发射 `drag-release` 事件触发合并。
//!
//! ## 拖拽合并流程
//! ```text
//! 1. 分离窗口挂载 → 前端调用 register_detached() 标记该 HWND 为可合并窗口
//! 2. 用户拖拽标题栏 → Windows 进入模态移动循环 → WM_ENTERSIZEMOVE
//! 3. 用户松开鼠标   → Windows 退出模态移动循环 → WM_EXITSIZEMOVE
//! 4. 子类过程检测到 detached 窗口的 WM_EXITSIZEMOVE：
//!    a. 获取当前光标位置（GetCursorPos）
//!    b. 获取主窗口矩形（GetWindowRect）
//!    c. 计算 Nav 区域（窗口顶部约 60~124 CSS px 范围，按 DPI 缩放）
//!    d. 若光标在 Nav 区域内 → emit!("drag-release") → 前端发起合并
//! ```

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager,
};

pub mod commands;
#[cfg(target_os = "windows")]
pub mod windows_impl;

/// 初始化 titlebar 插件
///
/// 在 `setup` 阶段对主窗口安装子类化，之后前端可通过
/// `update_regions` 动态更新自定义标题栏区域，
/// 分离窗口通过 `register_detached` 注册后可触发拖拽合并。
pub fn init() -> TauriPlugin<tauri::Wry> {
    Builder::new("titlebar")
        .invoke_handler(tauri::generate_handler![
            commands::update_regions,
            commands::register_detached,
        ])
        .setup(|app, _api| {
            #[cfg(target_os = "windows")]
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(hwnd) = window.hwnd() {
                    let hwnd_raw = hwnd.0 as isize;
                    unsafe { windows_impl::install_subclass(hwnd_raw); }
                }
            }
            Ok(())
        })
        .build()
}
