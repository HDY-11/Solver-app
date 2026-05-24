use std::io;
use std::path::Path;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use error_system::ResultLogExt;

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

// ── 获取连接 ──────────────────────────────────────

pub(crate) fn get_conn(pool: &Pool<SqliteConnectionManager>) -> io::Result<r2d2::PooledConnection<SqliteConnectionManager>> {
    pool.get()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("获取数据库连接失败: {}", e)))
}

// ── 单节点查询 ────────────────────────────────────

pub(crate) fn find_node_by_path(conn: &rusqlite::Connection, path: &str) -> io::Result<Option<NodeMeta>> {
    log::debug!("[VFS-query] find_node_by_path: path='{}'", path);

    let components = env_system::vfs_components(Path::new(path))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "无效的 VFS 路径"))?;
    let volume = env_system::vfs_volume(Path::new(path))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "无效的 VFS 路径"))?;

    log::debug!("[VFS-query]   解析结果: volume='{}', components={:?}", volume, components);

    // 根路径：直接查卷根节点
    if components.is_empty() {
        log::debug!("[VFS-query]   是根路径，查卷根节点");
        let mut stmt = conn.prepare(
            "SELECT id, name, node_type, parent_id, volume, content_hash,
                    storage_offset, size, version, created_at, modified_at, deleted, linked_files
             FROM nodes WHERE parent_id IS NULL AND name = ? AND volume = ? AND deleted = 0"
        )
        .expect_log("准备 SQL 查询失败");

        let root_name = format!("{}:", volume);
        log::debug!("[VFS-query]   查根节点: name='{}', volume='{}'", root_name, volume);

        return match stmt.query_row(params![root_name, volume], |row| row_to_node(row)) {
            Ok(node) => {
                log::debug!("[VFS-query]   根节点找到: id={}", node.id);
                Ok(Some(node))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                log::debug!("[VFS-query]   根节点不存在");
                Ok(None)
            }
            Err(e) => {
                log::error!("[VFS-query]   根节点查询失败: {}", e);
                Err(io::Error::new(io::ErrorKind::Other, e.to_string()))
            }
        };
    }

    // 非根路径：先找卷根节点，再逐级查找
    let mut stmt = conn.prepare(
        "SELECT id, name, node_type, parent_id, volume, content_hash,
                storage_offset, size, version, created_at, modified_at, deleted, linked_files
         FROM nodes WHERE parent_id IS ? AND name = ? AND volume = ? AND deleted = 0"
    )
    .expect_log("准备 SQL 查询失败");

    // 先查卷根节点作为起始 parent_id
    let root_name = format!("{}:", volume);
    log::debug!("[VFS-query]   先查根节点作为起点: name='{}', volume='{}'", root_name, volume);

    let root = match stmt.query_row(params![None::<i64>, &root_name, volume], |row| row_to_node(row)) {
        Ok(node) => {
            log::debug!("[VFS-query]   根节点: id={}", node.id);
            node
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            log::debug!("[VFS-query]   卷根节点不存在，无法继续查找");
            return Ok(None);
        }
        Err(e) => {
            log::error!("[VFS-query]   查卷根节点失败: {}", e);
            return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
        }
    };

    // 从根节点开始逐级查找
    let mut parent_id = Some(root.id);
    let mut result = Some(root);

    for component in components.iter() {
        log::debug!("[VFS-query]   查找组件: name='{}', parent_id={:?}, volume='{}'", 
            component, parent_id, volume);

        match stmt.query_row(params![parent_id, component, volume], |row| row_to_node(row)) {
            Ok(node) => {
                log::debug!("[VFS-query]     找到: id={}, type={}", node.id, node.node_type);
                parent_id = Some(node.id);
                result = Some(node);
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                log::debug!("[VFS-query]     未找到，路径中断于组件 '{}'", component);
                return Ok(None);
            }
            Err(e) => {
                log::error!("[VFS-query]     查询失败: {}", e);
                return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
            }
        }
    }

    log::debug!("[VFS-query]   最终节点: id={}", 
        result.as_ref().map(|n| n.id).unwrap_or(-1));
    Ok(result)
}


