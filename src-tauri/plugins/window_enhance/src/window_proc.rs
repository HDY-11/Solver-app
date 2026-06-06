//! window_proc.rs — 统一窗口过程 + 命中测试 + 三层安全防护
//!
//! ## 架构角色
//!
//! 本模块包含：
//! - [`enhanced_window_proc`]：**唯一**的窗口过程，替代旧的双过程设计
//! - [`handle_nchittest`]：纯函数命中测试（公开导出，上层可复用）
//! - [`get_original_proc`]：子类化安装前获取原始过程地址
//! - [`forward_message`]：消息转发辅助
//!
//! ## 消息分发流程
//!
//! ```text
//! enhanced_window_proc(hwnd, msg, wparam, lparam)
//!   ├─ try_lock → 获取状态快照（Arc<behavior> + regions clone）→ 释放锁
//!   ├─ 内置 handle_nchittest（纯函数，优先于 trait 调用）
//!   ├─ 若 behaviors ⊇ NCHITTEST  && msg == WM_NCHITTEST → trait.on_nchittest
//!   ├─ 若 behaviors ⊇ DRAG_START && msg == WM_ENTERSIZEMOVE → trait.on_drag_start
//!   ├─ 若 behaviors ⊇ DRAG_END   && msg == WM_EXITSIZEMOVE → trait.on_drag_end
//!   └─ forward_message(original_proc, ...)
//! ```
//!
//! ## 三层安全防护（R7）
//!
//! 1. **try_lock 防重入**：锁竞争或 Poison 时降级到 `DefWindowProcW`
//! 2. **catch_unwind 防 panic**：trait 方法 panic 被捕获，不穿过 FFI 边界
//! 3. **Result 错误处理**：`Err` 记录日志后消息继续转发

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW, GetWindowLongPtrW,
    GWLP_WNDPROC, HTCAPTION, HTCLOSE, HTMAXBUTTON, HTMINBUTTON,
    WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_NCHITTEST,
};

use crate::behaviors::HookBehaviors;
use crate::manager::WindowManager;
use crate::platform;
use crate::window_behavior::WindowBehavior;

// ═══════════════════════════════════════════════════════════════════
// 子类化安装
// ═══════════════════════════════════════════════════════════════════

/// 获取目标窗口的原始窗口过程地址。
///
/// 仅调用 `GetWindowLongPtrW(hwnd, GWLP_WNDPROC)`，
/// 子类化安装由 [`WindowManager::register`] 在锁内完成。
///
/// # Safety
///
/// `hwnd` 必须是有效的窗口句柄。
#[cfg(target_os = "windows")]
pub unsafe fn get_original_proc(hwnd: HWND) -> isize {
    unsafe { GetWindowLongPtrW(hwnd, GWLP_WNDPROC) }
}

// ═══════════════════════════════════════════════════════════════════
// 命中测试纯函数（公开导出）
// ═══════════════════════════════════════════════════════════════════

