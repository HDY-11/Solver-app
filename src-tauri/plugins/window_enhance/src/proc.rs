//! proc.rs — 统一窗口过程 + Hook 分发 + 命中测试纯函数
//!
//! ## 职责
//! - [`enhanced_window_proc`]：**唯一**窗口过程 — bitflags 跳表分发 + catch_unwind 安全包裹
//! - [`handle_nchittest`]：**纯函数** — 公开导出，供业务层 hook 实现复用
//! - [`get_original_proc`]：获取目标窗口的原始窗口过程地址
//! - [`forward_message`]：消息转发辅助
//!
//! ## 性能设计
//!
//! ### 跳表分发（方案 A 精髓）
//!
//! ```text
//! 编译器将以下 if 链优化为位测试 + 条件分支（等价跳表）：
//!   if msg == WM_NCHITTEST && behaviors.contains(NCHITTEST) → 处理
//!   if msg == WM_ENTERSIZEMOVE && behaviors.contains(DRAG_START) → 处理
//!   if msg == WM_EXITSIZEMOVE && behaviors.contains(DRAG_END) → 处理
//!   → 转发
//! ```
//!
//! 每个 `contains()` 是 O(1) 位操作；每个 `msg == WM_*` 是 O(1) 整数比较。
//! 总体分发 O(k) where k ≤ 3（行为标志数，常数级）。
//!
//! ### 锁内最小化
//!
//! - Step 1 仅持有锁获取快照（regions clone + Arc clone）→ 立即释放
//! - Step 2-4 在锁外执行（trait 调用、消息转发）
//! - 防止嵌套消息死锁（trait 方法内可能触发新消息）
//!
//! ## 安全设计（方案 B 精髓 — 三层防护）
//!
//! 1. **编译期**：trait 方法返回 `Result<_, BehaviorError>`，类型系统强制错误处理
//! 2. **运行时**：`catch_unwind` 捕获 panic，防止 unwinding 穿过 FFI 边界（UB）
//! 3. **降级**：panic 或 `Err` 均记录日志后转发消息，不影响窗口正常功能
//!
//! ### catch_unwind 安全
//!
//! - `WindowBehavior: RefUnwindSafe` 约束确保闭包自身满足 `UnwindSafe`
//! - **不使用 AssertUnwindSafe 滥用模式**——完全依赖类型系统
//! - 每个 trait 调用使用独立的 `Arc::clone`（O(1)），move 入闭包
//!
//! ### 防重入
//!
//! `try_lock_windows()` 非阻塞尝试获取锁：
//! - `WouldBlock`：正常锁竞争（嵌套消息），静默降级 → `DefWindowProcW`
//! - `Poisoned`：前持有者 panic，恢复数据后继续（HashMap 操作为原子替换，安全）
//!
//! ## 复杂度
//!
//! - 消息分发：O(k)，k = 行为标志数（当前 3，常数级）
//! - `handle_nchittest`：O(n)，n = regions 数量（实际 < 10）
//! - 防御性区域校验：O(n)，跳过 `rw <= 0 || rh <= 0` 的无效区域
//! - `try_lock`：O(1) 非阻塞
//! - regions clone：O(n)，n 小
//! - Arc clone：O(1) 原子操作

#[cfg(target_os = "windows")]
use std::sync::Arc;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW,
    GetWindowLongPtrW, GWLP_WNDPROC,
    HTCAPTION, HTCLOSE, HTMAXBUTTON, HTMINBUTTON,
    WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_NCHITTEST,
};

#[cfg(target_os = "windows")]
use crate::behavior::HookBehaviors;
#[cfg(target_os = "windows")]
use crate::behavior::WindowBehavior;
#[cfg(target_os = "windows")]
use crate::manager::WindowManager;

// ═══════════════════════════════════════════════════════════════════
// 子类化安装
// ═══════════════════════════════════════════════════════════════════

/// 获取目标窗口的原始窗口过程地址。O(1) 系统调用。
///
/// 此函数仅调用 `GetWindowLongPtrW(hwnd, GWLP_WNDPROC)`，
/// 子类化安装由 [`WindowManager::register`] 在锁内完成。
///
/// # Safety
///
/// `hwnd` 必须是有效的窗口句柄。传入无效句柄可能导致未定义行为。
/// 调用方（`WindowManager::register`）通过前端验证确保 hwnd 有效性。
#[cfg(target_os = "windows")]
pub unsafe fn get_original_proc(hwnd: HWND) -> isize {
    GetWindowLongPtrW(hwnd, GWLP_WNDPROC)
}

