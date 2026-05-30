use std::io;
use std::path::Path;

// ── 列名常量 ──────────────────────────────────────

/// nodes 表全部列（统一维护，避免每处 SELECT 重复写）
const NODE_COLS: &str = "id, name, node_type, parent_id, volume, content_hash, \
    storage_offset, size, version, created_at, modified_at, deleted, linked_files";

// ── 通用工具 ──────────────────────────────────────

/// 将任意 Display 错误映射为 io::Error
fn map_io(context: &str, e: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::Other, format!("{}: {}", context, e))
}

/// 获取 DB 连接
/// 执行 query_row，统一处理 NotFound 和 Err 分支
fn query_single(conn: &rusqlite::Connection, sql: &str, p: &[&dyn rusqlite::types::ToSql]) -> io::Result<Option<NodeMeta>> {
    let mut stmt = conn.prepare(sql).map_err(|e| map_io("准备查询", e))?;
    match stmt.query_row(p, |row| row_to_node(row)) {
        Ok(node) => Ok(Some(node)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(map_io("查询失败", e)),
    }
}

/// 执行 query_map，返回 Vec<NodeMeta>
fn query_many(conn: &rusqlite::Connection, sql: &str, p: &[&dyn rusqlite::types::ToSql]) -> io::Result<Vec<NodeMeta>> {
    let mut stmt = conn.prepare(sql).map_err(|e| map_io("准备查询", e))?;
    stmt.query_map(p, |row| row_to_node(row))
        .map_err(|e| map_io("查询映射", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| map_io("收集结果", e))
}

/// 执行 INSERT/UPDATE/DELETE，返回影响行数
fn exec(conn: &rusqlite::Connection, sql: &str, p: &[&dyn rusqlite::types::ToSql]) -> io::Result<usize> {
    conn.execute(sql, p).map_err(|e| map_io("执行写操作", e))
}

// ── NodeMeta ──────────────────────────────────────

#[derive(Debug, Clone)]
pub struct NodeMeta {
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

// ── NodeVersionMeta（时间线）──────────────────────

#[derive(Debug, Clone)]
pub struct NodeVersionMeta {
    pub id: i64,
    pub node_id: i64,
    pub content_hash: String,
    pub storage_offset: i64,
    pub size: i64,
    pub created_at: String,
}

// ── 单节点查询 ────────────────────────────────────
/// 通过路径查找数据库中对应的节点元信息
pub(crate) fn find_node_by_path(conn: &rusqlite::Connection, path: &str) -> io::Result<Option<NodeMeta>> {
    let components = env_system::vfs_components(Path::new(path))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "无效的 VFS 路径"))?;
    let volume = env_system::vfs_volume(Path::new(path))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "无效的 VFS 路径"))?;

    if components.is_empty() {
        let root = format!("{}:", volume);
        return query_single(conn,
            &format!("SELECT {} FROM nodes WHERE parent_id IS NULL AND name=? AND volume=? AND deleted=0", NODE_COLS),
            rusqlite::params![&root, &volume],
        );
    }

    let root_name = format!("{}:", volume);
    let root = match find_node_by_name_and_parent(conn, &root_name, None, &volume)? {
        Some(r) => r,
        None => return Ok(None),
    };

    let mut parent_id = Some(root.id);
    let mut last = Some(root);
    for comp in &components {
        match find_node_by_name_and_parent(conn, comp, parent_id, &volume)? {
            Some(node) => { parent_id = Some(node.id); last = Some(node); }
            None => return Ok(None),
        }
    }
    Ok(last)
}
/// 通过id查找数据库中对应的节点id元信息
pub(crate) fn find_node_by_id(conn: &rusqlite::Connection, id: i64) -> io::Result<Option<NodeMeta>> {
    query_single(conn,
        &format!("SELECT {} FROM nodes WHERE id=? AND deleted=0", NODE_COLS),
        rusqlite::params![&id],
    )
}
/// 通过
pub(crate) fn find_node_by_name_and_parent(conn: &rusqlite::Connection, name: &str, parent_id: Option<i64>, volume: &str) -> io::Result<Option<NodeMeta>> {
    query_single(conn,
        &format!("SELECT {} FROM nodes WHERE parent_id IS ? AND name=? AND volume=? AND deleted=0", NODE_COLS),
        rusqlite::params![&parent_id, &name, &volume],
    )
}

