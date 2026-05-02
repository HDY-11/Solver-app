use crossbeam_channel::{Receiver, select};
use std::io::Result as IoResult;
use std::io::Write;
use std::path::PathBuf;

use env_system::{RotatingLogFile};

use crate::message::LogMessage;

/// 控制命令，通过专用通道发送给后台工作线程。
#[derive(Debug)]
pub enum ControlCommand {
    /// 刷新高优先级文件（仅 `flush`，不调用 `sync_all`）。
    FlushHigh,
    /// 清空高优先级文件：先 `flush`，再截断文件，最后 `sync_all`。
    ClearHigh,
    /// 紧急排空低优先级日志：排空通道 + `flush` + `sync_all`。
    /// 完成后通过 `std::sync::mpsc::Sender` 通知调用方。
    EmergencyFlushLow(std::sync::mpsc::Sender<()>),
    /// 优雅关闭：通知线程立即退出主循环，执行最终的排空与刷盘。
    Shutdown,
}

/// 后台日志工作线程。
///
/// 负责从三个通道（高优先级、低优先级、控制命令）读取数据，并将日志写入对应的文件。
/// 所有 I/O 操作均在此线程中串行执行，避免了锁竞争。
pub struct LogWorker {
    high_rx: Receiver<LogMessage>,
    low_rx: Receiver<LogMessage>,
    ctrl_rx: Receiver<ControlCommand>,

    high_file: RotatingLogFile,
    low_file: RotatingLogFile,

    /// 低优先级通道的高水位阈值（90% 容量）。
    high_watermark: usize,
    /// 低优先级通道的低水位阈值（70% 容量）。
    low_watermark: usize,
}

impl LogWorker {
    /// 创建新的工作线程实例。
    ///
    /// # 参数
    /// - `high_rx`, `low_rx`, `ctrl_rx`: 三个通道的接收端。
    /// - `high_log_path`, `low_log_path`: 日志文件路径（父目录不存在时将自动创建）。
    /// - `low_capacity`: 低优先级通道容量，用于计算水位阈值。
    pub fn new(
        high_rx: Receiver<LogMessage>,
        low_rx: Receiver<LogMessage>,
        ctrl_rx: Receiver<ControlCommand>,
        high_log_path: PathBuf,
        low_log_path: PathBuf,
        low_capacity: usize,
    ) -> std::io::Result<Self> {
        // 确保日志文件的父目录存在
        if let Some(parent) = high_log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if let Some(parent) = low_log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let high_file = RotatingLogFile::new(&high_log_path, 5, 8192)?;
        let low_file = RotatingLogFile::new(&low_log_path, 5, 8192)?;

        // 水位：容量 90% 触发排空，排至 70% 停止
        let high_watermark = low_capacity * 90 / 100;
        let low_watermark = low_capacity * 70 / 100;

        Ok(Self {
            high_rx,
            low_rx,
            ctrl_rx,
            high_file,
            low_file,
            high_watermark,
            low_watermark,
        })
    }

