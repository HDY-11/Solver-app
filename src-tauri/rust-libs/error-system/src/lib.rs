//! 错误处理扩展工具
//!
//! 为 `Result<T, E>` 和 `Option<T>` 提供链式日志记录方法，自动包含调用位置。
//! 所有方法均标注 `#[track_caller]`，日志中会显示准确的调用点。
//!
//! # 核心 API 分类
//!
//! | 方法 | 行为 | 返回值 |
//! |------|------|--------|
//! | `inspect_log(msg)` | 错误时记录 error，继续传播 | `Result<T, E>` |
//! | `inspect_log_warn(msg)` | 错误时记录 warn，继续传播 | `Result<T, E>` |
//! | `unwrap_log()` | 错误时记录 error，返回默认值 | `T` (需 `T: Default`) |
//! | `unwrap_or_log(default)` | 错误时记录 error，返回指定值 | `T` |
//! | `unwrap_warn_log()` | 错误时记录 warn，返回默认值 | `T` (需 `T: Default`) |
//! | `unwrap_warn_or_log(default)` | 错误时记录 warn，返回指定值 | `T` |
//! | `expect_log(msg)` | 错误时记录 error，然后 panic | `T` |
//! | `expect_warn_log(msg)` | 错误时记录 warn，然后 panic | `T` |

use std::path::PathBuf;
use anyhow::Error;
use thiserror::Error;
use std::sync::PoisonError;
/// 自定义错误类型
///
/// # 扩展点
/// 根据业务需求添加具体错误变体
#[derive(Error, Debug)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Channel send error")]
    ChannelSend,

    #[error("Log system not initialized")]
    LogNotInitialized,

    #[error("Event system not initialized")]
    EventNotInitialized,

    #[error("Python error: {0}")]
    Python(#[from] pyo3::PyErr),

    #[error("Python type cast error: {0}")]
    PythonCast(String),

    #[error("Path contains invalid UTF-8: {0}")]
    InvalidPath(PathBuf),

    #[error("Nul byte in script content")]
    NulByte(#[from] std::ffi::NulError),

    #[error("Blocking task join error: {0}")]
    JoinError(String),

    #[error("Mutex poisoned: {0}")]
    PoisonError(String),

    #[error("Other Error happened: {0}")]
    Other(Error)
}

impl AppError {
    /// 从 CastError 创建 AppError
    pub fn from_cast_error(e: &pyo3::CastError<'_, '_>) -> Self {
        AppError::PythonCast(e.to_string())
    }
    pub fn from_poison_error<T>(e: PoisonError<T>) -> Self {
        AppError::PoisonError(e.to_string())
    }
}

impl From<tokio::task::JoinError> for AppError {
    fn from(e: tokio::task::JoinError) -> Self {
        AppError::JoinError(e.to_string())
    }
}
impl From<AppError> for tauri::ipc::InvokeError {
    fn from(e: AppError) -> Self {
        tauri::ipc::InvokeError::from(e.to_string())
    }
}
// ==================== Result 日志扩展 ====================

pub trait ResultLogExt<T, E> {
    /// 错误时记录 `error!`，继续传播错误
    ///
    /// # 示例
    /// ```ignore
    /// let content = std::fs::read_to_string("config.toml")
    ///     .inspect_log("读取配置文件失败")?;
    /// ```
    #[track_caller]
    fn inspect_log(self, context: impl Into<String>) -> Self;

    /// 错误时记录 `warn!`，继续传播错误
    #[track_caller]
    fn inspect_log_warn(self, context: impl Into<String>) -> Self;

    /// 错误时记录 `error!`，返回 `T::default()`（吞掉错误）
    ///
    /// # 示例
    /// ```ignore
    /// let port: u16 = std::env::var("PORT")
    ///     .ok()
    ///     .and_then(|s| s.parse().ok())
    ///     .unwrap_log(); // 失败时返回 0
    /// ```
    #[track_caller]
    fn unwrap_log(self) -> T
    where
        T: Default;

    /// 错误时记录 `error!`，返回指定默认值（吞掉错误）
    #[track_caller]
    fn unwrap_or_log(self, default: T) -> T;

    /// 错误时记录 `warn!`，返回 `T::default()`（吞掉错误）
    #[track_caller]
    fn unwrap_warn_log(self) -> T
    where
        T: Default;

    /// 错误时记录 `warn!`，返回指定默认值（吞掉错误）
    #[track_caller]
    fn unwrap_warn_or_log(self, default: T) -> T;

    /// 错误时记录 `error!`，然后 panic（类似标准库 `expect`）
    #[track_caller]
    fn expect_log(self, context: impl Into<String>) -> T;

    /// 错误时记录 `warn!`，然后 panic
    #[track_caller]
    fn expect_warn_log(self, context: impl Into<String>) -> T;
}

impl<T, E> ResultLogExt<T, E> for Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    #[track_caller]
    fn inspect_log(self, context: impl Into<String>) -> Self {
        if let Err(e) = &self {
            let loc = std::panic::Location::caller();
            log::error!(
                "[{}:{}] {}: {:#}",
                loc.file(),
                loc.line(),
                context.into(),
                e
            );
        }
        self
    }

    #[track_caller]
    fn inspect_log_warn(self, context: impl Into<String>) -> Self {
        if let Err(e) = &self {
            let loc = std::panic::Location::caller();
            log::warn!(
                "[{}:{}] {}: {:#}",
                loc.file(),
                loc.line(),
                context.into(),
                e
            );
        }
        self
    }

    #[track_caller]
    fn unwrap_log(self) -> T
    where
        T: Default,
    {
        match self {
            Ok(v) => v,
            Err(e) => {
                let loc = std::panic::Location::caller();
                log::error!("[{}:{}] {:#}", loc.file(), loc.line(), e);
                T::default()
            }
        }
    }

    #[track_caller]
    fn unwrap_or_log(self, default: T) -> T {
        match self {
            Ok(v) => v,
            Err(e) => {
                let loc = std::panic::Location::caller();
                log::error!("[{}:{}] {:#}", loc.file(), loc.line(), e);
                default
            }
        }
    }

    #[track_caller]
    fn unwrap_warn_log(self) -> T
    where
        T: Default,
    {
        match self {
            Ok(v) => v,
            Err(e) => {
                let loc = std::panic::Location::caller();
                log::warn!("[{}:{}] {:#}", loc.file(), loc.line(), e);
                T::default()
            }
        }
    }

    #[track_caller]
    fn unwrap_warn_or_log(self, default: T) -> T {
        match self {
            Ok(v) => v,
            Err(e) => {
                let loc = std::panic::Location::caller();
                log::warn!("[{}:{}] {:#}", loc.file(), loc.line(), e);
                default
            }
        }
    }

    #[track_caller]
    fn expect_log(self, context: impl Into<String>) -> T {
        match self {
            Ok(v) => v,
            Err(e) => {
                let loc = std::panic::Location::caller();
                let ctx = context.into();
                log::error!("[{}:{}] {}: {:#}", loc.file(), loc.line(), ctx, e);
                panic!("[{}:{}] {}: {:#}", loc.file(), loc.line(), ctx, e);
            }
        }
    }

    #[track_caller]
    fn expect_warn_log(self, context: impl Into<String>) -> T {
        match self {
            Ok(v) => v,
            Err(e) => {
                let loc = std::panic::Location::caller();
                let ctx = context.into();
                log::warn!("[{}:{}] {}: {:#}", loc.file(), loc.line(), ctx, e);
                panic!("[{}:{}] {}: {:#}", loc.file(), loc.line(), ctx, e);
            }
        }
    }
}

