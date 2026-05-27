use std::fs::File as StdFile;
use std::io::{self, Read, Write, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use utils::Lease;
use error_system::ResultLogExt;
use serde::Serialize;
use crate::pool::DataFilePool;
use crate::pool::LeaseFileExt;
use crate::query;

#[derive(Debug, Clone, Serialize)]
pub struct VfsNodeInfo {
    pub id: i64,
    pub name: String,
    pub node_type: String,
    pub size: Option<u64>,
    pub modified_at: String,
    pub version: String,
}

pub struct VirFile {
    lease: Lease<StdFile>,
    pool_ref: *const DataFilePool,
    node_id: i64,
    virt_pos: u64,
    db_pool: Arc<Pool<SqliteConnectionManager>>,
}

impl VirFile {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let vfs = crate::vfs_core::VirtualFileSystem::get();
        let path_str = path.as_ref().to_string_lossy();
        log::info!("[VirFile] open 开始: path='{}'", path_str);

        let conn = vfs.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
            .inspect_log("open: 获取数据库连接失败")?;

        let meta = query::find_node_by_path(&conn, &path_str)
            .inspect_log(format!("open: 查找节点失败: path='{}'", path_str))?
            .ok_or_else(|| {
                log::error!("[VirFile] open: 文件不存在: '{}'", path_str);
                io::Error::new(io::ErrorKind::NotFound, format!("文件不存在: {}", path_str))
            })?;

        log::debug!("[VirFile] open: 找到节点 id={}, name='{}', type={}, volume='{}'", 
            meta.id, meta.name, meta.node_type, meta.volume);

        let volume = meta.volume.clone();
        let pool = vfs.get_pool(&volume)
            .inspect_log(format!("open: 获取卷池失败: volume='{}'", volume))?;

        log::debug!("[VirFile] open: 借出文件句柄...");
        let lease = pool.acquire();
        log::info!("[VirFile] open 完成: path='{}', node_id={}", path_str, meta.id);

        Ok(Self {
            lease,
            pool_ref: pool as *const DataFilePool,
            node_id: meta.id,
            virt_pos: 0,
            db_pool: Arc::new(vfs.db_pool.clone()),
        })
    }

    pub fn create(path: impl AsRef<Path>) -> io::Result<Self> {
        let vfs = crate::vfs_core::VirtualFileSystem::get();
        let path_str = path.as_ref().to_string_lossy();
        log::info!("[VirFile] create 开始: path='{}'", path_str);

        let volume = env_system::vfs_volume(path.as_ref())
            .ok_or_else(|| {
                log::error!("[VirFile] create: 无效的 VFS 路径: '{}'", path_str);
                io::Error::new(io::ErrorKind::InvalidInput, "无效的 VFS 路径")
            })?;
        let inner = env_system::vfs_inner_path(path.as_ref()).unwrap_or_default();
        let name = Path::new(&inner)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "item.txt".to_string());

        log::debug!("[VirFile] create: volume='{}', inner='{}', name='{}'", volume, inner, name);

        let conn = vfs.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
            .inspect_log("create: 获取数据库连接失败")?;

        let parent_id = query::ensure_parent_dirs(&conn, &path_str)
            .inspect_log(format!("create: 创建父目录失败: path='{}'", path_str))?;

        log::debug!("[VirFile] create: 父目录已确保, parent_id={}", parent_id);

        let node_id = query::insert_node(&conn, &name, "file", Some(parent_id), &volume)
            .inspect_log(format!("create: 插入节点失败: name='{}', parent_id={}, volume='{}'", 
                name, parent_id, volume))?;

        log::debug!("[VirFile] create: 节点已创建, node_id={}", node_id);

        let pool = vfs.get_pool(&volume)
            .inspect_log(format!("create: 获取卷池失败: volume='{}'", volume))?;

        log::debug!("[VirFile] create: 借出文件句柄...");
        let lease = pool.acquire();
        log::info!("[VirFile] create 完成: path='{}', node_id={}", path_str, node_id);

        Ok(Self {
            lease,
            pool_ref: pool as *const DataFilePool,
            node_id,
            virt_pos: 0,
            db_pool: Arc::new(vfs.db_pool.clone()),
        })
    }

    pub(crate) fn node_id(&self) -> i64 {
        self.node_id
    }

    pub fn list_children(path: impl AsRef<Path>) -> io::Result<Vec<VfsNodeInfo>> {
        let path_ref = path.as_ref();
        let path_str = path_ref.to_string_lossy();
        log::debug!("[VirFile] list_children: path='{}'", path_str);

        let vfs = crate::vfs_core::VirtualFileSystem::get();
        let conn = vfs.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
            .inspect_log("list_children: 获取数据库连接失败")?;

        let metas = query::list_children_by_path(&conn, &path_str)
            .inspect_log(format!("list_children: 查询失败: path='{}'", path_str))?;

        let result: Vec<VfsNodeInfo> = metas.iter().map(|m| VfsNodeInfo {
            id: m.id,
            name: m.name.clone(),
            node_type: m.node_type.clone(),
            size: m.size.map(|s| s as u64),
            modified_at: m.modified_at.clone(),
            version: m.version.clone(),
        }).collect();

        log::debug!("[VirFile] list_children 完成: path='{}', 子节点数={}", path_str, result.len());
        Ok(result)
    }

    pub fn delete(self) -> io::Result<()> {
        log::info!("[VirFile] delete: node_id={}", self.node_id);
        let conn = self.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
            .inspect_log("delete: 获取数据库连接失败")?;
        query::soft_delete_node(&conn, self.node_id)
            .inspect_log(format!("delete: 软删除失败: id={}", self.node_id))?;
        log::info!("[VirFile] delete 完成: node_id={}", self.node_id);
        Ok(())
    }

    pub fn rename(&self, new_name: &str) -> io::Result<()> {
        log::info!("[VirFile] rename: node_id={} → '{}'", self.node_id, new_name);
        let conn = self.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
            .inspect_log("rename: 获取数据库连接失败")?;
        query::rename_node(&conn, self.node_id, new_name)
            .inspect_log(format!("rename: 重命名失败: id={}", self.node_id))?;
        log::info!("[VirFile] rename 完成: node_id={} → '{}'", self.node_id, new_name);
        Ok(())
    }

    /// 设置节点版本号（用户手动修改）
    pub fn set_version(&self, new_version: &str) -> io::Result<()> {
        log::info!("[VirFile] set_version: node_id={} → '{}'", self.node_id, new_version);
        let conn = self.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
            .inspect_log("set_version: 获取数据库连接失败")?;
        query::set_node_version(&conn, self.node_id, new_version)
            .inspect_log(format!("set_version: 设置版本失败: id={}", self.node_id))?;
        log::info!("[VirFile] set_version 完成: node_id={} → '{}'", self.node_id, new_version);
        Ok(())
    }

    /// 获取当前内容哈希（用于写入前去重判断）
    pub fn current_hash(&self) -> io::Result<Option<String>> {
        let conn = self.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
            .inspect_log("current_hash: 获取数据库连接失败")?;
        query::get_content_hash(&conn, self.node_id)
    }

    /// 获取节点版本号
    pub fn version(&self) -> io::Result<String> {
        let conn = self.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
            .inspect_log("version: 获取数据库连接失败")?;
        let meta = query::find_node_by_id(&conn, self.node_id)
            .inspect_log(format!("version: 查询节点失败: id={}", self.node_id))?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "节点不存在"))?;
        Ok(meta.version)
    }

    pub fn create_dir(path: impl AsRef<Path>) -> io::Result<()> {
        let path_ref = path.as_ref();
        let path_str = path_ref.to_string_lossy();
        log::info!("[VirFile] create_dir: path='{}'", path_str);

        let vfs = crate::vfs_core::VirtualFileSystem::get();
        let volume = env_system::vfs_volume(path_ref)
            .ok_or_else(|| {
                log::error!("[VirFile] create_dir: 无效的 VFS 路径: '{}'", path_str);
                io::Error::new(io::ErrorKind::InvalidInput, "无效的 VFS 路径")
            })?;
        let inner = env_system::vfs_inner_path(path_ref).unwrap_or_default();
        let name = Path::new(&inner)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "新文件夹".to_string());

        log::debug!("[VirFile] create_dir: volume='{}', inner='{}', name='{}'", volume, inner, name);

        let conn = vfs.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
            .inspect_log("create_dir: 获取数据库连接失败")?;

        let parent_id = query::ensure_parent_dirs(&conn, &path_str)
            .inspect_log(format!("create_dir: 创建父目录失败: path='{}'", path_str))?;

        log::debug!("[VirFile] create_dir: 父目录已确保, parent_id={}", parent_id);

        let node_id = query::insert_node(&conn, &name, "folder", Some(parent_id), &volume)
            .inspect_log(format!("create_dir: 插入节点失败: name='{}', parent_id={}, volume='{}'", 
                name, parent_id, volume))?;

        log::info!("[VirFile] create_dir 完成: path='{}', node_id={}", path_str, node_id);
        Ok(())
    }

    pub fn exists(path: impl AsRef<Path>) -> io::Result<bool> {
        let path_str = path.as_ref().to_string_lossy();
        let vfs = crate::vfs_core::VirtualFileSystem::get();
        let conn = vfs.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
            .inspect_log("exists: 获取数据库连接失败")?;
        query::node_exists_path(&conn, &path_str)
    }

    /// 获取某节点的版本时间线列表
    pub fn list_versions(path: impl AsRef<Path>) -> io::Result<Vec<query::NodeVersionMeta>> {
        let path_str = path.as_ref().to_string_lossy();
        let vfs = crate::vfs_core::VirtualFileSystem::get();
        let conn = vfs.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
            .inspect_log("list_versions: 获取数据库连接失败")?;
        let meta = query::find_node_by_path(&conn, &path_str)
            .inspect_log(format!("list_versions: 查找节点失败: {}", path_str))?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "文件不存在"))?;
        query::get_version_history(&conn, meta.id)
    }

    /// 从 BlobStore 读取指定版本的原始字节
    pub fn read_version(path: impl AsRef<Path>, content_hash: &str) -> io::Result<Vec<u8>> {
        let path_str = path.as_ref().to_string_lossy();
        let vfs = crate::vfs_core::VirtualFileSystem::get();

        // 解析卷名 → 获取 BlobStore 连接
        let volume = env_system::vfs_volume(path.as_ref())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "无效的 VFS 路径"))?;
        let pool = vfs.get_pool(&volume)?;

        let conn = vfs.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
            .inspect_log("read_version: 获取数据库连接失败")?;

        let meta = query::find_node_by_path(&conn, &path_str)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "文件不存在"))?;

        let version = query::find_version_by_hash(&conn, meta.id, content_hash)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "版本不存在"))?;

        let lease = pool.acquire();
        let mut buf = vec![0u8; version.size as usize];
        lease.pread_at(version.storage_offset as u64, &mut buf)?;
        Ok(buf)
    }

    fn pool(&self) -> &DataFilePool {
        unsafe { &*self.pool_ref }
    }
}

