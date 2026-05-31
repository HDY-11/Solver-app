//! # 环境系统
//!
//! 这个库用来解决整个应用环境配置问题。
//!
//! ## 职责
//!
//! - **路径构造函数**：用 [`const fn`] 和 [`fn`] 提供编译期和运行时路径
//! - **虚拟路径辅助**：统一 `(vfs)/{卷名}/{路径}` 语法
//! - **配置 trait**：让调用方声明自己的配置格式和位置
//!
//! ## 不做的事
//!
//! - 不定义新的路径类型（用标准库 [`Path`] 和 [`PathBuf`]）
//! - 不提供全局变量（所有路径按需调用函数获取）
//! - 不预设配置内容（由调用方通过 serde 定义）
//!
//! ## 虚拟路径语法
//!
//! | 表达式 | 含义 |
//! |--------|------|
//! | `(vfs)/C/脚本/model.py` | VFS C 盘下的路径 |
//! | `(vfs)/B/数据/raw.sav` | VFS B 盘下的路径 |
//! | `/home/user/data.csv` | 真实文件系统路径 |
//! | `(tmp)/script_abc.py` | 临时导出路径 |

pub mod config;
pub mod paths;
pub mod vfs_path;


// 重导出常用项
pub use config::AppConfig;
pub use paths::*;
pub use vfs_path::*;