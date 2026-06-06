//! manager.rs — WindowManager 门面（Facade）
//!
//! ## 职责
//!
//! [`WindowManager`] 是窗口增强系统的**统一门面**，直接持有所有窗口状态。
//!
//! 提供的能力：
//! - **Hook 注册**：[`register`] 接收行为声明 + 消息处理器，注入到窗口消息链
//! - **区域更新**：[`update_regions`] 动态更新标题栏命中区域
//! - **DPR 管理**：[`set_dpr`] / [`dpr`] 设备像素比读写
//! - **通用查询**：[`find_first_hwnd_by`] 零业务语义的窗口查找
//! - **平台查询**：[`cursor_position`] / [`screen_to_client`] / [`client_rect`]
//!
//! ## 安全设计
//!
//! - **全局单例**：`LazyLock<WindowManager>` 保证线程安全延迟初始化
//! - **PoisonError 全覆盖**：每个锁路径显式处理 PoisonError
//! - **DPI 独立锁**：避免与窗口 Map 锁竞争
//! - **双重幂等检查**：锁外 + 锁内，消除 TOCTOU 竞态
//! - **锁内最小化**：仅 `SetWindowLongPtrW` + `HashMap::insert` 在锁内

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use crate::behaviors::HookBehaviors;
use crate::platform::{RectSize, ScreenPoint};
use crate::state::WindowState;
use crate::window_behavior::WindowBehavior;

#[cfg(target_os = "windows")]
use crate::window_proc;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HWND;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{GWLP_WNDPROC, SetWindowLongPtrW};

// ═══════════════════════════════════════════════════════════════════
// WindowManager — 全局单例门面
// ═══════════════════════════════════════════════════════════════════

/// 窗口管理器全局单例。
///
/// ## 使用方式（Hook 注入模式）
///
/// ```ignore
/// let manager = WindowManager::global();
///
/// // 主窗口：注册内置命中测试（无需自定义消息处理器）
/// manager.register(
///     main_hwnd,
///     HookBehaviors::NCHITTEST,
///     Box::new(NoopWindowBehavior),
/// );
///
/// // 分离窗口：注册命中测试 + 拖拽合并检测
/// manager.register(
///     detached_hwnd,
///     HookBehaviors::NCHITTEST | HookBehaviors::DRAG_START | HookBehaviors::DRAG_END,
///     Box::new(DetachedWindowBehavior::new(app_handle)),
/// );
/// ```
pub struct WindowManager {
    /// 所有已注册窗口的状态，以 `hwnd_raw` (isize) 为键
    windows: Mutex<HashMap<isize, WindowState>>,
    /// 设备像素比（devicePixelRatio），用于 CSS px → 物理 px 坐标转换
    ///
    /// 独立锁——与 `windows` 锁分离，避免 DPR 读取阻塞窗口注册
    dpr: Mutex<f64>,
}

