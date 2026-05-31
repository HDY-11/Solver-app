use crossbeam_channel::{SendError, Sender, TrySendError};
use std::sync::mpsc;
use std::thread::JoinHandle;

use crate::message::LogMessage;
use crate::worker::ControlCommand;

// ==================== 轻量写入句柄（可克隆） ====================

/// 高优先级日志写入句柄。
///
/// 此句柄可安全克隆，并可在多线程间传递。用于：
/// - 写入高优先级日志（自动捕获调用位置）
/// - 手动刷新高优先级文件
/// - 清空高优先级文件
/// - panic 时紧急抢救低优先级日志
#[derive(Clone)]
pub struct LogHandle {
    high_tx: Sender<LogMessage>,
    ctrl_tx: Sender<ControlCommand>,
}

impl LogHandle {
    pub(crate) fn new(high_tx: Sender<LogMessage>, ctrl_tx: Sender<ControlCommand>) -> Self {
        Self { high_tx, ctrl_tx }
    }

    /// 发送一条高优先级日志（Info 级别），阻塞直到有空间。
    ///
    /// 调用位置会自动捕获并附加到日志中。
    #[track_caller]
    pub fn log(&self, content: impl Into<String>) -> Result<(), SendError<LogMessage>> {
        let loc = std::panic::Location::caller();
        let location = format!("[{}:{}]", loc.file(), loc.line());
        let msg = LogMessage::new(log::Level::Info, content.into(), Some(location));
        self.high_tx.send(msg)
    }

    /// 非阻塞发送，通道满时立即返回错误。
    #[track_caller]
    pub fn try_log(&self, content: impl Into<String>) -> Result<(), TrySendError<LogMessage>> {
        let loc = std::panic::Location::caller();
        let location = format!("[{}:{}]", loc.file(), loc.line());
        let msg = LogMessage::new(log::Level::Info, content.into(), Some(location));
        self.high_tx.try_send(msg)
    }

    /// 自定义日志等级的阻塞发送。
    #[track_caller]
    pub fn log_with_level(
        &self,
        level: log::Level,
        content: impl Into<String>,
    ) -> Result<(), SendError<LogMessage>> {
        let loc = std::panic::Location::caller();
        let location = format!("[{}:{}]", loc.file(), loc.line());
        let msg = LogMessage::new(level, content.into(), Some(location));
        self.high_tx.send(msg)
    }

    /// 手动刷新高优先级日志文件（将缓冲区数据写入操作系统）。
    pub fn flush(&self) -> Result<(), SendError<ControlCommand>> {
        self.ctrl_tx.send(ControlCommand::FlushHigh)
    }

    /// 清空高优先级日志文件（先刷新，再截断，最后同步到磁盘）。
    pub fn clear(&self) -> Result<(), SendError<ControlCommand>> {
        self.ctrl_tx.send(ControlCommand::ClearHigh)
    }

    /// 紧急排空低优先级日志（供 panic hook 调用）。
    ///
    /// 此方法会阻塞直到后台线程完成排空与刷盘，或超时 3 秒。
    pub fn emergency_flush_low(&self) -> Result<(), SendError<ControlCommand>> {
        let (tx, rx) = mpsc::channel();
        self.ctrl_tx.send(ControlCommand::EmergencyFlushLow(tx))?;
        // 等待后台线程处理完成，最多 3 秒，防止 panic 时无限阻塞
        let _ = rx.recv_timeout(std::time::Duration::from_secs(3));
        Ok(())
    }
}

// ==================== 控制句柄（不可克隆） ====================

/// 日志系统控制句柄。
///
/// **此类型不可克隆**，且应当在整个程序中仅存在一份。
/// 其唯一目的是在程序退出前调用 [`LogCtrl::shutdown`]，以确保后台线程被正确终止并刷盘。
pub struct LogCtrl {
    high_tx: Sender<LogMessage>,
    low_tx: Sender<LogMessage>,
    ctrl_tx: Sender<ControlCommand>,
    worker_handle: Option<JoinHandle<()>>,
}

impl LogCtrl {
    pub(crate) fn new(
        high_tx: Sender<LogMessage>,
        low_tx: Sender<LogMessage>,
        ctrl_tx: Sender<ControlCommand>,
        worker_handle: JoinHandle<()>,
    ) -> Self {
        Self {
            high_tx,
            low_tx,
            ctrl_tx,
            worker_handle: Some(worker_handle),
        }
    }

    /// 优雅关闭日志系统。
    ///
    /// 该方法会：
    /// 1. 向后台线程发送 [`ControlCommand::Shutdown`] 命令。
    /// 2. 等待后台线程结束（`join`）。
    pub fn shutdown(&mut self) {
        eprintln!("[LogCtrl] 发送 Shutdown 命令...");

        match self.ctrl_tx.send(ControlCommand::Shutdown) {
            Ok(_) => eprintln!("[LogCtrl] Shutdown 命令已发送"),
            Err(e) => eprintln!("[LogCtrl] 发送 Shutdown 失败: {:?}", e),
        }

        eprintln!("[LogCtrl] 等待后台线程结束...");
        if let Some(handle) = self.worker_handle.take() {
            match handle.join() {
                Ok(_) => eprintln!("[LogCtrl] 后台线程已结束"),
                Err(e) => eprintln!("[LogCtrl] 后台线程 panic: {:?}", e),
            }
        } else {
            eprintln!("[LogCtrl] worker_handle 为 None！");
        }

        eprintln!("[LogCtrl] shutdown 完成");
    }
}

impl Drop for LogCtrl {
    fn drop(&mut self) {
        eprintln!("[LogCtrl] Drop 被调用，自动关闭日志系统");
        // 只要 worker_handle 还存在，就执行 shutdown
        let should_shutdown = (&self).worker_handle.is_some();
        if should_shutdown.clone() {
            self.shutdown();
        }
    }
}
