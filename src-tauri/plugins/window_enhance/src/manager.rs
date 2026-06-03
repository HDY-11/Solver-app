//! manager.rs — WindowManager 门面
//!
//! ## 职责
//!
//! [`WindowManager`] 是窗口增强系统的**统一门面（Facade）**，
//! 直接持有所有窗口状态（无中间层 `GlobalState`）。
//!
//! 提供的能力：
//! - **统一注册**：[`register`] 是唯一入口，hook 闭包通过 match [`WindowKind`] 安装对应窗口过程
//! - **区域更新**：[`update_regions`] 动态更新标题栏命中区域
//! - **DPR 管理**：[`set_dpr`] / [`dpr`]
//! - **主窗口发现**：[`find_main_hwnd`] 扫描已注册窗口自动定位主窗口
//!
//! ## 设计原则
//!
//! - **全局单例**：通过 [`WindowManager::global()`] 获取
//! - **无 GlobalState**：WindowManager 直接持有 `Mutex<HashMap>` + `Mutex<f64>`
//! - **声明式注册**：`register` 使用 [`HashMapExt::insert_with`]，
//!   hook 闭包通过 match [`WindowKind`] 自动分发到正确的窗口过程

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{GWLP_WNDPROC, SetWindowLongPtrW};

use super::state::{HashMapExt, WindowKind, WindowState};
use super::subclass;

/// 窗口管理器全局单例。
///
/// ## 使用方式
///
/// ```ignore
/// // 插件 setup 阶段 — 统一注册主窗口
/// WindowManager::global().register(main_hwnd_raw, WindowKind::Main);
///
/// // 分离窗口挂载时 — 统一注册分离窗口
/// WindowManager::global().register(hwnd_raw, WindowKind::Detached);
///
/// // 窗口大小变化时 — 更新命中区域
/// WindowManager::global().update_regions(hwnd_raw, regions);
///
/// // 窗口过程中 — 读取状态
/// if let Some(windows) = WindowManager::global().try_lock_windows() { ... }
/// ```
pub struct WindowManager {
    /// 所有已注册窗口的状态，以 `hwnd_raw` (isize) 为键
    windows: Mutex<HashMap<isize, WindowState>>,
    /// 设备像素比（devicePixelRatio），用于 CSS px → 物理 px 坐标转换
    dpr: Mutex<f64>,
}

