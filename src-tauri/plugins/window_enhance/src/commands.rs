//! commands.rs — 窗口增强能力的公共 API
//!
//! ## 职责
//!
//! 本模块导出**纯公共函数**（非 Tauri 命令），供上层业务代码调用。
//! 每个函数封装对 [`WindowManager`](super::manager::WindowManager) 门面的委托，
//! 并处理 HWND 提取等平台细节。
//!
//! ## 与 Tauri 命令的关系
//!
//! 插件**只提供能力，不绑定业务**。
//! 业务逻辑（如命令名称、调用时机、错误处理策略）由上层
//!（`src-tauri/src/lib.rs` 中的 `#[command]`）定义。
//!
//! | 公共函数 | 封装的能力 |
//! |----------|-----------|
//! | `update_regions` | 更新窗口的自定义标题栏命中区域 |
//! | `register_detached` | 将窗口注册为可合并的分离窗口 |
//! | `set_device_pixel_ratio` | 设置 DPR 用于坐标转换 |

use serde::Deserialize;

#[cfg(target_os = "windows")]
use super::manager::WindowManager;
#[cfg(target_os = "windows")]
use super::state::WindowKind;

// ═══════════════════════════════════════════════════════════════════
// 数据类型
// ═══════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════
// 公共 API（非 Tauri 命令，纯能力委托）
// ═══════════════════════════════════════════════════════════════════

/// 更新指定窗口的自定义标题栏区域。
///
/// 前端应在窗口大小变化时（ResizeObserver）通过 Tauri 命令间接调用。
/// 此函数仅做能力委托：提取 HWND → 转换坐标 → 存入 WindowManager。
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

/// 泛型窗口注册。
///
/// 通过 [`WindowManager::register`] 统一注册，
/// [`WindowKind`] 决定安装哪个窗口过程。
pub fn register(hwnd_raw: isize, kind: WindowKind) {
    #[cfg(target_os = "windows")]
    WindowManager::global().register(hwnd_raw, kind);
    #[cfg(not(target_os = "windows"))]
    let _ = (hwnd_raw, kind);
}

/// 设置设备像素比。
///
/// 前端应在获取到 `window.devicePixelRatio` 后通过 Tauri 命令间接调用。
pub fn set_dpr(dpr: f64) {
    #[cfg(target_os = "windows")]
    WindowManager::global().set_dpr(dpr);
    #[cfg(not(target_os = "windows"))]
    let _ = dpr;
}