// ── Read ──────────────────────────────────────────

impl Read for VirFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        log::debug!("[VirFile] read: node_id={}, virt_pos={}, buf_len={}", 
            self.node_id, self.virt_pos, buf.len());

        let conn = self.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
            .inspect_log("read: 获取数据库连接失败")?;

        let (offset, size) = query::get_storage_offset(&conn, self.node_id)
            .inspect_log(format!("read: 查询偏移量失败: node_id={}", self.node_id))?;

        log::debug!("[VirFile] read: offset={}, size={}, virt_pos={}", offset, size, self.virt_pos);

        if self.virt_pos >= size {
            log::debug!("[VirFile] read: 已到文件末尾");
            return Ok(0);
        }

        let remaining = size - self.virt_pos;
        let to_read = buf.len().min(remaining as usize);
        let read_offset = offset + self.virt_pos;

        log::debug!("[VirFile] read: read_offset={}, to_read={}", read_offset, to_read);

        let n = self.lease.pread_at(read_offset, &mut buf[..to_read])
            .inspect_log(format!("read: pread 失败: offset={}, len={}", read_offset, to_read))?;

        self.virt_pos += n as u64;
        log::debug!("[VirFile] read 完成: 读取 {} 字节, 新 virt_pos={}", n, self.virt_pos);
        Ok(n)
    }
}