// ═══════════════════════════════════════════════════════════════════
// 命中测试（纯函数，公共导出供业务层 hook 复用）
// ═══════════════════════════════════════════════════════════════════

/// 处理 `WM_NCHITTEST` — **纯函数**，不访问任何全局状态。
///
/// ## 参数
/// - `lparam`: LPARAM 原始值（`lparam.0`），
///   低 16 位 = 鼠标 X 屏幕坐标，高 16 位 = 鼠标 Y 屏幕坐标
/// - `regions`: 自定义标题栏区域列表 `[(x, y, w, h, kind)]`，
///   物理像素坐标，可能为空
///
/// ## 返回值
/// - `Some(LRESULT)`: 命中某个自定义区域
/// - `None`: 未命中任何区域（或区域列表为空）
///
/// ## 复杂度
/// O(n) where n = regions.len()。对于 n < 10 的实际场景，
/// 线性扫描优于哈希表/树结构（无额外内存开销、无哈希计算、缓存友好）。
///
/// ## 防御性校验
///
/// 跳过 `rw <= 0` 或 `rh <= 0` 的无效区域（前端可能传入零尺寸或负尺寸区域）。
///
/// ## 区域类型映射
///
/// - `"maxbutton"` → `HTMAXBUTTON`
/// - `"minbutton"` → `HTMINBUTTON`
/// - `"closebutton"` → `HTCLOSE`
/// - 其他（包括未知类型）→ `HTCAPTION`（视为可拖拽标题栏，安全降级）
pub fn handle_nchittest(
    lparam: isize,
    regions: &[(i32, i32, i32, i32, String)],
) -> Option<isize> {
    if regions.is_empty() {
        return None;
    }

    // LPARAM 解码：lo-word = X, hi-word = Y（屏幕坐标）
    // 使用 u64 中间类型避免 isize 可能的符号扩展问题
    let mx = ((lparam as u64) & 0xFFFF) as i32;
    let my = (((lparam as u64) >> 16) & 0xFFFF) as i32;

    for region in regions {
        let rx = region.0;
        let ry = region.1;
        let rw = region.2;
        let rh = region.3;
        let kind = &region.4;

        // 防御性校验：跳过无效尺寸区域
        if rw <= 0 || rh <= 0 {
            continue;
        }

        if mx >= rx && mx <= rx + rw && my >= ry && my <= ry + rh {
            let hit = match kind.as_str() {
                "maxbutton" => HTMAXBUTTON.0 as isize,
                "minbutton" => HTMINBUTTON.0 as isize,
                "closebutton" => HTCLOSE.0 as isize,
                _ => HTCAPTION.0 as isize, // 未知类型安全降级为拖拽区
            };
            return Some(hit);
        }
    }

    None
}

// ═══════════════════════════════════════════════════════════════════
// enhanced_window_proc — 统一窗口过程（跳表分发 + 三层防护）
// ═══════════════════════════════════════════════════════════════════