// ── 列表查询 ──────────────────────────────────────

pub(crate) fn list_children(conn: &rusqlite::Connection, parent_id: Option<i64>) -> io::Result<Vec<NodeMeta>> {
    if let Some(pid) = parent_id {
        query_many(conn,
            &format!("SELECT {} FROM nodes WHERE parent_id=? AND deleted=0 ORDER BY node_type, name", NODE_COLS),
            rusqlite::params![&pid],
        )
    } else {
        query_many(conn,
            &format!("SELECT {} FROM nodes WHERE parent_id IS NULL AND deleted=0 ORDER BY volume, name", NODE_COLS),
            rusqlite::params![],
        )
    }
}

pub(crate) fn list_children_by_path(conn: &rusqlite::Connection, path: &str) -> io::Result<Vec<NodeMeta>> {
    let node = if is_volume_root(path) {
        let volume = env_system::vfs_volume(Path::new(path))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "无效的 VFS 路径"))?;
        let root_name = format!("{}:", volume);
        find_node_by_name_and_parent(conn, &root_name, None, &volume)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("卷根节点不存在: {}", root_name)))?
    } else {
        find_node_by_path(conn, path)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("路径不存在: {}", path)))?
    };
    list_children(conn, Some(node.id))
}

fn is_volume_root(path: &str) -> bool {
    env_system::vfs_inner_path(Path::new(path)).as_deref().map_or(true, |s| s.is_empty())
}

// ── 写入操作 ──────────────────────────────────────

pub(crate) fn insert_node(conn: &rusqlite::Connection, name: &str, node_type: &str, parent_id: Option<i64>, volume: &str) -> io::Result<i64> {
    exec(conn,
        "INSERT INTO nodes (name, node_type, parent_id, volume) VALUES (?, ?, ?, ?)",
        rusqlite::params![&name, &node_type, &parent_id, &volume],
    )?;
    Ok(conn.last_insert_rowid())
}

/// 更新节点的 blob 存储偏移——不再自动递增 version（version 由用户手动设置）
pub(crate) fn update_node_storage(conn: &rusqlite::Connection, id: i64, offset: u64, size: u64, hash: &str) -> io::Result<()> {
    exec(conn,
        "UPDATE nodes SET storage_offset=?, size=?, content_hash=?, modified_at=datetime('now') WHERE id=?",
        rusqlite::params![&(offset as i64), &(size as i64), &hash, &id],
    )?;
    Ok(())
}

pub(crate) fn update_node_modified_at(conn: &rusqlite::Connection, id: i64) -> io::Result<()> {
    exec(conn, "UPDATE nodes SET modified_at=datetime('now') WHERE id=?", rusqlite::params![&id])?;
    Ok(())
}

pub(crate) fn soft_delete_node(conn: &rusqlite::Connection, id: i64) -> io::Result<()> {
    exec(conn, "UPDATE nodes SET deleted=1, modified_at=datetime('now') WHERE id=?", rusqlite::params![&id])?;
    Ok(())
}

/// 确保卷根节点存在（用于 real_fs 同步）
pub(crate) fn ensure_root_node(conn: &rusqlite::Connection, root_name: &str, volume: &str) -> io::Result<i64> {
    if let Some(node) = find_node_by_name_and_parent(conn, root_name, None, volume)? {
        return Ok(node.id);
    }
    let id = insert_node(conn, root_name, "folder", None, volume)?;
    Ok(id)
}

/// 更新真实文件元数据（size + hash + modified_at），不改 storage_offset
pub(crate) fn update_node_real_meta(conn: &rusqlite::Connection, id: i64, size: i64, hash: &str) -> io::Result<()> {
    exec(conn,
        "UPDATE nodes SET size=?, content_hash=?, modified_at=datetime('now') WHERE id=?",
        rusqlite::params![&size, &hash, &id],
    )?;
    Ok(())
}

pub(crate) fn rename_node(conn: &rusqlite::Connection, id: i64, new_name: &str) -> io::Result<()> {
    exec(conn, "UPDATE nodes SET name=?, modified_at=datetime('now') WHERE id=?", rusqlite::params![&new_name, &id])?;
    Ok(())
}