pub(crate) fn find_node_by_id(conn: &rusqlite::Connection, id: i64) -> io::Result<Option<NodeMeta>> {
    log::debug!("[VFS-query] find_node_by_id: id={}", id);

    let mut stmt = conn.prepare(
        "SELECT id, name, node_type, parent_id, volume, content_hash,
                storage_offset, size, version, created_at, modified_at, deleted, linked_files
         FROM nodes WHERE id = ? AND deleted = 0"
    )
    .expect_log("准备 SQL 查询失败");

    match stmt.query_row(params![id], |row| row_to_node(row)) {
        Ok(node) => {
            log::debug!("[VFS-query]   找到: name='{}', type={}", node.name, node.node_type);
            Ok(Some(node))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            log::debug!("[VFS-query]   未找到");
            Ok(None)
        }
        Err(e) => {
            log::error!("[VFS-query]   查询失败: {}", e);
            Err(io::Error::new(io::ErrorKind::Other, e.to_string()))
        }
    }
}

// ── 列表查询 ──────────────────────────────────────

pub(crate) fn list_children(conn: &rusqlite::Connection, parent_id: Option<i64>) -> io::Result<Vec<NodeMeta>> {
    log::debug!("[VFS-query] list_children: parent_id={:?}", parent_id);

    let mut stmt = if let Some(pid) = parent_id {
        log::debug!("[VFS-query]   查子节点: parent_id={}", pid);
        conn.prepare(
            "SELECT id, name, node_type, parent_id, volume, content_hash,
                    storage_offset, size, version, created_at, modified_at, deleted, linked_files
             FROM nodes WHERE parent_id = ? AND deleted = 0 ORDER BY node_type, name"
        )
        .expect_log("准备列表查询失败")
    } else {
        log::debug!("[VFS-query]   查根级节点: parent_id IS NULL");
        conn.prepare(
            "SELECT id, name, node_type, parent_id, volume, content_hash,
                    storage_offset, size, version, created_at, modified_at, deleted, linked_files
             FROM nodes WHERE parent_id IS NULL AND deleted = 0 ORDER BY volume, name"
        )
        .expect_log("准备列表查询失败")
    };

    let rows = stmt.query_map(params![parent_id], |row| row_to_node(row))
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    let nodes = rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    log::debug!("[VFS-query]   返回 {} 个子节点", nodes.len());
    Ok(nodes)
}

pub(crate) fn list_children_by_path(conn: &rusqlite::Connection, path: &str) -> io::Result<Vec<NodeMeta>> {
    log::debug!("[VFS-query] list_children_by_path: path='{}'", path);

    let node = if is_volume_root(path) {
        let volume = env_system::vfs_volume(Path::new(path))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "无效的 VFS 路径"))?;
        let root_name = format!("{}:", volume);
        log::debug!("[VFS-query]   是卷根路径，查根节点: name='{}', volume='{}'", root_name, volume);

        find_node_by_name_and_parent(conn, &root_name, None, &volume)?
            .ok_or_else(|| {
                log::error!("[VFS-query]   卷根节点不存在: {}", root_name);
                io::Error::new(io::ErrorKind::NotFound, format!("卷根节点不存在: {}", root_name))
            })?
    } else {
        log::debug!("[VFS-query]   非根路径，用 find_node_by_path");
        find_node_by_path(conn, path)?
            .ok_or_else(|| {
                log::error!("[VFS-query]   路径不存在: {}", path);
                io::Error::new(io::ErrorKind::NotFound, format!("路径不存在: {}", path))
            })?
    };

    log::debug!("[VFS-query]   找到节点: id={}, name='{}', type={}", node.id, node.name, node.node_type);
    list_children(conn, Some(node.id))
}

fn is_volume_root(path: &str) -> bool {
    let inner = env_system::vfs_inner_path(Path::new(path));
    let result = inner.as_deref().map_or(true, |s| s.is_empty());
    log::debug!("[VFS-query] is_volume_root: path='{}', inner={:?}, result={}", path, inner, result);
    result
}


// ── 写入操作 ──────────────────────────────────────

pub(crate) fn insert_node(conn: &rusqlite::Connection, name: &str, node_type: &str, parent_id: Option<i64>, volume: &str) -> io::Result<i64> {
    log::debug!("[VFS-query] insert_node: name='{}', type={}, parent_id={:?}, volume='{}'", 
        name, node_type, parent_id, volume);

    conn.execute(
        "INSERT INTO nodes (name, node_type, parent_id, volume) VALUES (?, ?, ?, ?)",
        params![name, node_type, parent_id, volume],
    )
    .map_err(|e| {
        log::error!("[VFS-query] insert_node 失败: name='{}', type={}, parent_id={:?}, volume='{}', error={}", 
            name, node_type, parent_id, volume, e);
        io::Error::new(io::ErrorKind::Other, format!("insert_node failed!{}", e.to_string()))
    })?;

    let id = conn.last_insert_rowid();
    log::debug!("[VFS-query] insert_node 成功: id={}", id);
    Ok(id)
}

