//! # tauri-plugin-window-enhance — 窗口增强系统
//!
//! ## 提供的能力（不绑定业务）
//!
//! - **窗口子类化**：拦截 `WM_NCHITTEST`，将前端自定义 UI 区域映射为原生标题栏按钮
//!   （最大化/最小化/关闭），同时支持贴靠布局弹出菜单。
//! - **拖拽合并检测**：分离窗口拖拽结束时，通过 `WM_EXITSIZEMOVE` 判断光标是否
//!   落在主窗口 Nav 区域，若是则发射 `drag-release` 事件。
//!
//! ## 架构
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │ 前端 (TypeScript)                                   │
//! │  invoke("register_detached")                        │
//! │  invoke("update_regions", { ... })                  │
//! └──────────────────────┬──────────────────────────────┘
//!                        │ Tauri IPC
//! ┌──────────────────────▼──────────────────────────────┐
//! │ src-tauri/src/lib.rs — #[command] 业务逻辑          │
//! │  调用 plugin 导出的公共函数                          │
//! └──────────────────────┬──────────────────────────────┘
//!                        │
//! ┌──────────────────────▼──────────────────────────────┐
//! │ commands.rs — 公共 API（非 Tauri 命令）              │
//! │  update_regions / register_detached / set_dpr       │
//! └──────────────────────┬──────────────────────────────┘
//!                        │
//! ┌──────────────────────▼──────────────────────────────┐
//! │ manager.rs — WindowManager 门面（Facade）            │
//! │  直接持有 Mutex<HashMap> + Mutex<f64>（无 GlobalState）│
//! │  register(hwnd, kind) 统一入口                       │
//! │  hook 通过 match WindowKind 安装对应窗口过程          │
//! └──────┬──────────────────────────────────────────────┘
//!        │
//! ┌──────▼──────────────────────────────────────────────┐
//! │ subclass.rs — 类型特化窗口过程 + 纯函数               │
//! │  main_window_proc     → 仅 WM_NCHITTEST             │
//! │  detached_window_proc → WM_NCHITTEST + 拖拽合并检测  │
//! │  handle_nchittest      → 纯函数（两种过程共享）       │
//! │  check_cursor_in_nav   → 纯函数（Nav 命中测试）      │
//! └──────┬──────────────────────────────────────────────┘
//!        │
//! ┌──────▼──────────────────────────────────────────────┐
//! │ state.rs — 类型定义                                  │
//! │  WindowKind, WindowState, HashMapExt                 │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! ## 拖拽合并流程
//!
//! ```text
//! 1. 分离窗口挂载 → 前端调用 register_detached()
//! 2. WindowManager::register(Detached) → hook 安装 detached_window_proc
//! 3. 用户拖拽标题栏 → WM_ENTERSIZEMOVE（detached_window_proc 内聚处理）
//! 4. 用户松开鼠标 → WM_EXITSIZEMOVE → try_merge_on_drag_end：
//!    a. GetCursorPos（屏幕坐标）
//!    b. WindowManager::find_main_hwnd（扫描 Main 窗口）
//!    c. ScreenToClient（转主窗口客户区坐标）
//!    d. check_cursor_in_nav（纯函数命中测试）
//!    e. 若命中 → emit!("drag-release") → 前端发起合并
//! ```

use tauri::plugin::{Builder, TauriPlugin};

pub mod commands;
pub mod state;

#[cfg(target_os = "windows")]
pub mod subclass;
#[cfg(target_os = "windows")]
pub mod manager;

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
