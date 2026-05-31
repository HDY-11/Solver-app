//! 真实文件系统后端（B 盘）
//!
//! B 盘文件直接映射到 `vault_dir()` 下的真实文件。
//! SQLite 保持元数据（名称、类型、版本），实际 I/O 走真实文件系统。

use std::io;
use std::path::{Path, PathBuf};
use std::fs;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use sha2::{Sha256, Digest};
use crate::query;

/// 将 VFS 路径映射到真实文件系统路径
/// `(vfs)/B/foo/bar.txt` → `vault_dir()/foo/bar.txt`
/// `(vfs)/A/data.csv` → `imports_dir()/data.csv`
pub fn vfs_to_real(vfs_path: &str) -> Option<PathBuf> {
    let volume = env_system::vfs_volume(Path::new(vfs_path))?;
    let inner = env_system::vfs_inner_path(Path::new(vfs_path)).unwrap_or_default();
    let base = match volume.as_str() {
        "A" => env_system::imports_dir(),
        "B" => env_system::vault_dir(),
        _ => return None,
    };
    if inner.is_empty() {
        Some(base)
    } else {
        Some(base.join(&inner))
    }
}

/// 计算文件的 SHA-256 哈希
pub fn file_hash(path: &Path) -> io::Result<String> {
    let data = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(hex::encode(hasher.finalize()))
}

/// 同步真实文件卷（A/B 盘）：扫描真实目录 → 确保 DB 中有对应节点
pub fn sync_real_volume(
    pool: &Pool<SqliteConnectionManager>,
    volume: &str,
) -> io::Result<()> {
    let base_dir = match volume {
        "A" => env_system::imports_dir(),
        "B" => env_system::vault_dir(),
        _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "不支持的卷")),
    };
    if !base_dir.exists() {
        fs::create_dir_all(&base_dir)?;
    }

    let conn = pool.get()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    // 确保根节点存在
    let root_name = format!("{}:", volume);
    let root_id = query::ensure_root_node(&conn, &root_name, volume)?;

    // 扫描到的所有真实路径（DB 风格：相对路径段）
    let mut seen_ids: Vec<i64> = Vec::new();

    // 递归扫描
    scan_dir(&conn, &base_dir, root_id, volume, &mut seen_ids)?;

    // A/B 盘不软删除：直接硬删除不在真实目录中的 DB 记录
    if !seen_ids.is_empty() {
        let placeholders = seen_ids.iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "DELETE FROM nodes WHERE volume=? AND parent_id IS NOT NULL AND id NOT IN ({})",
            placeholders
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        params.push(Box::new(volume.to_string()));
        for id in &seen_ids {
            params.push(Box::new(*id));
        }
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, param_refs.as_slice())
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    }

    log::info!("[real_fs] {}盘同步完成: {} 个条目", volume, seen_ids.len());
    Ok(())
}

fn scan_dir(
    conn: &rusqlite::Connection,
    real_dir: &Path,
    parent_db_id: i64,
    volume: &str,
    seen: &mut Vec<i64>,
) -> io::Result<()> {
    let entries = match fs::read_dir(real_dir) {
        Ok(e) => e,
        Err(e) => {
            log::warn!("[real_fs] 读取目录失败: {}: {}", real_dir.display(), e);
            return Ok(());
        }
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let node_type = if is_dir { "folder" } else { "file" };

        // 查询已有节点
        let existing = query::find_node_by_name_and_parent(conn, &name, Some(parent_db_id), volume)
            .unwrap_or(None);

        let node_id = if let Some(ref node) = existing {
            node.id
        } else {
            // 新建节点
            let id = query::insert_node(conn, &name, node_type, Some(parent_db_id), volume)?;
            log::debug!("[real_fs] 新增: {} (id={})", name, id);
            id
        };

        // 对于文件，更新 size/hash
        if !is_dir {
            if let Ok(meta) = path.metadata() {
                let size = meta.len();
                let hash = file_hash(&path).unwrap_or_default();
                let _ = query::update_node_real_meta(conn, node_id, size as i64, &hash);
            }
        }

        seen.push(node_id);

        // 递归子目录
        if is_dir {
            scan_dir(conn, &path, node_id, volume, seen)?;
        }
    }

    Ok(())
}
