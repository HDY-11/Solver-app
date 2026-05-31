//! 应用配置 trait
//!
//! 让调用方声明自己需要什么格式的配置文件，以及配置文件在哪。
//! 不预设配置内容——由调用方通过 serde 定义自己的配置结构。

use serde::{de::DeserializeOwned, Serialize};
use std::io;
use std::path::PathBuf;
use toml;
/// 应用配置 trait。
///
/// 实现者指定：
/// - 配置文件路径
/// - 默认配置
///
/// 使用示例：
/// ```
/// use env_system::AppConfig;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Serialize, Deserialize)]
/// struct MyConfig {
///     python_path: String,
/// }
///
/// struct SolverConfig;
/// impl AppConfig<MyConfig> for SolverConfig {
///     fn config_path() -> std::path::PathBuf {
///         env_system::config::config_file_path()
///     }
///     
///     fn default_config() -> MyConfig {
///         MyConfig { python_path: "python3".into() }
///     }
/// }
/// ```
pub trait AppConfig<T: Serialize + DeserializeOwned + Default> {
    /// 配置文件路径。
    fn config_path() -> PathBuf;

    /// 默认配置。
    fn default_config() -> T {
        T::default()
    }

    /// 读取配置。
    ///
    /// 如果文件不存在，返回默认配置并写入文件。
    fn read_config() -> io::Result<T> {
        let path = Self::config_path();
        if !path.exists() {
            let default = Self::default_config();
            Self::write_config(&default)?;
            return Ok(default);
        }

        let content = std::fs::read_to_string(&path)?;
        toml::from_str(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
    }

    /// 写入配置。
    fn write_config(config: &T) -> io::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(config)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        std::fs::write(&path, content)
    }

    /// 更新部分配置（读取 → 修改 → 写入）。
    fn update_config(f: impl FnOnce(&mut T)) -> io::Result<()> {
        let mut config = Self::read_config()?;
        f(&mut config);
        Self::write_config(&config)
    }
}

/// 便捷函数：读取配置文件。
///
/// 不需要实现完整的 trait——直接指定路径和类型。
pub fn read_config_from<T: Serialize + DeserializeOwned + Default>(path: &PathBuf) -> io::Result<T> {
    if !path.exists() {
        let default = T::default();
        write_config_to(path, &default)?;
        return Ok(default);
    }

    let content = std::fs::read_to_string(path)?;
    toml::from_str(&content)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
}

/// 便捷函数：写入配置文件。
pub fn write_config_to<T: Serialize>(path: &PathBuf, config: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = toml::to_string_pretty(config)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    std::fs::write(path, content)
}