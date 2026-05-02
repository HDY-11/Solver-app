//! # 双通道日志系统
//!
//! 本库提供了一个生产级日志系统，核心特性如下：
//!
//! - **双优先级通道**：
//!   - **高优先级**：通过 [`LogHandle`] 手动写入，独立文件，支持手动刷新与清空。
//!   - **低优先级**：通过标准 [`log`] 宏自动写入，程序退出时统一刷盘，panic 时紧急抢救。
//! - **水位排空**：低优先级通道积压达到 90% 容量时触发批量写入，排至 70% 停止。
//! - **线程安全**：后台 I/O 线程与调用方完全解耦，使用 `crossbeam-channel` 实现无锁通信。
//! - **优雅关闭**：通过 [`LogCtrl`] 显式关闭，确保所有日志完整落盘。
//! - **位置追踪**：高优先级日志自动捕获调用位置（`#[track_caller]`）。
//!
//! ## 快速开始
//!
//! ```no_run
//! use log_system::init_logging;
//!
//! fn main() {
//!     // 初始化日志系统，返回控制句柄和写入句柄
//!     let (log_ctrl, log_handle) = init_logging(
//!         "./logs/high.log",
//!         "./logs/low.log",
//!         4096, // 低优先级通道容量
//!     ).expect("日志系统初始化失败");
//!
//!     // 设置 panic 钩子，抢救低优先级日志
//!     let panic_handle = log_handle.clone();
//!     std::panic::set_hook(Box::new(move |info| {
//!         let _ = panic_handle.emergency_flush_low();
//!         let _ = panic_handle.flush();
//!         eprintln!("Panic: {}", info);
//!     }));
//!
//!     // 使用标准 log 宏写入低优先级日志
//!     log::info!("应用启动");
//!
//!     // 使用句柄写入高优先级日志
//!     log_handle.log("用户点击保存");
//!
//!     // 程序退出前，必须调用 shutdown 确保日志完整
//!     log_ctrl.shutdown();
//! }
//! ```

mod handle;
mod logger;
mod message;
mod worker;

pub use handle::{LogCtrl, LogHandle};
pub use logger::{Logger};
pub use message::LogMessage;

use std::path::PathBuf;
use logger::GLOBAL_LOGGER;

/// 初始化日志系统。
///
/// # 参数
/// - `high_log_path`: 高优先级日志文件路径。
/// - `low_log_path`: 低优先级日志文件路径。
/// - `low_channel_capacity`: 低优先级通道容量（用于水位计算，建议 ≥ 1024）。
///
/// # 返回值
/// 返回一个元组 `(LogCtrl, LogHandle)`：
/// - [`LogCtrl`]：不可克隆的控制句柄，用于调用 [`LogCtrl::shutdown`] 关闭系统。
/// - [`LogHandle`]：可克隆的轻量句柄，用于写入高优先级日志、刷新、清空等操作。
pub fn init_logging(
    high_log_path: impl Into<PathBuf>,
    low_log_path: impl Into<PathBuf>,
    low_channel_capacity: usize,
) -> std::io::Result<(LogCtrl, LogHandle)> {
    let (logger, ctrl, handle) = Logger::init(
        high_log_path.into(),
        low_log_path.into(),
        low_channel_capacity,
    )?;

    // 注册全局 Logger，供 `log` 宏使用
    GLOBAL_LOGGER
        .set(logger)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "logger already set"))?;

    log::set_logger(GLOBAL_LOGGER.get().unwrap())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

    // 扩展点：可配置日志级别过滤
    log::set_max_level(log::LevelFilter::Debug);

    Ok((ctrl, handle))
}
