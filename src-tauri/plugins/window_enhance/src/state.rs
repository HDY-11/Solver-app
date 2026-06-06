//! state.rs — 窗口运行时状态
//!
//! ## 架构角色
//!
//! [`WindowState`] 是单个被子类化窗口的完整运行时状态。
//! 不再包含 `WindowKind` 枚举——行为由 [`HookBehaviors`] + [`WindowBehavior`] trait object 联合表达。

use std::sync::Arc;

use crate::behaviors::HookBehaviors;
use crate::window_behavior::WindowBehavior;

// ═══════════════════════════════════════════════════════════════════
// WindowState — 单个窗口的运行时状态
// ═══════════════════════════════════════════════════════════════════

/// 单个被子类化窗口的完整状态。
///
/// 由 [`WindowManager`](super::manager::WindowManager) 内部持有，
/// 以 `hwnd_raw` (isize) 为键存储在 `HashMap` 中。
///
/// ## 字段说明
///
/// | 字段 | 类型 | 说明 |
/// |------|------|------|
/// | `original_proc` | `isize` | 原始窗口过程地址，0 表示无效 |
/// | `regions` | `Vec<(i32,i32,i32,i32,String)>` | 自定义标题栏区域（物理像素） |
/// | `behaviors` | `HookBehaviors` | 行为标志集（位组合） |
/// | `behavior` | `Arc<dyn WindowBehavior>` | 注入的消息处理器 |
///
/// ## Clone 语义
///
/// `Arc::clone` 仅增加引用计数（O(1) 原子操作）。
/// `regions` clone 为 O(n)，n 在实际使用中 < 10。
/// 窗口过程通过 clone 获取快照后释放锁，防止嵌套消息死锁。
#[derive(Clone)]
pub struct WindowState {
    /// 原始窗口过程地址。
    ///
    /// 0 表示窗口从未注册或状态丢失——此时消息降级到 `DefWindowProcW`。
    pub original_proc: isize,

    /// 自定义标题栏区域列表：`(x, y, width, height, kind)`。
    ///
    /// 坐标均为物理像素，相对于窗口客户区左上角。
    /// `kind` 为 `"maxbutton"` / `"minbutton"` / `"closebutton"` / 其他（视为拖拽区）。
    pub regions: Vec<(i32, i32, i32, i32, String)>,

    /// 行为标志集 — 声明此窗口需要拦截的消息类型。
    pub behaviors: HookBehaviors,

    /// 注入的消息处理器。
    ///
    /// `Arc` 包装允许 O(1) clone（原子引用计数），
    /// 跨线程共享（`Send + Sync`），以及 `catch_unwind` 安全传递（`RefUnwindSafe`）。
    pub behavior: Arc<dyn WindowBehavior>,
}

impl WindowState {
    /// 创建新的窗口状态。
    pub fn new(
        original_proc: isize,
        behaviors: HookBehaviors,
        behavior: Box<dyn WindowBehavior>,
    ) -> Self {
        Self {
            original_proc,
            regions: Vec::new(),
            behaviors,
            behavior: Arc::from(behavior),
        }
    }
}