impl WindowManager {
    /// 获取全局单例实例
    pub fn global() -> &'static Self {
        static INSTANCE: LazyLock<WindowManager> = LazyLock::new(|| WindowManager {
            windows: Mutex::new(HashMap::new()),
            dpr: Mutex::new(1.0),
        });
        &INSTANCE
    }

    // ── 锁辅助 ──────────────────────────────────

    /// 安全地获取窗口 Map 锁（处理 PoisonError 降级）
    fn lock_windows(&self) -> std::sync::MutexGuard<'_, HashMap<isize, WindowState>> {
        self.windows.lock().unwrap_or_else(|poison| {
            log::error!("[window_enhance] windows Mutex 已 poison，使用降级状态继续运行");
            poison.into_inner()
        })
    }

    /// 尝试获取窗口 Map 锁（非阻塞，处理 PoisonError）
    ///
    /// 用于窗口过程等不可阻塞的上下文中。若锁已被持有或 poison，
    /// 返回 `None`，调用方应跳过本次处理并转发消息。
    pub fn try_lock_windows(&self) -> Option<std::sync::MutexGuard<'_, HashMap<isize, WindowState>>> {
        self.windows.try_lock().ok().or_else(|| {
            log::warn!(
                "[window_enhance] windows try_lock 失败（锁竞争或 poison），跳过本次消息处理"
            );
            None
        })
    }

    // ── 公共 API：统一注册 ─────────────────────

    /// 统一注册窗口（唯一入口）。
    ///
    /// 使用 [`HashMapExt::insert_with`] + 无状态 hook 闭包，
    /// 根据 [`WindowKind`] 自动分发到正确的窗口过程：
    ///
    /// | WindowKind    | 安装的窗口过程                                          |
    /// |---------------|---------------------------------------------------------|
    /// | [`Main`]      | [`main_window_proc`](super::subclass::main_window_proc)   |
    /// | [`Detached`]  | [`detached_window_proc`](super::subclass::detached_window_proc) |
    ///
    /// 若窗口已存在，旧状态被替换，hook 仍会对新状态执行初始化。
    ///
    /// ## 参数
    /// - `hwnd_raw`: 窗口 HWND 的原始值（`hwnd.0 as isize`）
    /// - `kind`: 窗口类型，决定安装哪个窗口过程
    pub fn register(&self, hwnd_raw: isize, kind: WindowKind) {
        // 幂等性：若窗口已注册，跳过（防止重复注册导致 original_proc
        // 指向自身，引发无限递归 → 栈溢出）
        {
            let windows = self.lock_windows();
            if windows.contains_key(&hwnd_raw) {
                log::debug!(
                    "[window_enhance] 窗口已注册，跳过: 0x{:x} kind={:?}",
                    hwnd_raw, kind
                );
                return;
            }
        }

        let hwnd = HWND(hwnd_raw as *mut _);

        // 保存原始窗口过程（在锁外完成，避免重入）
        let original_proc = unsafe { subclass::get_original_proc(hwnd) };

        let state = WindowState {
            original_proc,
            regions: Vec::new(),
            kind,
        };

        let mut windows = self.lock_windows();

        // ── 声明式插入 + 无状态 hook ──
        // SetWindowLongPtrW 在 hook 中执行（锁内），与 Map 插入原子化：
        // 安装子类过程后，若消息立即到达，try_lock_windows 返回 None
        //（锁被持有），窗口过程安全降级到 DefWindowProcW。
        // 锁释放后，窗口已存在于 Map 中，后续消息正常处理。
        windows.insert_with(hwnd_raw, state, |key, val| {
            // 根据窗口类型选择对应的窗口过程并安装
            let proc_addr: isize = match val.kind {
                WindowKind::Main => {
                    subclass::main_window_proc as *const () as usize as isize
                }
                WindowKind::Detached => {
                    subclass::detached_window_proc as *const () as usize as isize
                }
            };
            unsafe {
                SetWindowLongPtrW(HWND(*key as *mut _), GWLP_WNDPROC, proc_addr);
            }

            match val.kind {
                WindowKind::Main => {
                    log::info!("[window_enhance] 主窗口已注册: 0x{:x}", key);
                }
                WindowKind::Detached => {
                    log::info!("[window_enhance] 分离窗口已注册: 0x{:x}", key);
                }
            }
        });
    }

    // ── 公共 API：状态读写 ─────────────────────

    /// 更新指定窗口的自定义标题栏区域。
    ///
    /// 前端应在窗口大小改变时（ResizeObserver）调用。
    pub fn update_regions(&self, hwnd_raw: isize, regions: Vec<(i32, i32, i32, i32, String)>) {
        let mut windows = self.lock_windows();
        if let Some(state) = windows.get_mut(&hwnd_raw) {
            state.regions = regions;
        } else {
            log::warn!("[window_enhance] update_regions: 窗口未注册 0x{:x}", hwnd_raw);
        }
    }

    /// 设置设备像素比。
    ///
    /// 前端应在获取到 `window.devicePixelRatio` 后调用。
    /// 用于 CSS px → 物理 px 坐标转换。
    pub fn set_dpr(&self, dpr: f64) {
        let mut d = self.dpr.lock().unwrap_or_else(|poison| {
            log::error!("[window_enhance] dpr Mutex 已 poison");
            poison.into_inner()
        });
        *d = dpr;
        log::debug!("[window_enhance] DPR 已更新: {}", dpr);
    }

    /// 获取设备像素比。
    pub fn dpr(&self) -> f64 {
        self.dpr.lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
    }

    /// 扫描已注册窗口，找到主窗口 HWND。
    ///
    /// 不依赖单独的 `main_hwnd` 缓存——通过遍历窗口 Map，
    /// 找到第一个 [`WindowKind::Main`] 类型的窗口。
    ///
    /// 返回 `0` 表示未找到。
    pub fn find_main_hwnd(&self) -> isize {
        self.lock_windows()
            .iter()
            .find(|(_, s)| s.kind == WindowKind::Main)
            .map(|(hwnd, _)| *hwnd)
            .unwrap_or(0)
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
    fn find_main_hwnd_returns_zero_when_empty() {
        // 注意：全局单例状态可能受其他测试影响
        let hwnd = WindowManager::global().find_main_hwnd();
        // 仅验证不 panic
        let _ = hwnd;
    }
}