    /// 启动工作循环。
    ///
    /// 该方法应在独立的线程中调用（例如 `std::thread::spawn`）。
    /// 它会持续运行，直到收到 [`ControlCommand::Shutdown`] 或低优先级通道被完全关闭。
    pub fn run(mut self) {
        const MAX_LOW_BATCH_SIZE: usize = 200;
        let thread_id = std::thread::current().id();
        eprintln!("[LogWorker-{:?}] 线程启动", thread_id);

        loop {
            select! {
                recv(self.high_rx) -> msg => {
                    match msg {
                        Ok(msg) => {
                            eprintln!("[LogWorker-{:?}] 收到高优先级: {}", thread_id, msg.content);
                            self.process_high_batch(msg);
                        }
                        Err(_) => {
                            eprintln!("[LogWorker-{:?}] 高优先级通道断开", thread_id);
                        }
                    }
                }
                recv(self.low_rx) -> msg => {
                    match msg {
                        Ok(msg) => {
                            eprintln!("[LogWorker-{:?}] 收到低优先级: {}", thread_id, msg.content);
                            if let Err(e) = self.write_low(&msg) {
                                eprintln!("[LogWorker-{:?}] 低优先级写入失败: {}", thread_id, e);
                            }
                            if self.low_rx.len() >= self.high_watermark {
                                eprintln!("[LogWorker-{:?}] 触发水位排空 (len={})", thread_id, self.low_rx.len());
                                self.drain_low_to_watermark(MAX_LOW_BATCH_SIZE);
                            }
                        }
                        Err(_) => {
                            eprintln!("[LogWorker-{:?}] 低优先级通道断开，准备退出循环", thread_id);
                            break;
                        }
                    }
                }
                recv(self.ctrl_rx) -> cmd => {
                    match cmd {
                        Ok(ControlCommand::FlushHigh) => {
                            eprintln!("[LogWorker-{:?}] 收到 FlushHigh", thread_id);
                            if let Err(e) = self.high_file.lend_writer().flush() {
                                eprintln!("[LogWorker-{:?}] FlushHigh 失败: {}", thread_id, e);
                            }
                        }
                        Ok(ControlCommand::ClearHigh) => {
                            eprintln!("[LogWorker-{:?}] 收到 ClearHigh", thread_id);
                            if let Err(e) = self.clear_high_file() {
                                eprintln!("[LogWorker-{:?}] ClearHigh 失败: {}", thread_id, e);
                            }
                        }
                        Ok(ControlCommand::EmergencyFlushLow(tx)) => {
                            eprintln!("[LogWorker-{:?}] 收到 EmergencyFlushLow", thread_id);
                            self.drain_low_all();
                            let _ = self.low_file.lend_writer().flush();
                            let _ = self.low_file.lend_writer().get_ref().sync_all();
                            let _ = tx.send(());
                            eprintln!("[LogWorker-{:?}] EmergencyFlushLow 完成", thread_id);
                        }
                        Ok(ControlCommand::Shutdown) => {
                            eprintln!("[LogWorker-{:?}] 收到 Shutdown，退出循环", thread_id);
                            break;
                        }
                        Err(_) => {
                            eprintln!("[LogWorker-{:?}] 控制通道断开", thread_id);
                        }
                    }
                }
            }
        }

        eprintln!("[LogWorker-{:?}] 退出主循环，开始清理...", thread_id);

        // 排空低优先级通道
        let before_drain = self.low_rx.len();
        self.drain_low_all();
        eprintln!(
            "[LogWorker-{:?}] 排空低优先级: {} 条",
            thread_id, before_drain
        );

        // 刷盘
        eprintln!("[LogWorker-{:?}] 刷新高优先级...", thread_id);
        match self.high_file.lend_writer().flush() {
            Ok(_) => eprintln!("[LogWorker-{:?}] 高优先级 flush 成功", thread_id),
            Err(e) => eprintln!("[LogWorker-{:?}] 高优先级 flush 失败: {}", thread_id, e),
        }

        eprintln!("[LogWorker-{:?}] 刷新低优先级...", thread_id);
        match self.low_file.lend_writer().flush() {
            Ok(_) => eprintln!("[LogWorker-{:?}] 低优先级 flush 成功", thread_id),
            Err(e) => eprintln!("[LogWorker-{:?}] 低优先级 flush 失败: {}", thread_id, e),
        }

        // 同步到磁盘
        eprintln!("[LogWorker-{:?}] 同步高优先级到磁盘...", thread_id);
        let _ = self.high_file.lend_writer().get_ref().sync_all();
        eprintln!("[LogWorker-{:?}] 同步低优先级到磁盘...", thread_id);
        let _ = self.low_file.lend_writer().get_ref().sync_all();

        eprintln!("[LogWorker-{:?}] 线程退出", thread_id);
    }

    /// 批量处理高优先级消息（包括第一条以及后续可立即获取的）。
    ///
    /// 高优先级日志**不会**自动 `flush`，必须由用户通过 [`LogHandle::flush`] 手动触发。
    fn process_high_batch(&mut self, first: LogMessage) {
        let _ = self.write_high(&first);
        while let Ok(msg) = self.high_rx.try_recv() {
            let _ = self.write_high(&msg);
        }
    }

    /// 从低优先级通道取消息，直到达到低水位或超过单次处理上限。
    fn drain_low_to_watermark(&mut self, max_batch: usize) {
        let mut processed = 0;
        while self.low_rx.len() > self.low_watermark && processed < max_batch {
            match self.low_rx.try_recv() {
                Ok(msg) => {
                    let _ = self.write_low(&msg);
                    processed += 1;
                }
                Err(_) => break,
            }
        }
        // 不立即 flush，留给后续批次或退出时处理，以减少系统调用。
    }

    /// 排空低优先级通道中所有剩余消息。
    fn drain_low_all(&mut self) {
        while let Ok(msg) = self.low_rx.try_recv() {
            let _ = self.write_low(&msg);
        }
    }

    fn clear_high_file(&mut self) -> IoResult<()> {
        self.high_file.clear()
    }

    fn write_high(&mut self, msg: &LogMessage) -> IoResult<()> {
        let mut guard = self.high_file.split();
        guard.writeln(&msg.formatted())
    }

    fn write_low(&mut self, msg: &LogMessage) -> IoResult<()> {
        let mut guard = self.low_file.split();
        guard.writeln(&msg.formatted())
    }

    fn handle_save_high_as(&mut self, name: String) -> IoResult<()> {
        self.high_file.save_as(name)
    }
}
