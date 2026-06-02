//! windows_impl.rs — Windows 平台特定实现
//!
//! ## 架构概述
//!
//! 本模块通过窗口子类化（Window Subclassing）拦截目标窗口的 Windows 消息，
//! 实现以下两个核心功能：
//!
//! 1. **自定义标题栏命中测试**（`WM_NCHITTEST`）
//!    前端通过 `update_regions` 上报按钮区域的物理像素坐标，
//!    子类过程据此将鼠标点击映射为 `HTMAXBUTTON` / `HTMINBUTTON` / `HTCLOSE` / `HTCAPTION`。
//!
//! 2. **拖拽合并检测**（`WM_ENTERSIZEMOVE` / `WM_EXITSIZEMOVE`）
//!    当分离窗口被用户拖拽时，Windows 进入模态移动循环。
//!    `WM_ENTERSIZEMOVE` 标记移动开始，`WM_EXITSIZEMOVE` 标记移动结束。
//!    结束时检查光标是否落在主窗口的 Nav 区域，若是则发射 `drag-release` 事件。
//!
//! ## 全局状态封装
//!
//! 所有跨窗口共享的状态封装在 [`DragContext`] 中，通过 `LazyLock<Mutex<>>` 管理。
//! 不再使用全局鼠标钩子（`WH_MOUSE_LL`），避免了独立的钩子线程和 `eprintln!` 噪音。

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use tauri::Manager;
use event_system::{emit, GLOBAL_APPHANDLE};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, POINT, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW, GetClientRect, GetCursorPos, GetWindowLongPtrW,
    SetWindowLongPtrW,
    GWLP_WNDPROC, HTCAPTION, HTCLOSE, HTMAXBUTTON, HTMINBUTTON,
    WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_NCHITTEST,
};

// ScreenToClient 在 windows crate 0.62 中需要 Win32_Graphics_Gdi feature，
// 为避免额外依赖，直接通过 FFI 声明（link to user32.dll）。
// Windows BOOL ≡ i32，返回 0 表示失败。
#[cfg(target_os = "windows")]
extern "system" {
    fn ScreenToClient(hWnd: HWND, lpPoint: *mut POINT) -> i32;
}

use crate::commands::TitlebarRegion;

// ═══════════════════════════════════════════════════════════════════
// 全局上下文
// ═══════════════════════════════════════════════════════════════════

/// 拖拽/标题栏系统的全局状态容器
///
/// 所有可变状态封装于此，通过 `LazyLock<Mutex<>>` 提供线程安全的内部可变性。
/// 锁竞争极低：子类过程使用 `try_lock` 避免嵌套消息死锁，
/// Tauri 命令在独立线程中调用 `lock`。
static CONTEXT: LazyLock<Mutex<DragContext>> =
    LazyLock::new(|| Mutex::new(DragContext::new()));

/// 全局拖拽上下文
///
/// 管理所有被子类化的窗口状态、主窗口引用、以及 DPI 缩放信息。
struct DragContext {
    /// 每个被子类化窗口的独立状态，以 `hwnd_raw` 为键
    window_states: HashMap<isize, WindowState>,
    /// 主窗口 HWND（用于 Nav 区域命中测试）
    main_hwnd: Option<isize>,
    /// 前端传入的设备像素比（devicePixelRatio），用于 CSS px → 物理 px 坐标转换
    device_pixel_ratio: f64,
}

impl DragContext {
    fn new() -> Self {
        Self {
            window_states: HashMap::new(),
            main_hwnd: None,
            device_pixel_ratio: 1.0,
        }
    }
}

/// 单个窗口的子类化状态
#[derive(Clone)]
struct WindowState {
    /// 原始窗口过程地址（用于 `CallWindowProcW` 转发未处理消息）
    original_proc: isize,
    /// 自定义标题栏区域列表：`(x, y, width, height, kind)`
    regions: Vec<(i32, i32, i32, i32, String)>,
    /// 是否为分离窗口（可触发拖拽合并）
    is_detached: bool,
}

// ═══════════════════════════════════════════════════════════════════
// 全局上下文访问辅助
// ═══════════════════════════════════════════════════════════════════

