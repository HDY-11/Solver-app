use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use dashmap::DashMap;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use error_system::ResultLogExt;
use crate::pool::DataFilePool;
use crate::query;

static VFS: OnceLock<VirtualFileSystem> = OnceLock::new();

pub(crate) struct VirtualFileSystem {
    pub(crate) db_pool: Pool<SqliteConnectionManager>,
    pub(crate) blob_pools: DashMap<String, DataFilePool>,
    pub(crate) db_path: PathBuf,
}

impl VirtualFileSystem {
    pub fn init(db_path: &Path, volumes: &[(&str, u64)]) -> io::Result<()> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .inspect_log(format!("创建数据库目录失败: {}", parent.display()))?;
        }

        let manager = SqliteConnectionManager::file(db_path);
        let pool = Pool::builder()
            .max_size(8)
            .build(manager)
            .expect_log("创建数据库连接池失败");

        {
            let conn = pool.get()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
            conn.execute_batch(
                "
                PRAGMA journal_mode=WAL;
                PRAGMA foreign_keys=ON;

                CREATE TABLE IF NOT EXISTS nodes (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL DEFAULT 'item.txt',
                    node_type TEXT NOT NULL DEFAULT 'file'
                        CHECK(node_type IN ('file', 'folder', 'run')),
                    parent_id INTEGER REFERENCES nodes(id),
                    volume TEXT NOT NULL,
                    content_hash TEXT,
                    storage_offset INTEGER,
                    size INTEGER,
                    version TEXT NOT NULL DEFAULT '0.1.0',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    modified_at TEXT NOT NULL DEFAULT (datetime('now')),
                    deleted INTEGER NOT NULL DEFAULT 0,
                    linked_files TEXT
                );

                CREATE INDEX IF NOT EXISTS idx_nodes_parent ON nodes(parent_id);
                CREATE INDEX IF NOT EXISTS idx_nodes_hash ON nodes(content_hash);
                CREATE UNIQUE INDEX IF NOT EXISTS idx_nodes_name
                    ON nodes(parent_id, name, volume) WHERE deleted = 0;
                "
            )
            .expect_log("初始化数据库表失败");
        }

        let blob_pools = DashMap::new();
        for &(volume, max_size) in volumes {
            let blob_path = env_system::blob_path(volume);
            let p = DataFilePool::open(&blob_path, 8, max_size)
                .inspect_log(format!("打开 BlobStore 卷失败: {}", volume))?;
            blob_pools.insert(volume.to_string(), p);
        }

        let vfs = Self {
            db_pool: pool,
            blob_pools,
            db_path: db_path.to_path_buf(),
        };

        VFS.set(vfs)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "VFS 已初始化"))?;

        log::info!("VFS 初始化完成");
        Ok(())
    }

    pub(crate) fn get() -> &'static Self {
        VFS.get().expect("VFS 未初始化")
    }

    pub(crate) fn get_pool(&self, volume: &str) -> io::Result<&DataFilePool> {
        self.blob_pools.get(volume)
            .map(|r| {
                let ptr: *const DataFilePool = &*r;
                unsafe { &*ptr }
            })
            .ok_or_else(|| io::Error::new(
                io::ErrorKind::NotFound,
                format!("卷 '{}' 不存在", volume),
            ))
    }

    pub(crate) fn db(&self) -> &Pool<SqliteConnectionManager> {
        &self.db_pool
    }
}