use std::fs::{File as StdFile, OpenOptions};
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
use crate::real_fs;

/// B/A 盘等真实文件系统卷名集合
const REAL_VOLUMES: &[&str] = &["A", "B"];
/// A 盘只读
const READONLY_VOLUMES: &[&str] = &["A"];

/// 判断是否为真实文件卷（A/B盘等）
pub fn is_real_volume(vol: &str) -> bool {
    REAL_VOLUMES.contains(&vol)
}

pub fn is_readonly_volume(vol: &str) -> bool {
    READONLY_VOLUMES.contains(&vol)
}

/// 文件后端：BlobStore 或真实文件系统
enum FileBackend {
    Blob { lease: Lease<StdFile>, pool: *const DataFilePool },
    Real { file: StdFile },
}

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
    backend: FileBackend,
    node_id: i64,
    virt_pos: u64,
    db_pool: Arc<Pool<SqliteConnectionManager>>,
    is_real: bool,
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
        let is_real = is_real_volume(&volume);

        let backend = if is_real {
            // B 盘：打开真实文件
            let real_path = real_fs::vfs_to_real(&path_str)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "无法映射到真实路径"))?;
            log::debug!("[VirFile] open: 真实路径='{}'", real_path.display());
            let file = OpenOptions::new().read(true).write(true).open(&real_path)?;
            FileBackend::Real { file }
        } else {
            // C 盘：使用 BlobStore
            let pool = vfs.get_pool(&volume)
                .inspect_log(format!("open: 获取卷池失败: volume='{}'", volume))?;
            let lease = pool.acquire();
            FileBackend::Blob { lease, pool: pool as *const DataFilePool }
        };

        log::info!("[VirFile] open 完成: path='{}', node_id={}", path_str, meta.id);
        Ok(Self { backend, node_id: meta.id, virt_pos: 0, db_pool: Arc::new(vfs.db_pool.clone()), is_real })
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

        let is_real = is_real_volume(&volume);

        let backend = if is_real {
            let real_path = real_fs::vfs_to_real(&path_str)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "无法映射到真实路径"))?;
            if let Some(parent) = real_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let file = OpenOptions::new().read(true).write(true).create(true).truncate(true).open(&real_path)?;
            FileBackend::Real { file }
        } else {
            let pool = vfs.get_pool(&volume)
                .inspect_log(format!("create: 获取卷池失败: volume='{}'", volume))?;
            let lease = pool.acquire();
            FileBackend::Blob { lease, pool: pool as *const DataFilePool }
        };

        log::info!("[VirFile] create 完成: path='{}', node_id={}", path_str, node_id);
        Ok(Self { backend, node_id, virt_pos: 0, db_pool: Arc::new(vfs.db_pool.clone()), is_real })
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

    /// 硬删除（A/B 盘：直接移除 DB 记录）
    pub fn hard_delete(self) -> io::Result<()> {
        log::info!("[VirFile] hard_delete: node_id={}", self.node_id);
        let conn = self.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
            .inspect_log("hard_delete: 获取数据库连接失败")?;
        query::hard_delete_node(&conn, self.node_id)
            .inspect_log(format!("hard_delete: 硬删除失败: id={}", self.node_id))?;
        log::info!("[VirFile] hard_delete 完成: node_id={}", self.node_id);
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

    // ── 运行记录节点（静态方法）────────────────────

    /// 创建运行记录节点（open-or-create：已存在则复用，不存在则新建）
    pub fn create_run_node(
        run_name: &str,
        linked_files_json: &str,
        volume: &str,
        run_dir: &str,
    ) -> io::Result<i64> {
        let vfs = crate::vfs_core::VirtualFileSystem::get();

        let conn = vfs.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
            .inspect_log("create_run_node: 获取数据库连接失败")?;

        let parent_id = query::ensure_parent_dirs(&conn, run_dir)
            .inspect_log("create_run_node: 创建运行记录目录失败")?;

        // open-or-create：已有同名节点则直接复用
        if let Some(existing) = query::find_node_by_name_and_parent(&conn, run_name, Some(parent_id), volume)
            .inspect_log("create_run_node: 查询已有节点失败")?
        {
            log::info!("[VirFile] create_run_node: 复用已有节点 name='{}', id={}", run_name, existing.id);
            query::update_node_linked_files(&conn, existing.id, linked_files_json)
                .inspect_log(format!("create_run_node: 更新 linked_files 失败: id={}", existing.id))?;
            return Ok(existing.id);
        }

        let node_id = query::insert_run_node(&conn, run_name, parent_id, volume, linked_files_json)
            .inspect_log(format!("create_run_node: 插入节点失败: name={}", run_name))?;

        log::info!("[VirFile] create_run_node: name='{}', id={}", run_name, node_id);
        Ok(node_id)
    }

    /// 从源节点复制 BLOB 引用，创建新的运行记录节点（去重复用存储）
    pub fn create_run_node_from_source(
        run_name: &str,
        linked_files_json: &str,
        source_offset: i64,
        source_size: i64,
        source_hash: &str,
        volume: &str,
        run_dir: &str,
    ) -> io::Result<i64> {
        let vfs = crate::vfs_core::VirtualFileSystem::get();

        let conn = vfs.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
            .inspect_log("create_run_node_from_source: 获取数据库连接失败")?;

        let parent_id = query::ensure_parent_dirs(&conn, run_dir)
            .inspect_log("create_run_node_from_source: 创建运行记录目录失败")?;

        let node_id = query::insert_run_node_from_source(
            &conn, run_name, parent_id, volume, linked_files_json,
            source_offset, source_size, source_hash,
        )
        .inspect_log(format!("create_run_node_from_source: 插入节点失败: name={}", run_name))?;

        log::info!("[VirFile] create_run_node_from_source: name='{}', id={}, 复用 BLOB (offset={}, size={})",
            run_name, node_id, source_offset, source_size);
        Ok(node_id)
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
        match &self.backend {
            FileBackend::Blob { pool, .. } => unsafe { &**pool },
            FileBackend::Real { .. } => panic!("B盘不支持 BlobStore 操作"),
        }
    }
}

