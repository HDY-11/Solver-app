use std::io;
use std::path::Path;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use error_system::ResultLogExt;

#[derive(Debug, Clone)]
pub(crate) struct NodeMeta {
    pub id: i64,
    pub name: String,
    pub node_type: String,
    pub parent_id: Option<i64>,
    pub volume: String,
    pub content_hash: Option<String>,
    pub storage_offset: Option<i64>,
    pub size: Option<i64>,
    pub version: String,
    pub created_at: String,
    pub modified_at: String,
    pub deleted: bool,
    pub linked_files: Option<String>,
}

// ── 获取连接 ──────────────────────────────────────

pub(crate) fn get_conn(pool: &Pool<SqliteConnectionManager>) -> io::Result<r2d2::PooledConnection<SqliteConnectionManager>> {
    pool.get()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("获取数据库连接失败: {}", e)))
}

// ── 单节点查询 ────────────────────────────────────

pub(crate) fn find_node_by_path(conn: &rusqlite::Connection, path: &str) -> io::Result<Option<NodeMeta>> {
    let components = env_system::vfs_components(Path::new(path))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "无效的 VFS 路径"))?;
    let volume = env_system::vfs_volume(Path::new(path))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "无效的 VFS 路径"))?;

    // 不能容忍它失败
    let mut stmt = conn.prepare(
        "SELECT id, name, node_type, parent_id, volume, content_hash,
                storage_offset, size, version, created_at, modified_at, deleted, linked_files
         FROM nodes WHERE parent_id IS ? AND name = ? AND volume = ? AND deleted = 0"
    )
    .expect_log("准备 SQL 查询失败");

    let mut parent_id: Option<i64> = None;
    let mut result = None;
    for component in components.iter() {
        match stmt.query_row(params![parent_id, component, volume], |row| row_to_node(row)) {
            Ok(node) => {
                parent_id = Some(node.id);
                result = Some(node);
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(io::Error::new(io::ErrorKind::Other, e.to_string())),
        }
    }
    Ok(result)
}

pub(crate) fn find_node_by_id(conn: &rusqlite::Connection, id: i64) -> io::Result<Option<NodeMeta>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, node_type, parent_id, volume, content_hash,
                storage_offset, size, version, created_at, modified_at, deleted, linked_files
         FROM nodes WHERE id = ? AND deleted = 0"
    )
    .expect_log("准备 SQL 查询失败");

    match stmt.query_row(params![id], |row| row_to_node(row)) {
        Ok(node) => Ok(Some(node)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(io::Error::new(io::ErrorKind::Other, e.to_string())),
    }
}

// ── 列表查询 ──────────────────────────────────────

pub(crate) fn list_children(conn: &rusqlite::Connection, parent_id: Option<i64>) -> io::Result<Vec<NodeMeta>> {
    let mut stmt = if let Some(_pid) = parent_id {
        conn.prepare(
            "SELECT id, name, node_type, parent_id, volume, content_hash,
                    storage_offset, size, version, created_at, modified_at, deleted, linked_files
             FROM nodes WHERE parent_id = ? AND deleted = 0 ORDER BY node_type, name"
        )
        .expect_log("准备列表查询失败")
    } else {
        conn.prepare(
            "SELECT id, name, node_type, parent_id, volume, content_hash,
                    storage_offset, size, version, created_at, modified_at, deleted, linked_files
             FROM nodes WHERE parent_id IS NULL AND deleted = 0 ORDER BY volume, name"
        )
        .expect_log("准备列表查询失败")
    };

    let rows = stmt.query_map(params![parent_id], |row| row_to_node(row))
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
}

pub(crate) fn list_children_by_path(conn: &rusqlite::Connection, path: &str) -> io::Result<Vec<NodeMeta>> {
    let node = find_node_by_path(conn, path)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("路径不存在: {}", path)))?;
    list_children(conn, Some(node.id))
}

// ── 写入操作 ──────────────────────────────────────

pub(crate) fn insert_node(conn: &rusqlite::Connection, name: &str, node_type: &str, parent_id: Option<i64>, volume: &str) -> io::Result<i64> {
    conn.execute(
        "INSERT INTO nodes (name, node_type, parent_id, volume) VALUES (?, ?, ?, ?)",
        params![name, node_type, parent_id, volume],
    )
    .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("insert_node failed!{}", e.to_string())))?;
    Ok(conn.last_insert_rowid())
}

pub(crate) fn update_node_storage(conn: &rusqlite::Connection, id: i64, offset: u64, size: u64, hash: &str) -> io::Result<()> {
    conn.execute(
        "UPDATE nodes SET storage_offset = ?, size = ?, content_hash = ?, modified_at = datetime('now') WHERE id = ?",
        params![offset as i64, size as i64, hash, id],
    )
    .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("update_node_storage failed!{}", e.to_string())))?;
    Ok(())
}