/// 统一增强窗口过程 — 所有已注册窗口共用此过程。
///
/// 替代旧的 `main_window_proc` / `detached_window_proc` 双过程设计。
/// 通过 bitflags 行为声明 + `WindowBehavior` hook 决定消息处理路径。
///
/// ## 处理流程
///
/// ```text
/// enhanced_window_proc(hwnd, msg, wparam, lparam)
///   ├─ try_lock → 获取状态快照（regions + Arc<behavior>）→ 释放锁
///   ├─ [跳表] 若 msg == WM_NCHITTEST && behaviors ⊇ NCHITTEST:
///   │    ├─ handle_nchittest(lparam, &regions) → 内置纯函数（零虚函数开销）
///   │    └─ call_hook_safely(behavior.on_nchittest) → 业务层 hook
///   ├─ [跳表] 若 msg == WM_ENTERSIZEMOVE && behaviors ⊇ DRAG_START:
///   │    └─ call_hook_safely(behavior.on_enter_size_move)
///   ├─ [跳表] 若 msg == WM_EXITSIZEMOVE && behaviors ⊇ DRAG_END:
///   │    └─ call_hook_safely(behavior.on_exit_size_move)
///   └─ forward_message(original_proc, ...)
/// ```
///
/// ## 与旧设计对比
///
/// | 旧设计                                    | 新设计                          |
/// |-------------------------------------------|---------------------------------|
/// | 两个窗口过程（main/detached）             | 单一统一过程                    |
/// | WindowKind 枚举决定安装哪个过程           | HookBehaviors bitflags 决定分发 |
/// | 硬编码拖拽合并检测                        | 通过 hook（trait 方法）注入     |
/// | 直接调用 event_system::emit!             | 零业务逻辑，仅调用 hook         |
///
/// ## panic 安全（R7 — 三层防护）
///
/// 1. 编译期：trait 方法签名强制返回 `Result` → 类型系统保证错误被处理
/// 2. 运行时：`catch_unwind` 包裹每个 hook 调用 → panic 不穿过 FFI 边界
/// 3. 降级：panic 或 Err → `log::error!` + 转发消息 → 窗口功能不丧失
///
/// # Safety
///
/// 此函数作为 `SetWindowLongPtrW(GWLP_WNDPROC)` 的目标被安装。
/// 必须保持 `extern "system"` 调用约定。
#[cfg(target_os = "windows")]
pub unsafe extern "system" fn enhanced_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let hwnd_raw = hwnd.0 as isize;

    // ── 防御性校验：HWND 不得为 null ──
    if hwnd_raw == 0 {
        return LRESULT(0);
    }

    let manager = WindowManager::global();

    // ── Step 1: 获取状态快照（锁内最小化）──
    // O(1) try_lock + O(n) regions clone + O(1) Arc clone
    let snapshot = match manager.try_lock_windows() {
        Some(guard) => match guard.get(&hwnd_raw) {
            Some(state) => WindowProcSnapshot {
                original_proc: state.original_proc,
                regions: state.regions.clone(),       // O(n), n < 10
                behaviors: state.behaviors,           // Copy
                behavior: Arc::clone(&state.behavior), // O(1) 原子操作
            },
            None => {
                return forward_message(0, hwnd, msg, wparam, lparam);
            }
        },
        None => {
            return forward_message(0, hwnd, msg, wparam, lparam);
        }
    };
    // ── 锁已释放 ──

    let lparam_raw = lparam.0;
    let regions = snapshot.regions; // 转移所有权

    // ── Step 2: 跳表分发 ──
    // 编译器将 contains() + msg == 的 if 链优化为位测试 + 条件分支（等价跳表）
    // 每个 contains() 是 O(1) 位操作

    // NCHITTEST — 自定义标题栏命中测试
    if msg == WM_NCHITTEST && snapshot.behaviors.contains(HookBehaviors::NCHITTEST) {
        // 内置纯函数优先（零虚函数开销，O(n)）
        if let Some(hit) = handle_nchittest(lparam_raw, &regions) {
            return LRESULT(hit);
        }

        // 业务层 hook（catch_unwind 包裹）
        let behavior = Arc::clone(&snapshot.behavior);
        let regions_clone = regions.clone();
        match call_hook_safely(move || {
            behavior.on_nchittest(hwnd_raw, lparam_raw, &regions_clone)
        }) {
            HookResult::Handled(lresult) => return LRESULT(lresult),
            HookResult::NotHandled => { /* 未命中，继续转发 */ }
            HookResult::Error(e) => {
                log::error!("[window_enhance] on_nchittest hook 错误 0x{:x}: {}", hwnd_raw, e);
            }
            HookResult::Panic(payload) => {
                log::error!(
                    "[window_enhance] on_nchittest hook panic 0x{:x}: {}",
                    hwnd_raw,
                    extract_panic_msg(&payload)
                );
            }
        }
        // 未命中 → 继续转发
    }

    // DRAG_START — 窗口拖拽开始
    if msg == WM_ENTERSIZEMOVE && snapshot.behaviors.contains(HookBehaviors::DRAG_START) {
        let behavior = Arc::clone(&snapshot.behavior);
        match call_hook_safely(move || {
            behavior.on_enter_size_move(hwnd_raw).map(|_| None)
        }) {
            HookResult::Error(e) => {
                log::error!("[window_enhance] on_enter_size_move hook 错误 0x{:x}: {}", hwnd_raw, e);
            }
            HookResult::Panic(payload) => {
                log::error!(
                    "[window_enhance] on_enter_size_move hook panic 0x{:x}: {}",
                    hwnd_raw,
                    extract_panic_msg(&payload)
                );
            }
            _ => { /* 正常完成或未处理 */ }
        }
    }

    // DRAG_END — 窗口拖拽结束
    if msg == WM_EXITSIZEMOVE && snapshot.behaviors.contains(HookBehaviors::DRAG_END) {
        let dpr_snapshot = manager.dpr();
        let behavior = Arc::clone(&snapshot.behavior);
        match call_hook_safely(move || {
            behavior.on_exit_size_move(hwnd_raw, dpr_snapshot).map(|_| None)
        }) {
            HookResult::Error(e) => {
                log::error!("[window_enhance] on_exit_size_move hook 错误 0x{:x}: {}", hwnd_raw, e);
            }
            HookResult::Panic(payload) => {
                log::error!(
                    "[window_enhance] on_exit_size_move hook panic 0x{:x}: {}",
                    hwnd_raw,
                    extract_panic_msg(&payload)
                );
            }
            _ => { /* 正常完成或未处理 */ }
        }
    }

    // ── Step 3: 转发未处理消息 ──
    forward_message(snapshot.original_proc, hwnd, msg, wparam, lparam)
}

