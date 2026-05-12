//! 虚拟路径的辅助函数
//!
//! 虚拟路径语法：`(vfs)/{卷名}/{内部路径}`
//!
//! 示例：
//! - `(vfs)/C/脚本/model.py`
//! - `(vfs)/B/数据/survey.csv`

use std::path::{Path, PathBuf};

/// 虚拟路径的前缀。
pub const VFS_PREFIX: &str = "(vfs)";

// ── 构造 ────────────────────────────────────────

/// 构造 VFS 虚拟路径。
///
/// ```
/// use env_system::vfs_path;
/// let p = vfs_path("C", "脚本/model.py");
/// assert_eq!(p.to_string_lossy(), "(vfs)/C/脚本/model.py");
/// ```
pub fn vfs_path(volume: &str, path: &str) -> PathBuf {
    let clean = path.trim_start_matches('/').trim_start_matches('\\');
    PathBuf::from(format!("{}/{}/{}", VFS_PREFIX, volume, clean))
}

/// 构造某卷的根路径。
pub fn vfs_root(volume: &str) -> PathBuf {
    PathBuf::from(format!("{}/{}", VFS_PREFIX, volume))
}

/// 构造工作台路径。
pub fn workspace_path(workspace: &str) -> PathBuf {
    vfs_path("C", workspace)
}

/// 构造工作台内脚本目录路径。
pub fn workspace_scripts(workspace: &str) -> PathBuf {
    vfs_path("C", &format!("{}/{}", workspace, super::paths::SCRIPTS_DIR_NAME))
}

/// 构造工作台内运行结果目录路径。
pub fn workspace_runs(workspace: &str) -> PathBuf {
    vfs_path("C", &format!("{}/{}", workspace, super::paths::RUN_RESULTS_DIR_NAME))
}

/// 构造某次运行的目录路径。
pub fn run_dir(workspace: &str, run_id: i64) -> PathBuf {
    vfs_path("C", &format!(
        "{}/{}/run_{:04}",
        workspace,
        super::paths::RUN_RESULTS_DIR_NAME,
        run_id
    ))
}

/// 构造资料区（B 盘）路径。
pub fn storage_path(path: &str) -> PathBuf {
    vfs_path("B", path)
}

/// 构造临时路径标识（用于标记临时导出的文件）。
pub fn temp_path(name: &str) -> PathBuf {
    PathBuf::from(format!("(tmp)/{}", name))
}

// ── 判断 ────────────────────────────────────────

/// 判断是否为 VFS 虚拟路径。
pub fn is_vfs(path: &Path) -> bool {
    path.to_string_lossy().starts_with(VFS_PREFIX)
}

/// 判断是否为临时路径。
pub fn is_temp(path: &Path) -> bool {
    path.to_string_lossy().starts_with("(tmp)")
}

/// 判断是否为真实文件系统路径。
pub fn is_real(path: &Path) -> bool {
    !is_vfs(path) && !is_temp(path)
}

// ── 解析 ────────────────────────────────────────

/// 从 VFS 路径中提取卷名。
///
/// 返回 `None` 如果不是 VFS 路径。
pub fn vfs_volume(path: &Path) -> Option<String> {
    let s = path.to_string_lossy();
    if !s.starts_with(VFS_PREFIX) {
        return None;
    }
    let rest = &s[VFS_PREFIX.len()..].trim_start_matches('/').trim_start_matches('\\');
    let end = rest.find('/').or_else(|| rest.find('\\')).unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    Some(rest[..end].to_string())
}

/// 从 VFS 路径中提取内部路径（去掉 `(vfs)/卷名/` 前缀）。
pub fn vfs_inner_path(path: &Path) -> Option<String> {
    let s = path.to_string_lossy();
    if !s.starts_with(VFS_PREFIX) {
        return None;
    }
    let rest = &s[VFS_PREFIX.len()..].trim_start_matches('/').trim_start_matches('\\');
    let first_slash = rest.find('/').or_else(|| rest.find('\\'))?;
    let inner = rest[first_slash + 1..].to_string();
    if inner.is_empty() {
        None
    } else {
        Some(inner)
    }
}

/// 将 VFS 内部路径拆分为组件。
///
/// `(vfs)/C/脚本/model.py` → `["脚本", "model.py"]`
pub fn vfs_components(path: &Path) -> Option<Vec<String>> {
    vfs_inner_path(path).map(|inner| {
        inner
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    })
}

// ── 测试 ────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vfs_path_construction() {
        let p = vfs_path("C", "脚本/model.py");
        assert_eq!(p.to_string_lossy(), "(vfs)/C/脚本/model.py");
    }

    #[test]
    fn test_is_vfs() {
        assert!(is_vfs(&vfs_path("C", "test.py")));
        assert!(!is_vfs(Path::new("/home/user/test.py")));
        assert!(is_temp(&temp_path("abc")));
    }

    #[test]
    fn test_vfs_volume() {
        let p = vfs_path("C", "a/b.py");
        assert_eq!(vfs_volume(&p), Some("C".to_string()));
        assert_eq!(vfs_volume(Path::new("/real/path")), None);
    }

    #[test]
    fn test_vfs_inner_path() {
        let p = vfs_path("C", "脚本/model.py");
        assert_eq!(vfs_inner_path(&p), Some("脚本/model.py".to_string()));
    }

    #[test]
    fn test_vfs_components() {
        let p = vfs_path("C", "脚本/子目录/model.py");
        assert_eq!(
            vfs_components(&p),
            Some(vec!["脚本".to_string(), "子目录".to_string(), "model.py".to_string()])
        );
    }

    #[test]
    fn test_run_dir() {
        let p = run_dir("工作台", 42);
        let s = p.to_string_lossy();
        assert!(s.starts_with("(vfs)/C/"));
        assert!(s.contains("工作台"));
        assert!(s.contains("运行结果"));
        assert!(s.contains("run_0042"));
    }
}