// ── Read ──────────────────────────────────────────

impl Read for VirFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.is_real {
            return match &mut self.backend {
                FileBackend::Real { file, .. } => file.read(buf),
                _ => unreachable!(),
            };
        }

        log::debug!("[VirFile] read: node_id={}, virt_pos={}, buf_len={}",
            self.node_id, self.virt_pos, buf.len());

        let conn = self.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
            .inspect_log("read: 获取数据库连接失败")?;

        let (offset, size) = query::get_storage_offset(&conn, self.node_id)
            .inspect_log(format!("read: 查询偏移量失败: node_id={}", self.node_id))?;

        if self.virt_pos >= size { return Ok(0); }

        let remaining = size - self.virt_pos;
        let to_read = buf.len().min(remaining as usize);
        let read_offset = offset + self.virt_pos;

        let lease = match &self.backend {
            FileBackend::Blob { lease, .. } => lease,
            _ => unreachable!(),
        };
        let n = lease.pread_at(read_offset, &mut buf[..to_read])
            .inspect_log(format!("read: pread 失败"))?;

        self.virt_pos += n as u64;
        Ok(n)
    }
}

// ── Write ─────────────────────────────────────────

impl Write for VirFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // B 盘：直接写入真实文件
        if self.is_real {
            let n = match &mut self.backend {
                FileBackend::Real { file, .. } => {
                    let n = file.write(buf)?;
                    // 从缓冲区计算哈希（避免重新打开文件导致 Windows 共享冲突）
                    let hash: String = {
                        use sha2::{Sha256, Digest};
                        let mut hasher = Sha256::new();
                        hasher.update(&buf[..n]);
                        hex::encode(hasher.finalize())
                    };
                    // 更新 DB 元数据（size 用文件实际大小）
                    if let Ok(conn) = self.db_pool.get() {
                        let size = file.metadata().map(|m| m.len() as i64).unwrap_or(n as i64);
                        let _ = query::update_node_real_meta(&conn, self.node_id, size, &hash);
                    }
                    n
                }
                _ => unreachable!(),
            };
            return Ok(n);
        }

        // C 盘：原有 BlobStore 逻辑
        let new_hash: String = {
            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            hasher.update(buf);
            hex::encode(hasher.finalize())
        };

        let conn = self.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
            .inspect_log("write: 获取数据库连接失败")?;

        let should_skip = match query::get_content_hash(&conn, self.node_id) {
            Ok(Some(current_hash)) => {
                if current_hash == new_hash {
                    log::debug!("[VirFile] write: 内容未变, 跳过写入");
                    true
                } else { false }
            }
            _ => false,
        };

        if should_skip {
            self.virt_pos += buf.len() as u64;
            return Ok(buf.len());
        }

        if let Ok(Some(old_meta)) = query::find_node_by_id(&conn, self.node_id) {
            if let (Some(old_hash), Some(old_off), Some(old_sz)) =
                (old_meta.content_hash, old_meta.storage_offset, old_meta.size)
            {
                let _ = query::archive_version(&conn, self.node_id, &old_hash, old_off, old_sz)
                    .inspect_log(format!("write: 存档旧版本失败: node_id={}", self.node_id));
            }
        }

        let pool = self.pool();
        let new_offset = pool.alloc(buf.len())
            .inspect_log(format!("write: 分配 BlobStore 空间失败"))?;

        let lease = match &self.backend {
            FileBackend::Blob { lease, .. } => lease,
            _ => unreachable!(),
        };
        lease.pwrite_at(new_offset, buf)
            .inspect_log(format!("write: pwrite 失败"))?;

        query::archive_version(&conn, self.node_id, &new_hash, new_offset as i64, buf.len() as i64)
            .inspect_log(format!("write: 存档新版本失败"))?;
        query::update_node_storage(&conn, self.node_id, new_offset, buf.len() as u64, &new_hash)
            .inspect_log(format!("write: 更新元信息失败"))?;

        self.virt_pos += buf.len() as u64;
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
        if self.is_real {
            return match &mut self.backend {
                FileBackend::Real { file, .. } => file.seek(pos),
                _ => unreachable!(),
            };
        }

        let new_pos = match pos {
            SeekFrom::Start(n) => n,
            SeekFrom::End(offset) => {
                let conn = self.db_pool.get()
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
                let (_, size) = query::get_storage_offset(&conn, self.node_id)?;
                let end = size as i64 + offset;
                if end < 0 {
                    return Err(io::Error::new(io::ErrorKind::InvalidInput, "seek 到负位置"));
                }
                end as u64
            }
            SeekFrom::Current(offset) => {
                let current = self.virt_pos as i64 + offset;
                if current < 0 {
                    return Err(io::Error::new(io::ErrorKind::InvalidInput, "seek 到负位置"));
                }
                current as u64
            }
        };
        self.virt_pos = new_pos;
        Ok(new_pos)
    }
}

impl Drop for VirFile {
    fn drop(&mut self) {
        log::debug!("[VirFile] drop: node_id={}", self.node_id);
        if let Ok(conn) = self.db_pool.get() {
            if let Err(e) = query::update_node_modified_at(&conn, self.node_id) {
                log::warn!("[VirFile] drop: 更新 modified_at 失败: {}", e);
            }
        }
    }
}

unsafe impl Send for VirFile {}