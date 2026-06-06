//! # tauri-plugin-window-enhance — 窗口增强系统（依赖倒置架构）
//!
//! ## 架构哲学：Hook 注入模式
//!
//! 插件层定义**机制**（消息拦截与调度），上层注入**策略**（消息处理器）：
//!
//! - **插件层**：拦截 Windows 消息、管理命中区域、调度已注册的消息处理器。
//!   通过 [`WindowBehavior`] trait 定义处理器签名，不包含任何业务逻辑。
//! - **上层**：实现消息处理器（如拖拽合并检测），
//!   通过 `commands::register(hwnd, flags, Box::new(my_handler))` 注入到消息链。
//!
//! ## 模块地图
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────┐
//! │ trait.rs         WindowBehavior 处理器签名 + BehaviorError │
//! │ behaviors.rs     HookBehaviors bitflags（兴趣声明）       │
//! ├──────────────────────────────────────────────────────────┤
//! │ platform.rs      平台抽象（Windows / no-op）              │
//! │ window_proc.rs   统一窗口过程 + 命中测试 + 三层安全防护   │
//! │ manager.rs       WindowManager 门面（Hook 注册 + 查询）   │
//! │ state.rs         WindowState 运行时状态                   │
//! │ commands.rs      公共 API（非 Tauri 命令）                │
//! └──────────────────────────────────────────────────────────┘
//! ```
//!
//! ## 依赖方向
//!
//! ```text
//! 上层 (src-tauri/src/lib.rs)
//!   │ 实现 WindowBehavior 消息处理器
//!   │ 调用 commands::register(hwnd, behaviors, Box::new(handler))
//!   ▼
//! 插件层 (window_enhance)
//!   │ 定义 WindowBehavior trait（处理器签名）
//!   │ 提供消息拦截与处理器调度机制
//!   ▼
//! 平台层 (platform.rs)
//!   │ Windows: GetCursorPos / ScreenToClient / GetClientRect
//!   │ 其他:    no-op 占位
//! ```

use tauri::plugin::{Builder, TauriPlugin};

pub mod behaviors;
pub mod commands;
#[path = "trait.rs"]
pub mod window_behavior;
pub mod platform;
pub mod state;

#[cfg(target_os = "windows")]
pub mod manager;
#[cfg(target_os = "windows")]
pub mod window_proc;

// ── 便捷 re-export ──

pub use behaviors::HookBehaviors;
pub use window_behavior::{BehaviorError, NoopWindowBehavior, WindowBehavior};

#[cfg(target_os = "windows")]
pub use manager::WindowManager;
#[cfg(target_os = "windows")]
pub use platform::{ScreenPoint, RectSize};

// ═══════════════════════════════════════════════════════════════════
// 插件初始化
// ═══════════════════════════════════════════════════════════════════

/// 初始化 window_enhance 插件。
///
/// 插件**不做任何自动注册**——所有窗口（包括主窗口）由前端通过
/// `register_window` 命令主动通知 label，后端解析 HWND 后注册。
///
/// 插件**不注册 Tauri 命令**——命令由上层（`src-tauri/src/lib.rs`）定义，
/// 通过调用本插件导出的公共函数组合业务逻辑。
pub fn init() -> TauriPlugin<tauri::Wry> {
    Builder::new("window_enhance")
        .build()
}