/// 设置节点版本号（用户手动修改）
pub(crate) fn set_node_version(conn: &rusqlite::Connection, id: i64, new_version: &str) -> io::Result<()> {
    exec(conn, "UPDATE nodes SET version=?, modified_at=datetime('now') WHERE id=?", rusqlite::params![&new_version, &id])?;
    Ok(())
}

/// 获取节点当前 content_hash（用于写入前去重）
pub(crate) fn get_content_hash(conn: &rusqlite::Connection, id: i64) -> io::Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT content_hash FROM nodes WHERE id=? AND deleted=0")
        .map_err(|e| map_io("get_content_hash 准备", e))?;
    match stmt.query_row(rusqlite::params![&id], |row| row.get::<_, Option<String>>(0)) {
        Ok(hash) => Ok(hash),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(map_io("get_content_hash 查询", e)),
    }
}

pub(crate) fn get_storage_offset(conn: &rusqlite::Connection, id: i64) -> io::Result<(u64, u64)> {
    let mut stmt = conn.prepare("SELECT storage_offset, size FROM nodes WHERE id=? AND deleted=0")
        .map_err(|e| map_io("get_storage_offset 准备", e))?;
    stmt.query_row(rusqlite::params![&id], |row| {
        Ok((row.get::<_, Option<i64>>(0)?.unwrap_or(0) as u64,
            row.get::<_, Option<i64>>(1)?.unwrap_or(0) as u64))
    }).map_err(|e| map_io("get_storage_offset 查询", e))
}

pub(crate) fn node_exists_path(conn: &rusqlite::Connection, path: &str) -> io::Result<bool> {
    if is_volume_root(path) {
        let volume = env_system::vfs_volume(Path::new(path))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "无效的 VFS 路径"))?;
        let root_name = format!("{}:", volume);
        return Ok(find_node_by_name_and_parent(conn, &root_name, None, &volume)?.is_some());
    }
    Ok(find_node_by_path(conn, path)?.is_some())
}

pub(crate) fn ensure_parent_dirs(conn: &rusqlite::Connection, path: &str) -> io::Result<i64> {
    let volume = env_system::vfs_volume(Path::new(path))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "无效的 VFS 路径"))?;
    let inner = env_system::vfs_inner_path(Path::new(path)).unwrap_or_default();
    let parent = Path::new(&inner).parent().map(|p| p.to_string_lossy().to_string());

    let root_name = format!("{}:", volume);
    let root = find_node_by_name_and_parent(conn, &root_name, None, &volume)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("卷根节点不存在: {}", root_name)))?;

    let Some(parent) = parent else { return Ok(root.id) };
    if parent.is_empty() { return Ok(root.id) }

    let parts: Vec<&str> = parent.split('/').filter(|s| !s.is_empty()).collect();
    let mut cur = root.id;
    for part in parts {
        match find_node_by_name_and_parent(conn, part, Some(cur), &volume)? {
            Some(node) => cur = node.id,
            None => cur = insert_node(conn, part, "folder", Some(cur), &volume)?,
        }
    }
    Ok(cur)
}

// ── node_versions 时间线查询 ──────────────────────

/// 存档旧版本：写入前调用，将当前 (hash, offset, size) 插入 node_versions
pub(crate) fn archive_version(conn: &rusqlite::Connection, node_id: i64, hash: &str, offset: i64, size: i64) -> io::Result<()> {
    // 同一 (node_id, hash) 只存一次（内容去重），冲突时静默忽略
    exec(conn,
        "INSERT OR IGNORE INTO node_versions (node_id, content_hash, storage_offset, size) VALUES (?, ?, ?, ?)",
        rusqlite::params![&node_id, &hash, &offset, &size],
    )?;
    Ok(())
}

/// 获取某节点的版本时间线，按时间倒序
pub(crate) fn get_version_history(conn: &rusqlite::Connection, node_id: i64) -> io::Result<Vec<NodeVersionMeta>> {
    let stmt_str = "SELECT id, node_id, content_hash, storage_offset, size, created_at \
                    FROM node_versions WHERE node_id=? ORDER BY created_at DESC";
    let mut stmt = conn.prepare(stmt_str).map_err(|e| map_io("get_version_history 准备", e))?;
    stmt.query_map(rusqlite::params![&node_id], |row| {
        Ok(NodeVersionMeta {
            id: row.get(0)?,
            node_id: row.get(1)?,
            content_hash: row.get(2)?,
            storage_offset: row.get(3)?,
            size: row.get(4)?,
            created_at: row.get(5)?,
        })
    }).map_err(|e| map_io("get_version_history 映射", e))?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| map_io("get_version_history 收集", e))
}

