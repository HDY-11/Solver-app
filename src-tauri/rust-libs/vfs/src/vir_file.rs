use std::fs::File as StdFile;
use std::io::{self, Read, Write, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use utils::Lease;
use error_system::ResultLogExt;
use crate::pool::DataFilePool;
use crate::query;

pub struct VirFile {
    lease: Lease<StdFile>,
    pool_ref: *const DataFilePool,
    node_id: i64,
    virt_pos: u64,
    volume: String,
    db_pool: Arc<Pool<SqliteConnectionManager>>,
}

impl VirFile {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let vfs = crate::vfs_core::VirtualFileSystem::get();
        let path_str = path.as_ref().to_string_lossy();

        let conn = vfs.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        let meta = query::find_node_by_path(&conn, &path_str)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("文件不存在: {}", path_str)))?;

        let volume = meta.volume.clone();
        let pool = vfs.get_pool(&volume)?;

        Ok(Self {
            lease: pool.acquire(),
            pool_ref: pool as *const DataFilePool,
            node_id: meta.id,
            virt_pos: 0,
            volume,
            db_pool: Arc::new(vfs.db_pool.clone()),
        })
    }

    pub fn create(path: impl AsRef<Path>) -> io::Result<Self> {
        let vfs = crate::vfs_core::VirtualFileSystem::get();
        let path_str = path.as_ref().to_string_lossy();

        let volume = env_system::vfs_volume(path.as_ref())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "无效的 VFS 路径"))?;
        let inner = env_system::vfs_inner_path(path.as_ref()).unwrap_or_default();
        let name = Path::new(&inner)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "item.txt".to_string());

        let conn = vfs.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        // 确保父目录存在
        query::ensure_parent_dirs(&conn, &path_str)
            .inspect_log(format!("创建父目录失败: {}", path_str))?;

        let parent_path = Path::new(&inner)
            .parent()
            .and_then(|p| if p.as_os_str().is_empty() { None } else { Some(p.to_string_lossy()) });

        let parent_id = if let Some(pp) = parent_path {
            let vfs_parent = env_system::vfs_path(&volume, &pp);
            let parent_meta = query::find_node_by_path(&conn, &vfs_parent.to_string_lossy())?;
            parent_meta.map(|m| m.id)
        } else {
            None
        };

        let node_id = query::insert_node(&conn, &name, "file", parent_id, &volume)?;

        let pool = vfs.get_pool(&volume)?;

        Ok(Self {
            lease: pool.acquire(),
            pool_ref: pool as *const DataFilePool,
            node_id,
            virt_pos: 0,
            volume,
            db_pool: Arc::new(vfs.db_pool.clone()),
        })
    }

    pub fn node_id(&self) -> i64 {
        self.node_id
    }

    pub fn list_children(path: impl AsRef<Path>) -> io::Result<Vec<query::NodeMeta>> {
        let vfs = crate::vfs_core::VirtualFileSystem::get();
        let conn = vfs.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        query::list_children_by_path(&conn, &path.as_ref().to_string_lossy())
    }

    pub fn delete(path: impl AsRef<Path>) -> io::Result<()> {
        let vfs = crate::vfs_core::VirtualFileSystem::get();
        let conn = vfs.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        let meta = query::find_node_by_path(&conn, &path.as_ref().to_string_lossy())?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "文件不存在"))?;
        query::soft_delete_node(&conn, meta.id)
    }

    pub fn create_dir(path: impl AsRef<Path>) -> io::Result<()> {
        let vfs = crate::vfs_core::VirtualFileSystem::get();
        let path_str = path.as_ref().to_string_lossy();
        let volume = env_system::vfs_volume(path.as_ref())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "无效的 VFS 路径"))?;
        let inner = env_system::vfs_inner_path(path.as_ref()).unwrap_or_default();
        let name = Path::new(&inner)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "新文件夹".to_string());

        let conn = vfs.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        query::ensure_parent_dirs(&conn, &path_str)?;

        let parent_path = Path::new(&inner)
            .parent()
            .and_then(|p| if p.as_os_str().is_empty() { None } else { Some(p.to_string_lossy()) });
        let parent_id = if let Some(pp) = parent_path {
            let vfs_parent = env_system::vfs_path(&volume, &pp);
            let parent_meta = query::find_node_by_path(&conn, &vfs_parent.to_string_lossy())?;
            parent_meta.map(|m| m.id)
        } else {
            None
        };

        query::insert_node(&conn, &name, "folder", parent_id, &volume)?;
        Ok(())
    }

    pub fn exists(path: impl AsRef<Path>) -> io::Result<bool> {
        let vfs = crate::vfs_core::VirtualFileSystem::get();
        let conn = vfs.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        query::node_exists_path(&conn, &path.as_ref().to_string_lossy())
    }

    fn pool(&self) -> &DataFilePool {
        unsafe { &*self.pool_ref }
    }
}

// ── Read ──────────────────────────────────────────

impl Read for VirFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let conn = self.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        let (offset, size) = query::get_storage_offset(&conn, self.node_id)?;

        if self.virt_pos >= size {
            return Ok(0);
        }

        let remaining = size - self.virt_pos;
        let to_read = buf.len().min(remaining as usize);
        let read_offset = offset + self.virt_pos;
        let n = self.pool().read_at(&self.lease, read_offset, &mut buf[..to_read])?;
        self.virt_pos += n as u64;
        Ok(n)
    }
}

// ── Write ─────────────────────────────────────────

impl Write for VirFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let pool = self.pool();
        let new_offset = pool.alloc(buf.len())
            .inspect_log("分配 BlobStore 空间失败")?;

        pool.write_at(&self.lease, new_offset, buf)
            .inspect_log(format!("写入 BlobStore 失败: offset={}", new_offset))?;

        let hash = {
            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            hasher.update(buf);
            hex::encode(hasher.finalize())
        };

        let conn = self.db_pool.get()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        query::update_node_storage(&conn, self.node_id, new_offset, buf.len() as u64, &hash)?;

        self.virt_pos += buf.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// ── Seek ──────────────────────────────────────────

impl Seek for VirFile {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
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
        if let Ok(conn) = self.db_pool.get() {
            let _ = query::update_node_modified_at(&conn, self.node_id);
        }
    }
}

unsafe impl Send for VirFile {}
// VirFile 不 Sync — Lease 不 Sync