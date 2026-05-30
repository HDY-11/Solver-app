// titlebar-system — 自定义标题栏系统
//
// 通过 Windows 窗口子类化（SetWindowLongPtrW）拦截 WM_NCHITTEST，
// 使自定义 UI 区域被 Windows 识别为原生标题栏元素。
// 这使得：
// - 自定义"最大化"按钮区域可触发 Windows 11 贴靠布局弹出菜单
// - 自定义"关闭"/"最小化"按钮区域获得原生行为
// - 标题栏空白区域可拖拽
//
// 同时提供全局鼠标钩子（WH_MOUSE_LL）支持跨窗口拖拽检测。

pub mod subclass;
pub mod drag_tracker;

use windows::Win32::Foundation::HWND;

/// 窗口自定义区域配置（从 JS 端通过 Tauri 命令传入）
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TitlebarRegion {
    /// 区域类型
    pub kind: RegionKind,
    /// 相对于窗口左上角的像素矩形
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RegionKind {
    Caption,
    MaxButton,
    MinButton,
    CloseButton,
}

/// 初始化标题栏系统：对指定窗口进行子类化
///
/// # Safety
/// 必须在 Windows 平台上调用，且 hwnd 必须有效
pub unsafe fn init(hwnd: HWND) -> Result<(), String> {
    subclass::install(hwnd)
}

/// 更新自定义区域配置（JS 端布局变化时调用）
pub fn update_regions(hwnd: HWND, regions: Vec<TitlebarRegion>) {
    subclass::set_regions(hwnd, regions);
}

/// 开始全局拖拽追踪（标签拖拽时调用）
pub fn start_drag_track(tab_path: String, tab_label: String) {
    drag_tracker::start(tab_path, tab_label);
}

/// 停止全局拖拽追踪
pub fn stop_drag_track() {
    drag_tracker::stop();
}
