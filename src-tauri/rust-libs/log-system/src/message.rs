use chrono::Local;
use log::Level;

/// 日志消息。
///
/// 包含等级、时间戳、内容以及可选的调用位置。
/// 位置信息通常由上层（如 [`LogHandle`] 或错误库）通过 `#[track_caller]` 捕获后传入。
#[derive(Debug, Clone)]
pub struct LogMessage {
    pub level: Level,
    pub timestamp: String,
    pub content: String,
    /// 调用位置，格式如 `[src/main.rs:42]`。低优先级日志通常为 `None`。
    pub location: Option<String>,
}

impl LogMessage {
    /// 创建一条新的日志消息。
    pub fn new(level: Level, content: String, location: Option<String>) -> Self {
        Self {
            level,
            timestamp: Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
            content,
            location,
        }
    }

    /// 将消息格式化为单行字符串，用于写入文件。
    pub fn formatted(&self) -> String {
        if let Some(loc) = &self.location {
            format!(
                "{} [{}] {} {}",
                self.timestamp, self.level, loc, self.content
            )
        } else {
            format!("{} [{}] {}", self.timestamp, self.level, self.content)
        }
    }
}