// ── Write ─────────────────────────────────────────

impl Write for VirFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // 1. 先计算新内容的哈希
        let new_hash: String = {
            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            hasher.update(buf);
            hex::encode(hasher.finalize())
        };

        let conn = self.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
            .inspect_log("write: 获取数据库连接失败")?;

        // 2. 哈希去重：若内容未变，跳过写入
        if let Some(current_hash) = query::get_content_hash(&conn, self.node_id)
            .inspect_log(format!("write: 查询当前哈希失败: node_id={}", self.node_id))?
        {
            if current_hash == new_hash {
                log::debug!("[VirFile] write: 内容未变 (hash={}), 跳过写入 node_id={}",
                    &new_hash[..8], self.node_id);
                self.virt_pos += buf.len() as u64;
                return Ok(buf.len());
            }
        }

        // 3. 存档当前版本到 node_versions
        if let Ok(Some(old_meta)) = query::find_node_by_id(&conn, self.node_id) {
            if let (Some(old_hash), Some(old_off), Some(old_sz)) =
                (old_meta.content_hash, old_meta.storage_offset, old_meta.size)
            {
                query::archive_version(&conn, self.node_id, &old_hash, old_off, old_sz)
                    .inspect_log(format!("write: 存档旧版本失败: node_id={}", self.node_id))?;
            }
        }

        let pool = self.pool();

        let new_offset = pool.alloc(buf.len())
            .inspect_log(format!("write: 分配 BlobStore 空间失败: len={}", buf.len()))?;

        self.lease.pwrite_at(new_offset, buf)
            .inspect_log(format!("write: pwrite 失败: offset={}, len={}", new_offset, buf.len()))?;

        // 4. 新内容存档
        query::archive_version(&conn, self.node_id, &new_hash, new_offset as i64, buf.len() as i64)
            .inspect_log(format!("write: 存档新版本失败: node_id={}", self.node_id))?;

        // 5. 更新 nodes 表的当前存储信息
        query::update_node_storage(&conn, self.node_id, new_offset, buf.len() as u64, &new_hash)
            .inspect_log(format!("write: 更新元信息失败: node_id={}", self.node_id))?;

        self.virt_pos += buf.len() as u64;
        log::debug!("[VirFile] write: 写入 {} 字节, hash={}, node_id={}",
            buf.len(), &new_hash[..8], self.node_id);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        // BlobStore 无用户态缓冲区
        Ok(())
    }
}