/// 按 node_id + content_hash 查找版本（检查去重）
pub(crate) fn find_version_by_hash(conn: &rusqlite::Connection, node_id: i64, hash: &str) -> io::Result<Option<NodeVersionMeta>> {
    let mut stmt = conn.prepare(
        "SELECT id, node_id, content_hash, storage_offset, size, created_at \
         FROM node_versions WHERE node_id=? AND content_hash=? LIMIT 1"
    ).map_err(|e| map_io("find_version_by_hash 准备", e))?;
    match stmt.query_row(rusqlite::params![&node_id, &hash], |row| {
        Ok(NodeVersionMeta {
            id: row.get(0)?, node_id: row.get(1)?, content_hash: row.get(2)?,
            storage_offset: row.get(3)?, size: row.get(4)?, created_at: row.get(5)?,
        })
    }) {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(map_io("find_version_by_hash 查询", e)),
    }
}

// ── linked_files 查询（运行记录去重）────────────────

/// 按 linked_files LIKE 模式查询所有未删除的 run 类型节点
pub fn query_run_nodes_by_linked_files(
    conn: &rusqlite::Connection,
    pattern: &str,
) -> io::Result<Vec<NodeMeta>> {
    let sql = format!(
        "SELECT {} FROM nodes WHERE node_type='run' AND deleted=0 AND linked_files LIKE ?",
        NODE_COLS
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| map_io("query_run_nodes_like 准备", e))?;
    stmt.query_map(rusqlite::params![&pattern], |row| row_to_node(row))
        .map_err(|e| map_io("query_run_nodes_like 映射", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| map_io("query_run_nodes_like 收集", e))
}

/// 插入运行记录节点（带 linked_files）
pub fn insert_run_node(
    conn: &rusqlite::Connection,
    name: &str,
    parent_id: i64,
    volume: &str,
    linked_files: &str,
) -> io::Result<i64> {
    exec(
        conn,
        "INSERT INTO nodes (name, node_type, parent_id, volume, linked_files) VALUES (?, 'run', ?, ?, ?)",
        rusqlite::params![&name, &parent_id, &volume, &linked_files],
    )?;
    Ok(conn.last_insert_rowid())
}

/// 插入运行记录节点，直接复制源节点的 BLOB 引用（去重复用）
pub fn insert_run_node_from_source(
    conn: &rusqlite::Connection,
    name: &str,
    parent_id: i64,
    volume: &str,
    linked_files: &str,
    source_offset: i64,
    source_size: i64,
    source_hash: &str,
) -> io::Result<i64> {
    exec(
        conn,
        "INSERT INTO nodes (name, node_type, parent_id, volume, linked_files, \
         storage_offset, size, content_hash) VALUES (?, 'run', ?, ?, ?, ?, ?, ?)",
        rusqlite::params![&name, &parent_id, &volume, &linked_files,
                          &source_offset, &source_size, &source_hash],
    )?;
    Ok(conn.last_insert_rowid())
}

/// 更新节点的 linked_files 字段
pub fn update_node_linked_files(conn: &rusqlite::Connection, id: i64, linked_files: &str) -> io::Result<()> {
    exec(conn, "UPDATE nodes SET linked_files=?, modified_at=datetime('now') WHERE id=?",
        rusqlite::params![&linked_files, &id])?;
    Ok(())
}

// ── row → NodeMeta ────────────────────────────────

fn row_to_node(row: &rusqlite::Row) -> rusqlite::Result<NodeMeta> {
    Ok(NodeMeta {
        id: row.get(0)?, name: row.get(1)?, node_type: row.get(2)?,
        parent_id: row.get(3)?, volume: row.get(4)?, content_hash: row.get(5)?,
        storage_offset: row.get(6)?, size: row.get(7)?, version: row.get(8)?,
        created_at: row.get(9)?, modified_at: row.get(10)?,
        deleted: row.get::<_, i32>(11)? != 0,
        linked_files: row.get(12)?,
    })
}