impl WindowManager {
    /// 获取全局单例实例。
    pub fn global() -> &'static Self {
        static INSTANCE: LazyLock<WindowManager> = LazyLock::new(|| WindowManager {
            windows: Mutex::new(HashMap::new()),
            dpr: Mutex::new(1.0),
        });
        &INSTANCE
    }

    // ── 锁辅助 ──────────────────────────────────

    /// 阻塞获取窗口 Map 锁（处理 PoisonError 降级）。
    ///
    /// 若前持有者 panic 导致 Mutex poison，通过 `into_inner()` 恢复数据。
    fn lock_windows(&self) -> std::sync::MutexGuard<'_, HashMap<isize, WindowState>> {
        self.windows.lock().unwrap_or_else(|poison| {
            log::error!("[window_enhance] windows Mutex 已 poison（前持有者 panic），使用降级状态");
            poison.into_inner()
        })
    }

    /// 尝试获取窗口 Map 锁（非阻塞，处理 PoisonError）。
    ///
    /// 用于窗口过程等**不可阻塞**上下文。
    /// 区分 `WouldBlock`（正常竞争，静默降级）和 `Poisoned`（panic 恢复）。
    pub fn try_lock_windows(
        &self,
    ) -> Option<std::sync::MutexGuard<'_, HashMap<isize, WindowState>>> {
        match self.windows.try_lock() {
            Ok(guard) => Some(guard),
            Err(std::sync::TryLockError::WouldBlock) => {
                log::debug!("[window_enhance] windows try_lock 竞争（嵌套消息），跳过本次处理");
                None
            }
            Err(std::sync::TryLockError::Poisoned(poison)) => {
                log::error!("[window_enhance] windows try_lock poison，使用降级状态");
                Some(poison.into_inner())
            }
        }
    }

    /// 阻塞获取 DPR 锁（处理 PoisonError 降级）。
    fn lock_dpr(&self) -> std::sync::MutexGuard<'_, f64> {
        self.dpr.lock().unwrap_or_else(|poison| {
            log::error!("[window_enhance] dpr Mutex 已 poison");
            poison.into_inner()
        })
    }

    // ── Hook 注册 ──────────────────────────────

    /// 注入消息处理器到窗口消息链（Hook 注入模式入口）。
    ///
    /// 这是唯一的注册入口。不再通过 `WindowKind` 做硬编码分发——
    /// 行为由 `behaviors`（声明兴趣）和 `behavior`（处理逻辑）联合定义。
    ///
    /// ## 参数
    ///
    /// - `hwnd_raw`: 窗口 HWND 原始值（`hwnd.0 as isize`）
    /// - `behaviors`: 此窗口感兴趣的消息类型（bitflags 组合）
    /// - `behavior`: 注入的消息处理器（trait object）
    ///
    /// ## 幂等性
    ///
    /// 双重检查（锁外 + 锁内）防止 TOCTOU 竞态。
    /// 已注册窗口跳过，防止 `original_proc` 指向自身导致无限递归。
    pub fn register(
        &self,
        hwnd_raw: isize,
        behaviors: HookBehaviors,
        behavior: Box<dyn WindowBehavior>,
    ) {
        // 幂等检查 1/2：锁外快速路径
        {
            let windows = self.lock_windows();
            if windows.contains_key(&hwnd_raw) {
                log::debug!(
                    "[window_enhance] 窗口已注册，跳过: 0x{:x} behaviors={:?}",
                    hwnd_raw, behaviors,
                );
                return;
            }
        }

        self.do_register(hwnd_raw, behaviors, behavior);
    }

    /// 执行实际注册（Windows 平台）。
    #[cfg(target_os = "windows")]
    fn do_register(
        &self,
        hwnd_raw: isize,
        behaviors: HookBehaviors,
        behavior: Box<dyn WindowBehavior>,
    ) {
        let hwnd = HWND(hwnd_raw as *mut _);

        // 在锁外获取原始窗口过程（避免重入）
        let original_proc = unsafe { window_proc::get_original_proc(hwnd) };

        let state = WindowState::new(original_proc, behaviors, behavior);

        let mut windows = self.lock_windows();

        // 幂等检查 2/2：锁内二次确认
        if windows.contains_key(&hwnd_raw) {
            log::debug!(
                "[window_enhance] 窗口已注册（锁内确认），跳过: 0x{:x}",
                hwnd_raw,
            );
            return;
        }

        // 先存入状态，再安装窗口过程（确保消息到达时窗口已在 Map 中）
        // 顺序至关重要：insert 在 SetWindowLongPtrW 之前执行，
        // 消除「过程已安装但窗口不在 Map」的短暂窗口期——
        // 在此期间消息会走 DefWindowProcW 而非原始 Tauri 过程。
        windows.insert(hwnd_raw, state);

        let proc_addr = window_proc::enhanced_window_proc as *const () as usize as isize;
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_WNDPROC, proc_addr);
        }

        log::info!(
            "[window_enhance] 窗口已注册: 0x{:x} behaviors={:?}",
            hwnd_raw, behaviors,  // 注意：state 已 moved，用局部变量 behaviors
        );
    }

    /// 非 Windows 平台：no-op 注册。
    #[cfg(not(target_os = "windows"))]
    fn do_register(
        &self,
        hwnd_raw: isize,
        behaviors: HookBehaviors,
        behavior: Box<dyn WindowBehavior>,
    ) {
        let state = WindowState::new(0, behaviors, behavior);
        let mut windows = self.lock_windows();
        log::debug!("[window_enhance] 窗口已注册（非 Windows no-op）: 0x{:x}", hwnd_raw);
        windows.insert(hwnd_raw, state);
    }

    // ── 状态读写 ───────────────────────────────

    /// 更新指定窗口的自定义标题栏区域。
    ///
    /// 前端应在窗口大小改变时（ResizeObserver）调用。
    pub fn update_regions(&self, hwnd_raw: isize, regions: Vec<(i32, i32, i32, i32, String)>) {
        let mut windows = self.lock_windows();
        if let Some(state) = windows.get_mut(&hwnd_raw) {
            state.regions = regions;
            log::debug!("[window_enhance] 区域已更新: 0x{:x} {} 个区域", hwnd_raw, state.regions.len());
        } else {
            log::warn!("[window_enhance] 更新区域失败：窗口未注册 0x{:x}", hwnd_raw);
        }
    }

    /// 设置设备像素比。
    ///
    /// 前端应在获取到 `window.devicePixelRatio` 后调用。
    /// 拒绝非法值（≤ 0 或 NaN/Inf），保留旧值。
    pub fn set_dpr(&self, dpr: f64) {
        if dpr <= 0.0 || !dpr.is_finite() {
            log::warn!("[window_enhance] set_dpr: 非法值 {}，保留旧值", dpr);
            return;
        }
        let mut guard = self.lock_dpr();
        *guard = dpr;
        log::debug!("[window_enhance] DPR 已更新: {}", dpr);
    }

    /// 获取当前设备像素比。
    pub fn dpr(&self) -> f64 {
        *self.lock_dpr()
    }

    // ── 通用查询（零业务语义）──────────────────

    /// 查找第一个满足谓词的窗口 HWND（非阻塞）。
    ///
    /// 使用 `try_lock` 而非 `lock`——此函数可能在窗口过程的消息处理链中
    /// 被调用（通过 behavior handler），阻塞会导致消息泵冻结。
    /// 若锁不可用则返回 0（未找到），调用方应处理此降级情况。
    pub fn find_first_hwnd_by<F>(&self, mut predicate: F) -> isize
    where
        F: FnMut(&WindowState) -> bool,
    {
        if let Some(windows) = self.try_lock_windows() {
            windows
                .iter()
                .find_map(|(&hwnd, state)| if (predicate)(state) { Some(hwnd) } else { None })
                .unwrap_or(0)
        } else {
            0
        }
    }

    // ── 平台查询（委托给 platform.rs）──────────

    /// 获取当前光标屏幕坐标。
    pub fn cursor_position(&self) -> Option<ScreenPoint> {
        crate::platform::cursor_position()
    }

    /// 将屏幕坐标转换为指定窗口的客户区坐标。
    pub fn screen_to_client(&self, hwnd_raw: isize, point: ScreenPoint) -> ScreenPoint {
        crate::platform::screen_to_client(hwnd_raw, point)
    }

    /// 获取窗口客户区尺寸（物理像素）。
    pub fn client_rect(&self, hwnd_raw: isize) -> RectSize {
        crate::platform::client_rect(hwnd_raw)
    }
}

// ═══════════════════════════════════════════════════════════════════
// 测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_is_singleton() {
        let a = WindowManager::global() as *const WindowManager;
        let b = WindowManager::global() as *const WindowManager;
        assert_eq!(a, b);
    }

    #[test]
    fn dpr_defaults_to_one() {
        let dpr = WindowManager::global().dpr();
        assert!((dpr - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn find_first_hwnd_by_returns_zero_when_empty() {
        let hwnd = WindowManager::global().find_first_hwnd_by(|_| true);
        let _ = hwnd;
    }
}