// ── Seek ──────────────────────────────────────────

impl Seek for VirFile {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        log::debug!("[VirFile] seek: node_id={}, 当前 virt_pos={}, pos={:?}", 
            self.node_id, self.virt_pos, pos);

        let new_pos = match pos {
            SeekFrom::Start(n) => n,
            SeekFrom::End(offset) => {
                let conn = self.db_pool.get()
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
                    .inspect_log("seek: 获取数据库连接失败")?;
                let (_, size) = query::get_storage_offset(&conn, self.node_id)
                    .inspect_log(format!("seek: 查询文件大小失败: node_id={}", self.node_id))?;

                let end = size as i64 + offset;
                if end < 0 {
                    log::error!("[VirFile] seek: SeekFrom::End({}) 导致负位置: size={}", offset, size);
                    return Err(io::Error::new(io::ErrorKind::InvalidInput, "seek 到负位置"));
                }
                end as u64
            }
            SeekFrom::Current(offset) => {
                let current = self.virt_pos as i64 + offset;
                if current < 0 {
                    log::error!("[VirFile] seek: SeekFrom::Current({}) 导致负位置: virt_pos={}", 
                        offset, self.virt_pos);
                    return Err(io::Error::new(io::ErrorKind::InvalidInput, "seek 到负位置"));
                }
                current as u64
            }
        };

        log::debug!("[VirFile] seek: 新位置={}", new_pos);
        self.virt_pos = new_pos;
        Ok(new_pos)
    }
}

impl Drop for VirFile {
    fn drop(&mut self) {
        log::debug!("[VirFile] drop: node_id={}, 归还文件句柄", self.node_id);

        if let Ok(conn) = self.db_pool.get() {
            if let Err(e) = query::update_node_modified_at(&conn, self.node_id) {
                log::warn!("[VirFile] drop: 更新 modified_at 失败: node_id={}, error={}", 
                    self.node_id, e);
            }
        }
    }
}

unsafe impl Send for VirFile {}
// VirFile 不 Sync — Lease 不 Sync