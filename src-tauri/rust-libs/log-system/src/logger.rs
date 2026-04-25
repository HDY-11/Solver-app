use crossbeam_channel::{Sender, bounded};
use log::{Level, Log, Metadata, Record};
use once_cell::sync::OnceCell;
use std::path::PathBuf;
use std::thread;

use crate::handle::{LogCtrl, LogHandle};
use crate::message::LogMessage;
use crate::worker::{ControlCommand, LogWorker};

/// 全局 [`Logger`] 实例，用于注册到 `log` crate。
pub static GLOBAL_LOGGER: OnceCell<Logger> = OnceCell::new();

/// 实现 `log::Log` trait，将日志消息转发到低优先级通道。
pub struct Logger {
    low_tx: Sender<LogMessage>,
}

impl Logger {
    fn new(low_tx: Sender<LogMessage>) -> Self {
        Self { low_tx }
    }

    /// 初始化日志系统。
    ///
    /// 返回三元组 `(Logger, LogCtrl, LogHandle)`：
    /// - `Logger` 需注册到 `log` crate。
    /// - `LogCtrl` 用于关闭系统。
    /// - `LogHandle` 用于写入高优先级日志及控制操作。
    pub fn init(
        high_log_path: PathBuf,
        low_log_path: PathBuf,
        low_channel_capacity: usize,
    ) -> std::io::Result<(Self, LogCtrl, LogHandle)> {
        const HIGH_CHANNEL_CAP: usize = 1024;
        const CTRL_CHANNEL_CAP: usize = 8;

        // 创建三个通道
        let (high_tx, high_rx) = bounded(HIGH_CHANNEL_CAP);
        let (low_tx, low_rx) = bounded(low_channel_capacity);
        let (ctrl_tx, ctrl_rx) = bounded(CTRL_CHANNEL_CAP);

        let worker = LogWorker::new(
            high_rx,
            low_rx,
            ctrl_rx,
            high_log_path,
            low_log_path,
            low_channel_capacity,
        )?;

        // 启动独立的后台 I/O 线程
        let worker_handle = thread::Builder::new()
            .name("log-worker".to_string())
            .spawn(|| worker.run())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let logger = Self::new(low_tx.clone());

        // LogCtrl 持有所有 Sender 和线程句柄，用于最终关闭
        let ctrl = LogCtrl::new(high_tx.clone(), low_tx, ctrl_tx.clone(), worker_handle);

        // LogHandle 仅持有必要的 Sender，可安全克隆
        let handle = LogHandle::new(high_tx, ctrl_tx);

        Ok((logger, ctrl, handle))
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        // 扩展点：可在此处添加动态级别过滤逻辑
        metadata.level() <= Level::Debug
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            // 低优先级日志不主动捕获位置，位置信息由上层（如错误库）可选提供
            let msg = LogMessage::new(record.level(), record.args().to_string(), None);
            // 使用 try_send 避免阻塞 log 宏调用方；若通道满则丢弃（符合 log 语义）
            let _ = self.low_tx.try_send(msg);
        }
    }

    fn flush(&self) {
        // 刷新由后台线程统一管理，此处不执行任何操作
    }
}