/// 处理 `WM_NCHITTEST` — **纯函数**，无全局状态访问。
///
/// ## 参数
/// - `lparam`: LPARAM 原始值（低 16 位 = X，高 16 位 = Y 屏幕坐标）
/// - `regions`: 自定义标题栏区域列表 `[(x, y, w, h, kind)]`，物理像素坐标
///
/// ## 命中规则
///
/// | kind            | 返回值         |
/// |-----------------|----------------|
/// | `"maxbutton"`   | `HTMAXBUTTON`  |
/// | `"minbutton"`   | `HTMINBUTTON`  |
/// | `"closebutton"` | `HTCLOSE`      |
/// | 其他            | `HTCAPTION`    |
///
/// ## 复杂度
///
/// O(n)，n = regions.len()。对 n < 10 的场景，线性扫描优于任何数据结构。
#[cfg(target_os = "windows")]
pub fn handle_nchittest(
    lparam: LPARAM,
    regions: &[(i32, i32, i32, i32, String)],
) -> Option<LRESULT> {
    if regions.is_empty() {
        return None;
    }

    let mx = ((lparam.0 as u64) & 0xFFFF) as i32;
    let my = (((lparam.0 as u64) >> 16) & 0xFFFF) as i32;

    for (rx, ry, rw, rh, kind) in regions {
        // 防御性校验：跳过无效区域
        if *rw <= 0 || *rh <= 0 {
            continue;
        }

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
// 统一窗口过程
// ═══════════════════════════════════════════════════════════════════

/// 统一增强窗口过程 — 所有已注册窗口共用。
///
/// 替代旧的 `main_window_proc` / `detached_window_proc` 双过程设计。
/// 通过 `HookBehaviors` bitflags 决定消息分发路径，
/// 编译器将 `contains()` + `if` 链优化为跳表。
///
/// ## 处理流程
///
/// 1. **获取快照**：try_lock → clone regions + Arc → 释放锁（防死锁）
/// 2. **内置命中测试**：`handle_nchittest` 纯函数优先（零虚函数开销）
/// 3. **消息处理器调度**：根据 bitflags 调用已注册的 trait 方法
/// 4. **消息转发**：未处理消息转发给原始窗口过程
///
/// # Safety
///
/// 作为 `SetWindowLongPtrW(GWLP_WNDPROC)` 目标安装，必须保持 `extern "system"` 约定。
/// 内部 `catch_unwind` 确保 panic 不穿过 FFI 边界。
#[cfg(target_os = "windows")]
pub unsafe extern "system" fn enhanced_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let hwnd_raw = hwnd.0 as isize;

    // ── 防御性校验：null HWND ──
    if hwnd_raw == 0 {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }

    let manager = WindowManager::global();

    // ── Step 1: 获取状态快照（锁内 → 锁外）──
    let snapshot = match manager.try_lock_windows() {
        Some(guard) => match guard.get(&hwnd_raw) {
            Some(state) => WindowProcSnapshot {
                original_proc: state.original_proc,
                regions: state.regions.clone(),
                behaviors: state.behaviors,
                behavior: Arc::clone(&state.behavior),
            },
            None => return unsafe { forward_message(0, hwnd, msg, wparam, lparam) },
        },
        None => return unsafe { forward_message(0, hwnd, msg, wparam, lparam) },
    };
    // ── 锁已释放，以下操作在锁外执行 ──

    // ── Step 2: 消息分发 ──

    // 2a. WM_NCHITTEST — 内置命中测试优先
    if msg == WM_NCHITTEST && snapshot.behaviors.contains(HookBehaviors::NCHITTEST) {
        if let Some(result) = handle_nchittest(lparam, &snapshot.regions) {
            return result;
        }
        let pt = platform::unpack_nchittest_lparam(lparam.0);
        if let Some(result) = call_handler_safely(&snapshot.behavior, |handler| {
            handler.on_nchittest(hwnd_raw, pt.x, pt.y).map(|opt| opt)
        }) {
            return result;
        }
    }

    // 2b. WM_ENTERSIZEMOVE — 拖拽开始
    if msg == WM_ENTERSIZEMOVE && snapshot.behaviors.contains(HookBehaviors::DRAG_START) {
        let _ = call_handler_safely(&snapshot.behavior, |handler| {
            handler.on_drag_start(hwnd_raw).map(|()| None)
        });
    }

    // 2c. WM_EXITSIZEMOVE — 拖拽结束
    if msg == WM_EXITSIZEMOVE && snapshot.behaviors.contains(HookBehaviors::DRAG_END) {
        let _ = call_handler_safely(&snapshot.behavior, |handler| {
            handler.on_drag_end(hwnd_raw).map(|()| None)
        });
    }

    // ── Step 3: 转发未处理消息 ──
    unsafe { forward_message(snapshot.original_proc, hwnd, msg, wparam, lparam) }
}

// ═══════════════════════════════════════════════════════════════════
// 窗口过程快照
// ═══════════════════════════════════════════════════════════════════

/// 窗口过程在锁内提取的状态快照。
///
/// 提取完成后立即释放锁，后续所有操作使用快照。
struct WindowProcSnapshot {
    original_proc: isize,
    regions: Vec<(i32, i32, i32, i32, String)>,
    behaviors: HookBehaviors,
    behavior: Arc<dyn WindowBehavior>,
}

// ═══════════════════════════════════════════════════════════════════
// 消息转发
// ═══════════════════════════════════════════════════════════════════

/// 将消息转发给原始窗口过程，或降级为默认处理。
#[cfg(target_os = "windows")]
unsafe fn forward_message(
    original_proc: isize,
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if original_proc != 0 {
        let proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT =
            unsafe { std::mem::transmute(original_proc) };
        unsafe { CallWindowProcW(Some(proc), hwnd, msg, wparam, lparam) }
    } else {
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }
}

// ═══════════════════════════════════════════════════════════════════
// 安全调用包装（三层防护）
// ═══════════════════════════════════════════════════════════════════

/// 安全调用已注册的消息处理器。
///
/// ## 三层防护
///
/// | 层级 | 机制 | 失败行为 |
/// |------|------|----------|
/// | 编译期 | `Result<_, BehaviorError>` 类型约束 | 强制调用方处理错误 |
/// | 运行时 | `catch_unwind` | panic 被捕获，日志记录后降级 |
/// | 降级 | `Err` 日志记录 | 消息继续转发，不吞没 |
///
/// ## AssertUnwindSafe 使用说明
///
/// 闭包仅捕获 `&Arc<dyn WindowBehavior>`（不可变引用）。
/// `WindowBehavior: RefUnwindSafe` 约束保证此用法安全。
/// `AssertUnwindSafe` 仅用于满足 `catch_unwind` 的类型签名。
fn call_handler_safely<F>(behavior: &Arc<dyn WindowBehavior>, method: F) -> Option<LRESULT>
where
    F: FnOnce(&dyn WindowBehavior) -> Result<Option<isize>, crate::window_behavior::BehaviorError>,
{
    let result = catch_unwind(AssertUnwindSafe(|| method(behavior.as_ref())));

    match result {
        Ok(Ok(Some(value))) => Some(LRESULT(value)),
        Ok(Ok(None)) => None,
        Ok(Err(e)) => {
            log::error!("[window_enhance] 消息处理器返回错误: {}", e);
            None
        }
        Err(panic_payload) => {
            let msg = extract_panic_message(&panic_payload);
            log::error!("[window_enhance] 消息处理器 panic: {}", msg);
            None
        }
    }
}

/// 从 panic payload 中提取可读消息。
fn extract_panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<unknown panic>".to_string()
    }
}