// ═══════════════════════════════════════════════════════════════════
// 窗口过程快照（锁内提取，锁外使用）
// ═══════════════════════════════════════════════════════════════════

#[cfg(target_os = "windows")]
struct WindowProcSnapshot {
    original_proc: isize,
    regions: Vec<(i32, i32, i32, i32, String)>,
    behaviors: HookBehaviors,
    behavior: Arc<dyn WindowBehavior>,
}

// ═══════════════════════════════════════════════════════════════════
// Hook 调用结果（三层防护的中间表示）
// ═══════════════════════════════════════════════════════════════════

#[cfg(target_os = "windows")]
enum HookResult {
    /// 业务层 hook 返回了处理结果
    Handled(isize),
    /// 业务层 hook 未做处理（Ok(None) 或 Ok(())）
    NotHandled,
    /// 业务层 hook 返回错误
    Error(crate::behavior::BehaviorError),
    /// 业务层 hook panic
    Panic(Box<dyn std::any::Any + Send>),
}

// ═══════════════════════════════════════════════════════════════════
// 安全调用包装（catch_unwind + 结果分类）
// ═══════════════════════════════════════════════════════════════════

/// 安全调用业务层 hook — `catch_unwind` + 结构化结果分类。
///
/// ## 设计说明
///
/// 将 `catch_unwind` 的结果统一转换为 `HookResult` 枚举，
/// 调用方通过 match 穷举处理，**不吞没错误**（全程记录日志）。
///
/// ## AssertUnwindSafe 使用
///
/// 闭包捕获的类型：`Arc<dyn WindowBehavior>` + 原始类型（isize、i32、String）。
/// `WindowBehavior: RefUnwindSafe` 约束 + 原始类型均为 `UnwindSafe`，
/// 确保闭包整体满足 `UnwindSafe`。
/// `AssertUnwindSafe` 仅用于满足 `catch_unwind` 的类型签名。
///
/// ## 复杂度
/// O(1) — catch_unwind + 模式匹配。
#[cfg(target_os = "windows")]
fn call_hook_safely<F>(hook: F) -> HookResult
where
    F: FnOnce() -> Result<Option<isize>, crate::behavior::BehaviorError> + std::panic::UnwindSafe,
{
    match std::panic::catch_unwind(hook) {
        Ok(Ok(Some(value))) => HookResult::Handled(value),
        Ok(Ok(None)) => HookResult::NotHandled,
        Ok(Err(e)) => HookResult::Error(e),
        Err(payload) => HookResult::Panic(payload),
    }
}

// ═══════════════════════════════════════════════════════════════════
// 消息转发
// ═══════════════════════════════════════════════════════════════════