/// 安全地获取 CONTEXT 锁（处理 PoisonError 降级）
///
/// 当 Mutex 被 poison 时，记录 error 日志并使用 `into_inner` 恢复数据继续运行。
/// 这确保了即使某个线程 panic，系统仍能以降级状态工作。
fn lock_context() -> std::sync::MutexGuard<'static, DragContext> {
    CONTEXT.lock().unwrap_or_else(|poison| {
        log::error!("[titlebar] CONTEXT Mutex 已 poison，使用降级状态继续运行");
        poison.into_inner()
    })
}

/// 尝试获取 CONTEXT 锁（非阻塞，处理 PoisonError）
///
/// 用于子类过程等不可阻塞的上下文中。若锁已被持有或 poison，
/// 返回 `None`，调用方应跳过本次处理并转发消息。
fn try_lock_context() -> Option<std::sync::MutexGuard<'static, DragContext>> {
    CONTEXT.try_lock().ok().or_else(|| {
        // try_lock 在 poison 时也返回 Err，记录并降级
        log::warn!("[titlebar] CONTEXT try_lock 失败（锁竞争或 poison），跳过本次消息处理");
        None
    })
}

// ═══════════════════════════════════════════════════════════════════
// 公共 API
// ═══════════════════════════════════════════════════════════════════

/// 对主窗口安装子类化
///
/// # Safety
/// 必须在 Windows 平台上调用，且 `hwnd_raw` 必须为有效窗口句柄。
/// 仅在插件 `setup` 阶段对主窗口调用一次。
pub unsafe fn install_subclass(hwnd_raw: isize) {
    let hwnd = HWND(hwnd_raw as *mut _);
    let original = GetWindowLongPtrW(hwnd, GWLP_WNDPROC);

    let mut ctx = lock_context();
    ctx.main_hwnd = Some(hwnd_raw);
    ctx.window_states.insert(
        hwnd_raw,
        WindowState {
            original_proc: original,
            regions: Vec::new(),
            is_detached: false,
        },
    );

    SetWindowLongPtrW(hwnd, GWLP_WNDPROC, subclass_proc_addr());
    log::info!("[titlebar] 主窗口子类化已安装: 0x{:x}", hwnd_raw);
}

/// 将指定窗口注册为分离窗口，并安装子类化（若尚未安装）
///
/// 分离窗口的 `WM_EXITSIZEMOVE` 将触发合并检测。
///
/// # Safety
/// 必须在 Windows 平台上调用，且 `hwnd_raw` 必须有效。
pub unsafe fn register_detached(hwnd_raw: isize) {
    let hwnd = HWND(hwnd_raw as *mut _);

    let mut ctx = lock_context();

    // 若尚未子类化，则安装
    if !ctx.window_states.contains_key(&hwnd_raw) {
        let original = GetWindowLongPtrW(hwnd, GWLP_WNDPROC);
        ctx.window_states.insert(
            hwnd_raw,
            WindowState {
                original_proc: original,
                regions: Vec::new(),
                is_detached: true,
            },
        );
        SetWindowLongPtrW(hwnd, GWLP_WNDPROC, subclass_proc_addr());
        log::info!("[titlebar] 分离窗口子类化已安装: 0x{:x}", hwnd_raw);
    } else {
        // 已子类化（可能是主窗口），仅标记 detached
        if let Some(state) = ctx.window_states.get_mut(&hwnd_raw) {
            state.is_detached = true;
            log::info!("[titlebar] 窗口已标记为分离: 0x{:x}", hwnd_raw);
        }
    }
}

/// 更新指定窗口的自定义标题栏区域
///
/// 前端应在窗口大小改变时通过 `update_regions` 命令调用。
pub fn set_regions(hwnd_raw: isize, regions: Vec<TitlebarRegion>) {
    let converted: Vec<_> = regions
        .into_iter()
        .map(|r| (r.x, r.y, r.width, r.height, r.kind))
        .collect();

    let mut ctx = lock_context();
    if let Some(state) = ctx.window_states.get_mut(&hwnd_raw) {
        state.regions = converted;
    }
}

/// 更新 DPI 缩放比（由前端在拖拽开始时传入，或通过 ResizeObserver 动态更新）
#[allow(dead_code)]
fn update_dpi_scale(ratio: f64) {
    if ratio > 0.0 {
        lock_context().device_pixel_ratio = ratio;
    }
}

