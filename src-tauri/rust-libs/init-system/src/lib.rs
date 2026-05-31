//! 应用初始化进度系统
//!
//! 在 Tauri 启动各阶段报告进度，供前端轮询显示真实的进度条。
//! 不依赖事件系统——前端通过 `get_loading_status` 命令轮询。
//!
//! ## 使用
//!
//! ```rust
//! init_system::set_progress(10, "初始化 VFS...");
//! // ... 初始化完成
//! init_system::set_ready(app.handle().clone());
//! ```

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{LazyLock, Mutex};
use serde::Serialize;

static PCT: LazyLock<AtomicU32> = LazyLock::new(|| AtomicU32::new(0));
static MSG: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));

/// 设置当前进度（0-100）
pub fn set_progress(pct: u32, msg: &str) {
    PCT.store(pct.min(100), Ordering::SeqCst);
    if let Ok(mut m) = MSG.lock() {
        *m = msg.to_string();
    }
    eprintln!("[INIT {}%] {}", pct, msg);
}

/// 标记初始化完成，设为 100%
pub fn set_ready() {
    set_progress(100, "就绪");
}

#[derive(Serialize)]
pub struct LoadingStatus {
    pub pct: u32,
    pub msg: String,
}

/// 查询当前进度
pub fn get_loading_status() -> LoadingStatus {
    let msg = MSG.lock().ok().map(|m| m.clone()).unwrap_or_default();
    LoadingStatus {
        pct: PCT.load(Ordering::SeqCst),
        msg,
    }
}