pub(crate) fn update_node_storage(conn: &rusqlite::Connection, id: i64, offset: u64, size: u64, hash: &str) -> io::Result<()> {
    log::debug!("[VFS-query] update_node_storage: id={}, offset={}, size={}, hash={}", 
        id, offset, size, hash);

    conn.execute(
        "UPDATE nodes SET storage_offset = ?, size = ?, content_hash = ?, modified_at = datetime('now') WHERE id = ?",
        params![offset as i64, size as i64, hash, id],
    )
    .map_err(|e| {
        log::error!("[VFS-query] update_node_storage 失败: id={}, error={}", id, e);
        io::Error::new(io::ErrorKind::Other, format!("update_node_storage failed!{}", e.to_string()))
    })?;

    log::debug!("[VFS-query] update_node_storage 成功: id={}", id);
    Ok(())
}

pub(crate) fn update_node_modified_at(conn: &rusqlite::Connection, id: i64) -> io::Result<()> {
    log::debug!("[VFS-query] update_node_modified_at: id={}", id);

    conn.execute(
        "UPDATE nodes SET modified_at = datetime('now') WHERE id = ?",
        params![id],
    )
    .map_err(|e| {
        log::error!("[VFS-query] update_node_modified_at 失败: id={}, error={}", id, e);
        io::Error::new(io::ErrorKind::Other, format!("update_node_modified_at failed!{}", e.to_string()))
    })?;
    Ok(())
}

pub(crate) fn soft_delete_node(conn: &rusqlite::Connection, id: i64) -> io::Result<()> {
    log::debug!("[VFS-query] soft_delete_node: id={}", id);

    conn.execute(
        "UPDATE nodes SET deleted = 1, modified_at = datetime('now') WHERE id = ?",
        params![id],
    )
    .map_err(|e| {
        log::error!("[VFS-query] soft_delete_node 失败: id={}, error={}", id, e);
        io::Error::new(io::ErrorKind::Other, format!("soft_delete_node failed!{}", e.to_string()))
    })?;

    log::debug!("[VFS-query] soft_delete_node 成功: id={}", id);
    Ok(())
}

pub(crate) fn rename_node(conn: &rusqlite::Connection, id: i64, new_name: &str) -> io::Result<()> {
    log::debug!("[VFS-query] rename_node: id={}, new_name='{}'", id, new_name);

    conn.execute(
        "UPDATE nodes SET name = ?, modified_at = datetime('now') WHERE id = ?",
        params![new_name, id],
    )
    .map_err(|e| {
        log::error!("[VFS-query] rename_node 失败: id={}, new_name='{}', error={}", id, new_name, e);
        io::Error::new(io::ErrorKind::Other, format!("rename_node failed!{}", e.to_string()))
    })?;
    Ok(())
}

pub(crate) fn move_node(conn: &rusqlite::Connection, id: i64, new_parent_id: Option<i64>, new_volume: &str) -> io::Result<()> {
    log::debug!("[VFS-query] move_node: id={}, new_parent_id={:?}, new_volume='{}'", 
        id, new_parent_id, new_volume);

    conn.execute(
        "UPDATE nodes SET parent_id = ?, volume = ?, modified_at = datetime('now') WHERE id = ?",
        params![new_parent_id, new_volume, id],
    )
    .map_err(|e| {
        log::error!("[VFS-query] move_node 失败: id={}, error={}", id, e);
        io::Error::new(io::ErrorKind::Other, format!("move_node failed!{}", e.to_string()))
    })?;
    Ok(())
}

pub(crate) fn get_storage_offset(conn: &rusqlite::Connection, id: i64) -> io::Result<(u64, u64)> {
    log::debug!("[VFS-query] get_storage_offset: id={}", id);

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
    .map_err(|e| {
        log::error!("[VFS-query] get_storage_offset 查询失败: id={}, error={}", id, e);
        io::Error::new(io::ErrorKind::Other, e.to_string())
    })
}

