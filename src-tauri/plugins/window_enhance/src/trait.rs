//! trait.rs — 依赖倒置边界：Hook 注入接口
//!
//! ## 架构角色
//!
//! [`WindowBehavior`] 是插件层定义的**消息处理器签名**。
//! 业务层通过实现此 trait 来声明"我对哪些窗口消息感兴趣，以及如何处理"，
//! 然后通过 [`WindowManager::register`] 注入到窗口消息钩子链中。
//!
//! 这类似**事件监听器/观察者模式**——业务层注册 handler，
//! 插件层在消息到达时调用已注册的 handler。
//!
//! ## 跨平台设计
//!
//! 方法签名使用平台无关的原始类型（`isize`、`i32`），
//! 不暴露 `windows` crate 的任何类型。

use std::fmt;

// ═══════════════════════════════════════════════════════════════════
// BehaviorError — 结构化错误类型
// ═══════════════════════════════════════════════════════════════════

/// 消息处理器返回的错误。
///
/// 使用枚举而非 `Box<dyn Error>` 的理由：
/// 1. **精确分类**：调用方按变体选择恢复策略（业务错误可降级，系统错误需告警）
/// 2. **无堆分配**：不涉及 trait object 虚表查找
/// 3. **穷举匹配**：`match` 编译期保证所有错误路径被处理
#[derive(Debug)]
pub enum BehaviorError {
    /// 业务逻辑错误 — 可安全降级，记录日志后继续转发消息
    Business(String),
    /// 系统调用或外部依赖错误 — 通常不可恢复，需告警
    System(String),
}

impl fmt::Display for BehaviorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BehaviorError::Business(msg) => write!(f, "业务错误: {}", msg),
            BehaviorError::System(msg) => write!(f, "系统错误: {}", msg),
        }
    }
}

impl std::error::Error for BehaviorError {}

impl BehaviorError {
    /// 创建业务逻辑错误（可安全降级）。
    #[inline]
    pub fn business(msg: impl Into<String>) -> Self {
        BehaviorError::Business(msg.into())
    }

    /// 创建系统错误（通常需告警）。
    #[inline]
    pub fn system(msg: impl Into<String>) -> Self {
        BehaviorError::System(msg.into())
    }
}

// ═══════════════════════════════════════════════════════════════════
// WindowBehavior trait — 消息处理器签名
// ═══════════════════════════════════════════════════════════════════

/// 窗口消息处理器签名。
///
/// 业务层实现此 trait 来声明自己对窗口消息的兴趣和处理逻辑，
/// 然后通过 `WindowManager::global().register(hwnd, flags, Box::new(my_handler))`
/// 注入到窗口消息钩子链。
///
/// ## 设计理念：Hook 注入模式
///
/// 你不需要"实现框架规定的接口来满足框架的要求"。
/// 相反，你**主动注册消息处理器**到窗口消息链中：
///
/// 1. 声明兴趣：通过 [`HookBehaviors`] bitflags 告诉插件"我对这些消息感兴趣"
/// 2. 注入处理：实现对应的 trait 方法，注入 trait object
/// 3. 插件调度：消息到达时，插件调用你注册的处理器
///
/// ## 线程安全
///
/// `Send + Sync + RefUnwindSafe + 'static`：
/// - `Send + Sync`：`Arc<dyn WindowBehavior>` 可跨线程共享，存储在 `LazyLock` 全局单例中
/// - `RefUnwindSafe`：`catch_unwind` 包裹时闭包自身满足 `UnwindSafe`（不使用 `AssertUnwindSafe` 滥用）
/// - `'static`：无借用生命周期约束
pub trait WindowBehavior: Send + Sync + 'static {
    /// `WM_NCHITTEST` — 自定义命中测试。
    ///
    /// 插件层内置的 `handle_nchittest` 纯函数在此方法之前执行。
    /// 若内置测试已命中，此方法不会被调用。
    /// 此方法用于实现内置测试无法覆盖的额外命中逻辑。
    ///
    /// ## 参数
    /// - `hwnd`: 窗口 HWND 原始值（`hwnd.0 as isize`）
    /// - `screen_x`: 鼠标屏幕 X 坐标（从 `lParam` 低 16 位解出）
    /// - `screen_y`: 鼠标屏幕 Y 坐标（从 `lParam` 高 16 位解出）
    ///
    /// ## 返回值
    /// - `Ok(Some(lresult))`: 自定义命中，返回该值作为 LRESULT
    /// - `Ok(None)`: 未做决策，消息继续转发
    /// - `Err(_)`: 处理出错，日志记录后消息继续转发
    fn on_nchittest(&self, hwnd: isize, screen_x: i32, screen_y: i32) -> Result<Option<isize>, BehaviorError> {
        let _ = (hwnd, screen_x, screen_y);
        Ok(None)
    }

    /// `WM_ENTERSIZEMOVE` — 窗口拖拽/调整大小开始。
    ///
    /// 仅当窗口的 [`HookBehaviors`] 包含 `DRAG_START` 时调用。
    fn on_drag_start(&self, hwnd: isize) -> Result<(), BehaviorError> {
        let _ = hwnd;
        Ok(())
    }

    /// `WM_EXITSIZEMOVE` — 窗口拖拽/调整大小结束。
    ///
    /// 仅当窗口的 [`HookBehaviors`] 包含 `DRAG_END` 时调用。
    ///
    /// 典型用途：拖拽合并检测——检查光标是否落在主窗口 Nav 区域。
    /// DPR 可通过 `WindowManager::global().dpr()` 获取。
    fn on_drag_end(&self, hwnd: isize) -> Result<(), BehaviorError> {
        let _ = hwnd;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════
// NoopWindowBehavior — 空处理器（便捷类型）
// ═══════════════════════════════════════════════════════════════════

/// 空消息处理器 — 所有方法返回默认值。
///
/// 用于仅需要插件内置能力（如区域命中测试）而无需自定义消息处理的窗口。
///
/// ```ignore
/// // 主窗口：仅需命中测试，无需自定义消息处理
/// manager.register(hwnd, HookBehaviors::NCHITTEST, Box::new(NoopWindowBehavior));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct NoopWindowBehavior;

impl WindowBehavior for NoopWindowBehavior {}
