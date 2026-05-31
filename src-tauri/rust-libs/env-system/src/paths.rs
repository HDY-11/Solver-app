//! 路径构造函数
//!
//! 提供编译期常量和运行时动态的路径构造。
//! 所有函数返回标准库 [`PathBuf`]。
//!
//! 不使用全局变量，调用方按需调用函数获取路径。
//! 需要配置的路径通过 [`crate::AppConfig`] trait 获取。

use std::path::PathBuf;
use dirs;
// ── 编译期常量 ──────────────────────────────────

/// 应用在操作系统上的数据目录名。
pub const APP_DIR_NAME: &str = "solver";

/// 资料区目录名。
pub const VAULT_DIR_NAME: &str = "vault";

/// 日志目录名。
pub const LOG_DIR_NAME: &str = "logs";

/// BlobStore 文件扩展名。
pub const BLOB_EXT: &str = "blob";

/// SQLite 数据库文件名。
pub const DATABASE_FILE_NAME: &str = "meta.db";

/// 配置文件目录名。
pub const CONFIG_DIR_NAME: &str = "config";

/// 默认工作台名。
pub const DEFAULT_WORKSPACE: &str = "工作台";

/// 脚本目录名。
pub const SCRIPTS_DIR_NAME: &str = "脚本";

/// 运行结果目录名。
pub const RUN_RESULTS_DIR_NAME: &str = "运行结果";

// ── 操作系统数据目录 ────────────────────────────

/// 应用数据根目录。
///
/// 根据操作系统自动选择：
/// - Windows: `C:\Users\<user>\AppData\Roaming\solver\`
/// - macOS:   `~/Library/Application Support/solver/`
/// - Linux:   `~/.local/share/solver/`
pub fn app_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DIR_NAME)
}

/// 应用根目录
pub fn exe_root() -> PathBuf{
    let exe_dir = std::env::current_dir().expect("应用目录打开失败");

    #[cfg(debug_assertions)]
    {
        let mut root = exe_dir.clone();
        while !root.join("src-tauri").exists() {
            root = root.parent().expect("无法找到项目根目录").to_path_buf();
        }
        root
    }

    #[cfg(not(debug_assertions))]
    {
        exe_dir
    }
}

// ── VFS 存储路径 ────────────────────────────────

/// VFS 的 SQLite 数据库文件路径。
pub fn database_path() -> PathBuf {
    app_data_dir().join(DATABASE_FILE_NAME)
}

/// 资料区（B 盘）在真实文件系统中的根目录。
pub fn vault_dir() -> PathBuf {
    app_data_dir().join(VAULT_DIR_NAME)
}

/// 导入区（A 盘）在真实文件系统中的根目录（只读）。
pub fn imports_dir() -> PathBuf {
    app_data_dir().join("imports")
}

/// 指定卷的 BlobStore 文件路径。
pub fn blob_path(volume: &str) -> PathBuf {
    app_data_dir().join("blobs").join(format!("{}.{}", volume, BLOB_EXT))
}

/// 日志目录路径。
pub fn log_dir() -> PathBuf {
    app_data_dir().join(LOG_DIR_NAME)
}

/// 配置文件目录路径。
pub fn config_dir() -> PathBuf {
    app_data_dir().join(CONFIG_DIR_NAME)
}

/// 应用配置文件路径 (settings.toml)。
pub fn config_file_path() -> PathBuf {
    config_dir().join("settings.toml")
}

/// 内嵌 Python 的 site-packages 路径。
pub fn embedded_site_packages() -> PathBuf {
    let mut root = exe_root();
    #[cfg(not(debug_assertions))]
    {
        root = root.join("Lib\\site_packages");
    }
    
    #[cfg(debug_assertions)]
    {
        root = root.parent().unwrap().join(".venv\\Lib\\site_packages");
    }
    root
}