// ==================== Option 日志扩展 ====================

pub trait OptionLogExt<T> {
    /// `None` 时记录 `error!`，返回指定默认值
    #[track_caller]
    fn unwrap_or_log(self, default: T) -> T;

    /// `None` 时记录 `error!`，返回 `T::default()`
    #[track_caller]
    fn unwrap_log(self) -> T
    where
        T: Default;

    /// `None` 时记录 `warn!`，返回指定默认值
    #[track_caller]
    fn unwrap_warn_or_log(self, default: T) -> T;

    /// `None` 时记录 `warn!`，返回 `T::default()`
    #[track_caller]
    fn unwrap_warn_log(self) -> T
    where
        T: Default;

    /// `None` 时记录 `error!`，然后 panic
    #[track_caller]
    fn expect_log(self, context: impl Into<String>) -> T;

    /// `None` 时记录 `warn!`，然后 panic
    #[track_caller]
    fn expect_warn_log(self, context: impl Into<String>) -> T;
}

impl<T> OptionLogExt<T> for Option<T> {
    #[track_caller]
    fn unwrap_or_log(self, default: T) -> T {
        match self {
            Some(v) => v,
            None => {
                let loc = std::panic::Location::caller();
                log::error!("[{}:{}] Option was None", loc.file(), loc.line());
                default
            }
        }
    }

    #[track_caller]
    fn unwrap_log(self) -> T
    where
        T: Default,
    {
        match self {
            Some(v) => v,
            None => {
                let loc = std::panic::Location::caller();
                log::error!("[{}:{}] Option was None", loc.file(), loc.line());
                T::default()
            }
        }
    }

    #[track_caller]
    fn unwrap_warn_or_log(self, default: T) -> T {
        match self {
            Some(v) => v,
            None => {
                let loc = std::panic::Location::caller();
                log::warn!("[{}:{}] Option was None", loc.file(), loc.line());
                default
            }
        }
    }

    #[track_caller]
    fn unwrap_warn_log(self) -> T
    where
        T: Default,
    {
        match self {
            Some(v) => v,
            None => {
                let loc = std::panic::Location::caller();
                log::warn!("[{}:{}] Option was None", loc.file(), loc.line());
                T::default()
            }
        }
    }

    #[track_caller]
    fn expect_log(self, context: impl Into<String>) -> T {
        match self {
            Some(v) => v,
            None => {
                let loc = std::panic::Location::caller();
                let ctx = context.into();
                log::error!("[{}:{}] {}", loc.file(), loc.line(), ctx);
                panic!("[{}:{}] {}", loc.file(), loc.line(), ctx);
            }
        }
    }

    #[track_caller]
    fn expect_warn_log(self, context: impl Into<String>) -> T {
        match self {
            Some(v) => v,
            None => {
                let loc = std::panic::Location::caller();
                let ctx = context.into();
                log::warn!("[{}:{}] {}", loc.file(), loc.line(), ctx);
                panic!("[{}:{}] {}", loc.file(), loc.line(), ctx);
            }
        }
    }
}