/// 将消息转发给原始窗口过程或默认过程。O(1)。
///
/// - 若 `original_proc != 0` → 调用 `CallWindowProcW`
/// - 否则 → 调用 `DefWindowProcW`（安全降级）
///
/// # Safety
///
/// `original_proc` 必须是有效的窗口过程地址或为 0。
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
            std::mem::transmute(original_proc);
        CallWindowProcW(Some(proc), hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

// ═══════════════════════════════════════════════════════════════════
// panic 消息提取辅助
// ═══════════════════════════════════════════════════════════════════

/// 从 `catch_unwind` 的 Err 负载中提取可读的消息。O(1)。
///
/// 支持 `&str`、`String` 两种常见 panic 负载类型。
/// 未知类型返回占位字符串（不 panic）。
#[cfg(target_os = "windows")]
fn extract_panic_msg(panic_payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = panic_payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = panic_payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<unknown panic payload>".to_string()
    }
}

// ═══════════════════════════════════════════════════════════════════
// 测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── handle_nchittest 纯函数测试 ──

    #[test]
    fn nchittest_empty_regions_returns_none() {
        let regions: Vec<(i32, i32, i32, i32, String)> = vec![];
        assert!(handle_nchittest(0x00320064, &regions).is_none());
    }

    #[test]
    fn nchittest_hits_closebutton() {
        let regions = vec![(90, 5, 110, 30, "closebutton".to_string())];
        // lParam: x=100, y=10 (inside the region)
        let lparam = ((10i64 << 32) | 100i64) as isize;
        let result = handle_nchittest(lparam, &regions);
        assert_eq!(result, Some(HTCLOSE.0 as isize));
    }

    #[test]
    fn nchittest_hits_maxbutton() {
        let regions = vec![(50, 5, 70, 30, "maxbutton".to_string())];
        let lparam = ((10i64 << 32) | 60i64) as isize;
        let result = handle_nchittest(lparam, &regions);
        assert_eq!(result, Some(HTMAXBUTTON.0 as isize));
    }

    #[test]
    fn nchittest_hits_minbutton() {
        let regions = vec![(10, 5, 30, 30, "minbutton".to_string())];
        let lparam = ((10i64 << 32) | 20i64) as isize;
        let result = handle_nchittest(lparam, &regions);
        assert_eq!(result, Some(HTMINBUTTON.0 as isize));
    }

    #[test]
    fn nchittest_hits_caption_for_unknown_kind() {
        let regions = vec![(0, 0, 200, 50, "dragzone".to_string())];
        let lparam = ((25i64 << 32) | 100i64) as isize;
        let result = handle_nchittest(lparam, &regions);
        assert_eq!(result, Some(HTCAPTION.0 as isize));
    }

    #[test]
    fn nchittest_miss_outside_region() {
        let regions = vec![(90, 5, 110, 30, "closebutton".to_string())];
        // lParam: x=300, y=300 (far outside)
        let lparam = ((300i64 << 32) | 300i64) as isize;
        assert!(handle_nchittest(lparam, &regions).is_none());
    }

    #[test]
    fn nchittest_skips_invalid_dimensions() {
        // 包含零宽度和负高度的无效区域，应被跳过
        let regions = vec![
            (0, 0, 0, 30, "broken1".to_string()),    // width=0
            (50, 0, 40, -5, "broken2".to_string()),   // height<0
            (100, 0, 50, 30, "valid".to_string()),     // 有效区域
        ];
        // lParam: x=120, y=15 — 命中 valid 区域
        let lparam = ((15i64 << 32) | 120i64) as isize;
        let result = handle_nchittest(lparam, &regions);
        assert_eq!(result, Some(HTCAPTION.0 as isize));
    }

    #[test]
    fn nchittest_exact_boundary_hit() {
        let regions = vec![(0, 0, 50, 50, "minbutton".to_string())];
        // 边界值：等于左上角
        let result = handle_nchittest(0, &regions);
        assert!(result.is_some());
        // 边界值：等于右下角
        let result = handle_nchittest(((50i64 << 32) | 50i64) as isize, &regions);
        assert!(result.is_some());
    }

    #[test]
    fn nchittest_single_pixel_region() {
        // 1x1 像素的极小区域
        let regions = vec![(100, 100, 1, 1, "closebutton".to_string())];
        let lparam = ((100i64 << 32) | 100i64) as isize;
        let result = handle_nchittest(lparam, &regions);
        assert_eq!(result, Some(HTCLOSE.0 as isize));
    }
}