// ═══════════════════════════════════════════════════════════════════
// 子类过程
// ═══════════════════════════════════════════════════════════════════

/// 获取子类过程函数指针（用于 `SetWindowLongPtrW`）
fn subclass_proc_addr() -> isize {
    let fp: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT = subclass_proc;
    fp as usize as isize
}

/// 窗口子类回调过程
///
/// 拦截以下消息：
/// - `WM_NCHITTEST`：自定义标题栏区域命中测试
/// - `WM_ENTERSIZEMOVE`：窗口开始移动/缩放 → 标记 `is_moving`
/// - `WM_EXITSIZEMOVE`：窗口结束移动/缩放 → 若为分离窗口，检查是否触发合并
///
/// 其它消息全部转发给原始窗口过程。
unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let hwnd_raw = hwnd.0 as isize;

    // 使用 try_lock 防止嵌套消息死锁：
    // 若锁被持有（CallWindowProcW 嵌套回调中），返回 None 并跳过。
    let (original, regions, is_detached) = match try_lock_context() {
        Some(ctx) => {
            let state = ctx.window_states.get(&hwnd_raw).cloned();
            state
                .map(|s| (s.original_proc, s.regions, s.is_detached))
                .unwrap_or((0, Vec::new(), false))
        }
        None => (0, Vec::new(), false),
    };

    // ── WM_NCHITTEST：自定义标题栏按钮 ──────────────
    if msg == WM_NCHITTEST && !regions.is_empty() {
        let mx = ((lparam.0 as u64) & 0xFFFF) as i32;
        let my = (((lparam.0 as u64) >> 16) & 0xFFFF) as i32;

        for (rx, ry, rw, rh, ref kind) in &regions {
            if mx >= *rx && mx <= *rx + *rw && my >= *ry && my <= *ry + *rh {
                let hit = match kind.as_str() {
                    "maxbutton" => HTMAXBUTTON,
                    "minbutton" => HTMINBUTTON,
                    "closebutton" => HTCLOSE,
                    _ => HTCAPTION, // 其他区域视为标题栏拖拽区
                };
                return LRESULT(hit as isize);
            }
        }
    }

    // ── WM_ENTERSIZEMOVE：开始拖拽/缩放 ─────────────
    if msg == WM_ENTERSIZEMOVE && is_detached {
        log::debug!("[titlebar] WM_ENTERSIZEMOVE: 分离窗口开始移动 0x{:x}", hwnd_raw);
        // 移动开始：后续 WM_EXITSIZEMOVE 将触发合并检测
    }

    // ── WM_EXITSIZEMOVE：结束拖拽/缩放 ─────────────
    if msg == WM_EXITSIZEMOVE && is_detached {
        log::debug!("[titlebar] WM_EXITSIZEMOVE: 分离窗口结束移动 0x{:x}", hwnd_raw);
        // 检测是否应触发合并（在转发原始消息之前执行，不阻塞窗口过程）
        try_merge_on_drag_end();
    }

    // ── 转发给原始窗口过程 ──────────────────────────
    if original != 0 {
        let proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT =
            std::mem::transmute(original);
        CallWindowProcW(Some(proc), hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

// ═══════════════════════════════════════════════════════════════════
// 合并检测逻辑
// ═══════════════════════════════════════════════════════════════════

/// Nav 区域在窗口客户区中的 CSS 像素范围（从窗口顶部算起）
///
/// 对应前端 CSS Grid 的 row3，实际值与布局相关。
/// 这些值会乘以 `device_pixel_ratio` 转换为物理像素。
const NAV_TOP_CSS_PX: f64 = 60.0;   // Nav 区域上边界（CSS px）
const NAV_BOTTOM_CSS_PX: f64 = 124.0; // Nav 区域下边界（CSS px）

/// 拖拽结束后尝试触发合并
///
/// 执行步骤：
/// 1. 获取当前光标屏幕坐标
/// 2. 获取主窗口 HWND（优先缓存，fallback 从全局 AppHandle 懒加载）
/// 3. 获取主窗口屏幕矩形
/// 4. 计算主窗口 Nav 区域（物理像素）
/// 5. 若光标在 Nav 区域内 → 发射 `drag-release` 事件
fn try_merge_on_drag_end() {
    // 1. 获取光标位置
    let mut cursor_pt = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut cursor_pt);
    }

    // 2. 解析主窗口 HWND（缓存优先，缺失时从事件系统懒加载）
    let main_hwnd_raw = resolve_main_hwnd();

    if main_hwnd_raw == 0 {
        log::warn!("[titlebar] 无法获取主窗口 HWND，跳过合并检测");
        return;
    }

    // 3. 获取 DPI 缩放比
    let scale = {
        let ctx = lock_context();
        ctx.device_pixel_ratio
    };
    // lock_context 返回的 MutexGuard 在此处 drop

    let main_hwnd = HWND(main_hwnd_raw as *mut _);

    // 4. 将光标从屏幕坐标转换为主窗口客户区坐标
    //    GetCursorPos → 屏幕坐标
    //    ScreenToClient → 客户区坐标（以 webview 内容区左上角为原点）
    //    Nav Y=60~124 CSS px 是相对于客户区顶部定义的，转换后可直接比较
    unsafe {
        let _ = ScreenToClient(main_hwnd, &mut cursor_pt);
    }

    // 5. 获取主窗口客户区尺寸（用于 X 方向边界检查）
    let mut client_rect = RECT::default();
    unsafe {
        let _ = GetClientRect(main_hwnd, &mut client_rect);
    }

    // 6. 计算 Nav 区域边界（客户区坐标系，物理像素）
    let nav_top = (NAV_TOP_CSS_PX * scale) as i32;
    let nav_bottom = (NAV_BOTTOM_CSS_PX * scale) as i32;

    // 7. 命中测试（三者在同一参考系：主窗口客户区坐标）
    let cursor_in_nav = cursor_pt.x >= 0
        && cursor_pt.x <= client_rect.right
        && cursor_pt.y >= nav_top
        && cursor_pt.y <= nav_bottom;

    log::debug!(
        "[titlebar] 合并检测: scale={:.2} client_size=({},{}) nav_y=({},{}) cursor_client=({},{}) hit={}",
        scale, client_rect.right, client_rect.bottom,
        nav_top, nav_bottom, cursor_pt.x, cursor_pt.y, cursor_in_nav,
    );

    // 6. 若命中 Nav，发射合并事件
    if cursor_in_nav {
        log::info!("[titlebar] 光标在主窗口 Nav 区域，发射 drag-release 事件");
        let payload = serde_json::json!({
            "screenX": cursor_pt.x,
            "screenY": cursor_pt.y,
        });
        emit!(dyn "drag-release": payload);
    }
}