pub(crate) fn update_node_modified_at(conn: &rusqlite::Connection, id: i64) -> io::Result<()> {
    conn.execute(
        "UPDATE nodes SET modified_at = datetime('now') WHERE id = ?",
        params![id],
    )
    .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("update_node_modified_at failed!{}", e.to_string())))?;
    Ok(())
}

pub(crate) fn soft_delete_node(conn: &rusqlite::Connection, id: i64) -> io::Result<()> {
    conn.execute(
        "UPDATE nodes SET deleted = 1, modified_at = datetime('now') WHERE id = ?",
        params![id],
    )
    .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("soft_delete_node failed!{}", e.to_string())))?;
    Ok(())
}

pub(crate) fn rename_node(conn: &rusqlite::Connection, id: i64, new_name: &str) -> io::Result<()> {
    conn.execute(
        "UPDATE nodes SET name = ?, modified_at = datetime('now') WHERE id = ?",
        params![new_name, id],
    )
    .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("rename_node failed!{}", e.to_string())))?;
    Ok(())
}

pub(crate) fn move_node(conn: &rusqlite::Connection, id: i64, new_parent_id: Option<i64>, new_volume: &str) -> io::Result<()> {
    conn.execute(
        "UPDATE nodes SET parent_id = ?, volume = ?, modified_at = datetime('now') WHERE id = ?",
        params![new_parent_id, new_volume, id],
    )
    .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("move_node failed!{}", e.to_string())))?;
    Ok(())
}

pub(crate) fn get_storage_offset(conn: &rusqlite::Connection, id: i64) -> io::Result<(u64, u64)> {
    let mut stmt = conn.prepare(
        "SELECT storage_offset, size FROM nodes WHERE id = ? AND deleted = 0"
    )
    .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("get_storage_offset failed!{}", e.to_string())))?;

    stmt.query_row(params![id], |row| {
        Ok((
            row.get::<_, Option<i64>>(0)?.unwrap_or(0) as u64,
            row.get::<_, Option<i64>>(1)?.unwrap_or(0) as u64,
        ))
    })
    .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
}

pub(crate) fn node_exists_path(conn: &rusqlite::Connection, path: &str) -> io::Result<bool> {
    Ok(find_node_by_path(conn, path)?.is_some())
}

pub(crate) fn ensure_parent_dirs(conn: &rusqlite::Connection, path: &str) -> io::Result<i64> {
    let volume = env_system::vfs_volume(Path::new(path))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "无效的 VFS 路径"))?;
    let inner = env_system::vfs_inner_path(Path::new(path)).unwrap_or_default();
    let parent_path = Path::new(&inner).parent().map(|p| p.to_string_lossy().to_string());

    if let Some(parent) = parent_path {
        if !parent.is_empty() {
            let parts: Vec<&str> = parent.split('/').filter(|s| !s.is_empty()).collect();
            let mut current_parent: Option<i64> = None;
            
            for part in parts {
                let existing = find_node_by_name_and_parent(conn, part, current_parent, &volume)?;
                match existing {
                    Some(node) => {
                        current_parent = Some(node.id);
                    }
                    None => {
                        let new_id = insert_node(conn, part, "folder", current_parent, &volume)?;
                        current_parent = Some(new_id);
                    }
                }
            }
            return Ok(current_parent.unwrap_or(0));
        }
    }
    Ok(0)
}

fn find_node_by_name_and_parent(conn: &rusqlite::Connection, name: &str, parent_id: Option<i64>, volume: &str) -> io::Result<Option<NodeMeta>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, node_type, parent_id, volume, content_hash,
                storage_offset, size, version, created_at, modified_at, deleted, linked_files
         FROM nodes WHERE parent_id IS ? AND name = ? AND volume = ? AND deleted = 0"
    )
    .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("find_node_by_name_and_parent failed!{}", e.to_string())))?;

    match stmt.query_row(params![parent_id, name, volume], |row| row_to_node(row)) {
        Ok(node) => Ok(Some(node)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(io::Error::new(io::ErrorKind::Other, e.to_string())),
    }
}

fn row_to_node(row: &rusqlite::Row) -> rusqlite::Result<NodeMeta> {
    Ok(NodeMeta {
        id: row.get(0)?,
        name: row.get(1)?,
        node_type: row.get(2)?,
        parent_id: row.get(3)?,
        volume: row.get(4)?,
        content_hash: row.get(5)?,
        storage_offset: row.get(6)?,
        size: row.get(7)?,
        version: row.get(8)?,
        created_at: row.get(9)?,
        modified_at: row.get(10)?,
        deleted: row.get::<_, i32>(11)? != 0,
        linked_files: row.get(12)?,
    })
}