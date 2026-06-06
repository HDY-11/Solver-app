//! commands.rs — 窗口增强能力的公共 API
//!
//! ## 职责
//!
//! 本模块导出**纯公共函数**（非 Tauri 命令），供上层业务代码调用。
//! 每个函数封装对 [`WindowManager`] 门面的委托。
//!
//! ## 与 Tauri 命令的关系
//!
//! 插件**只提供能力，不绑定业务**。
//! 业务逻辑（如命令名称、调用时机、错误处理策略）由上层定义。

use serde::Deserialize;

use crate::behaviors::HookBehaviors;
use crate::window_behavior::WindowBehavior;

#[cfg(target_os = "windows")]
use crate::manager::WindowManager;

// ═══════════════════════════════════════════════════════════════════
// 数据类型
// ═══════════════════════════════════════════════════════════════════

/// 前端传入的自定义标题栏区域描述。
///
/// 通过 `ResizeObserver` 实时上报各按钮的物理像素坐标，
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

// ═══════════════════════════════════════════════════════════════════
// 公共 API
// ═══════════════════════════════════════════════════════════════════

/// 更新指定窗口的自定义标题栏命中区域。
pub fn update_regions(hwnd_raw: isize, regions: Vec<TitlebarRegion>) {
    #[cfg(target_os = "windows")]
    {
        let converted: Vec<_> = regions
            .into_iter()
            .map(|r| (r.x, r.y, r.width, r.height, r.kind))
            .collect();
        WindowManager::global().update_regions(hwnd_raw, converted);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = (hwnd_raw, regions);
}

/// Hook 注入入口：注册消息处理器到窗口消息链。
///
/// 替代旧的 `register(hwnd_raw, WindowKind)` 接口。
/// 上层通过 `behaviors` 声明兴趣，通过 `behavior` 注入处理器。
pub fn register(hwnd_raw: isize, behaviors: HookBehaviors, behavior: Box<dyn WindowBehavior>) {
    #[cfg(target_os = "windows")]
    WindowManager::global().register(hwnd_raw, behaviors, behavior);
    #[cfg(not(target_os = "windows"))]
    let _ = (hwnd_raw, behaviors, behavior);
}

/// 设置设备像素比。
pub fn set_dpr(dpr: f64) {
    #[cfg(target_os = "windows")]
    WindowManager::global().set_dpr(dpr);
    #[cfg(not(target_os = "windows"))]
    let _ = dpr;
}