pub(crate) fn node_exists_path(conn: &rusqlite::Connection, path: &str) -> io::Result<bool> {
    log::debug!("[VFS-query] node_exists_path: path='{}'", path);
    
    // 处理卷根路径
    if is_volume_root(path) {
        let volume = env_system::vfs_volume(Path::new(path))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "无效的 VFS 路径"))?;
        let root_name = format!("{}:", volume);
        let node = find_node_by_name_and_parent(conn, &root_name, None, &volume)?;
        let exists = node.is_some();
        log::debug!("[VFS-query] node_exists_path 结果: {}", exists);
        return Ok(exists);
    }
    
    let exists = find_node_by_path(conn, path)?.is_some();
    log::debug!("[VFS-query] node_exists_path 结果: {}", exists);
    Ok(exists)
}

pub(crate) fn ensure_parent_dirs(conn: &rusqlite::Connection, path: &str) -> io::Result<i64> {
    log::debug!("[VFS-query] ensure_parent_dirs: path='{}'", path);

    let volume = env_system::vfs_volume(Path::new(path))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "无效的 VFS 路径"))?;
    let inner = env_system::vfs_inner_path(Path::new(path)).unwrap_or_default();
    let parent_path = Path::new(&inner).parent().map(|p| p.to_string_lossy().to_string());

    log::debug!("[VFS-query]   解析: volume='{}', inner='{}', parent='{:?}'", 
        volume, inner, parent_path);

    // 先找到卷的根节点
    let root_name = format!("{}:", volume);
    log::debug!("[VFS-query]   查根节点: name='{}', volume='{}'", root_name, volume);

    let root = find_node_by_name_and_parent(conn, &root_name, None, &volume)?
        .ok_or_else(|| {
            log::error!("[VFS-query]   卷根节点不存在: {}", root_name);
            io::Error::new(io::ErrorKind::NotFound, format!("卷根节点不存在: {}", root_name))
        })?;

    log::debug!("[VFS-query]   根节点: id={}", root.id);
    
    if let Some(parent) = parent_path {
        if !parent.is_empty() {
            let parts: Vec<&str> = parent.split('/').filter(|s| !s.is_empty()).collect();
            log::debug!("[VFS-query]   父路径组件: {:?}", parts);

            let mut current_parent = root.id;
            
            for part in parts {
                log::debug!("[VFS-query]     检查组件: '{}', 当前父节点 id={}", part, current_parent);

                let existing = find_node_by_name_and_parent(conn, part, Some(current_parent), &volume)?;
                match existing {
                    Some(node) => {
                        log::debug!("[VFS-query]       已存在: id={}", node.id);
                        current_parent = node.id;
                    }
                    None => {
                        log::debug!("[VFS-query]       不存在，创建新目录");
                        let new_id = insert_node(conn, part, "folder", Some(current_parent), &volume)?;
                        log::debug!("[VFS-query]       创建成功: id={}", new_id);
                        current_parent = new_id;
                    }
                }
            }
            log::debug!("[VFS-query]   最终父节点: id={}", current_parent);
            return Ok(current_parent);
        }
    }
    log::debug!("[VFS-query]   父路径为空，使用根节点: id={}", root.id);
    Ok(root.id)
}


fn find_node_by_name_and_parent(conn: &rusqlite::Connection, name: &str, parent_id: Option<i64>, volume: &str) -> io::Result<Option<NodeMeta>> {
    log::debug!("[VFS-query] find_node_by_name_and_parent: name='{}', parent_id={:?}, volume='{}'", 
        name, parent_id, volume);

    let mut stmt = conn.prepare(
        "SELECT id, name, node_type, parent_id, volume, content_hash,
                storage_offset, size, version, created_at, modified_at, deleted, linked_files
         FROM nodes WHERE parent_id IS ? AND name = ? AND volume = ? AND deleted = 0"
    )
    .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("find_node_by_name_and_parent failed!{}", e.to_string())))?;

    match stmt.query_row(params![parent_id, name, volume], |row| row_to_node(row)) {
        Ok(node) => {
            log::debug!("[VFS-query]   找到: id={}, type={}", node.id, node.node_type);
            Ok(Some(node))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            log::debug!("[VFS-query]   未找到");
            Ok(None)
        }
        Err(e) => {
            log::error!("[VFS-query]   查询失败: {}", e);
            Err(io::Error::new(io::ErrorKind::Other, e.to_string()))
        }
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