/// 解析主窗口 HWND
///
/// 优先级：
/// 1. 从 `DragContext.main_hwnd` 缓存读取（`install_subclass` 在 setup 阶段写入）
/// 2. Fallback：从 `event_system::GLOBAL_APPHANDLE` 获取 main 窗口 → 写入缓存
///
/// 返回 0 表示无法获取（事件系统未初始化或 main 窗口不存在）。
fn resolve_main_hwnd() -> isize {
    let mut ctx = lock_context();

    // 优先使用缓存
    if let Some(cached) = ctx.main_hwnd {
        if cached != 0 {
            return cached;
        }
    }

    // Fallback：从全局 AppHandle 懒加载
    let resolved = GLOBAL_APPHANDLE
        .get()
        .and_then(|handle| handle.get_webview_window("main"))
        .and_then(|window| window.hwnd().ok())
        .map(|hwnd| hwnd.0 as isize)
        .unwrap_or(0);

    if resolved != 0 {
        // 回写缓存，避免后续调用重复查找
        ctx.main_hwnd = Some(resolved);
        log::info!("[titlebar] 主窗口 HWND 已从事件系统懒加载: 0x{:x}", resolved);
    }

    resolved
}

// ═══════════════════════════════════════════════════════════════════
// 测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_starts_empty() {
        let ctx = DragContext::new();
        assert!(ctx.window_states.is_empty());
        assert!(ctx.main_hwnd.is_none());
        assert!((ctx.device_pixel_ratio - 1.0).abs() < f64::EPSILON);
    }
}
