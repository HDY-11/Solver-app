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
        log::info!("[VFS-core] 初始化开始: db_path='{}', volumes={:?}", 
            db_path.display(), volumes.iter().map(|(v, s)| format!("{}:{}MB", v, s/1024/1024)).collect::<Vec<_>>());

        // 1. 确保数据库父目录存在
        if let Some(parent) = db_path.parent() {
            log::debug!("[VFS-core]   确保目录存在: '{}'", parent.display());
            std::fs::create_dir_all(parent)
                .inspect_log(format!("创建数据库目录失败: {}", parent.display()))?;
            log::debug!("[VFS-core]   目录已确保存在");
        }

        // 2. 创建连接池
        log::debug!("[VFS-core]   创建 SQLite 连接池...");
        let manager = SqliteConnectionManager::file(db_path);
        let pool = Pool::builder()
            .max_size(8)
            .build(manager)
            .expect_log("创建数据库连接池失败");
        log::debug!("[VFS-core]   连接池已创建 (最大连接数: 8)");

        // 3. 建表
        {
            log::debug!("[VFS-core]   获取连接，准备建表...");
            let conn = pool.get()
                .map_err(|e| {
                    log::error!("[VFS-core]   获取连接失败: {}", e);
                    io::Error::new(io::ErrorKind::Other, e.to_string())
                })?;

            log::debug!("[VFS-core]   执行建表 SQL...");
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

                -- 版本时间线表
                CREATE TABLE IF NOT EXISTS node_versions (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    node_id INTEGER NOT NULL,
                    content_hash TEXT NOT NULL,
                    storage_offset INTEGER NOT NULL,
                    size INTEGER NOT NULL,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE CASCADE,
                    UNIQUE(node_id, content_hash)
                );

                CREATE INDEX IF NOT EXISTS idx_versions_lookup
                    ON node_versions(node_id, created_at DESC);
                "
            )
            .expect_log("初始化数据库表失败");
            log::debug!("[VFS-core]   建表完成");

            // 4. 插入卷根节点
            for &(volume, max_size) in volumes {
                let root_name = format!("{}:", volume);
                log::debug!("[VFS-core]   检查卷根节点: name='{}', volume='{}'", root_name, volume);

                let exists: bool = conn
                    .query_row(
                        "SELECT COUNT(*) FROM nodes WHERE parent_id IS NULL AND name = ? AND volume = ?",
                        r2d2_sqlite::rusqlite::params![root_name, volume],
                        |row| row.get::<_, i64>(0),
                    )
                    .map(|c| {
                        let exists = c > 0;
                        log::debug!("[VFS-core]     查询结果: COUNT={}, exists={}", c, exists);
                        exists
                    })
                    .unwrap_or_else(|e| {
                        log::warn!("[VFS-core]     查询存在性失败: {}, 假定不存在", e);
                        false
                    });

                if !exists {
                    log::info!("[VFS-core]   创建卷根节点: '{}'", root_name);
                    conn.execute(
                        "INSERT INTO nodes (name, node_type, parent_id, volume) VALUES (?, 'folder', NULL, ?)",
                        r2d2_sqlite::rusqlite::params![root_name, volume],
                    )
                    .expect_log(format!("创建卷根节点失败: {}", root_name));
                    log::debug!("[VFS-core]     卷根节点已创建");
                } else {
                    log::debug!("[VFS-core]     卷根节点已存在，跳过");
                }
            }
            // conn 离开作用域，自动归还到池
        }

        // 5. 打开 BlobStore
        log::debug!("[VFS-core]   打开 BlobStore 卷...");
        let blob_pools = DashMap::new();
        for &(volume, max_size) in volumes {
            let blob_path = env_system::blob_path(volume);
            log::debug!("[VFS-core]     卷 '{}': path='{}', max_size={}MB", 
                volume, blob_path.display(), max_size / 1024 / 1024);

            let p = DataFilePool::open(&blob_path, 8, max_size)
                .inspect_log(format!("打开 BlobStore 卷失败: {}", volume))?;

            log::debug!("[VFS-core]     卷 '{}' 已打开，插入池", volume);
            blob_pools.insert(volume.to_string(), p);
        }
        log::debug!("[VFS-core]   所有 BlobStore 卷已打开 ({} 个)", blob_pools.len());

        // 6. 设置全局单例
        let vfs = Self {
            db_pool: pool,
            blob_pools,
            db_path: db_path.to_path_buf(),
        };

        log::debug!("[VFS-core]   设置全局单例...");
        VFS.set(vfs)
            .map_err(|_| {
                log::error!("[VFS-core]   VFS 已被初始化过");
                io::Error::new(io::ErrorKind::Other, "VFS 已初始化")
            })?;

        log::info!("[VFS-core] VFS 初始化完成: db='{}', {} 个卷", 
            db_path.display(), volumes.len());
        Ok(())
    }

    pub(crate) fn get() -> &'static Self {
        VFS.get().expect("VFS 未初始化")
    }

    pub(crate) fn get_pool(&self, volume: &str) -> io::Result<&DataFilePool> {
        log::debug!("[VFS-core] get_pool: volume='{}'", volume);

        self.blob_pools.get(volume)
            .map(|r| {
                log::debug!("[VFS-core]   卷 '{}' 找到", volume);
                let ptr: *const DataFilePool = &*r;
                unsafe { &*ptr }
            })
            .ok_or_else(|| {
                log::error!("[VFS-core]   卷 '{}' 不存在，可用卷: {:?}", 
                    volume, self.blob_pools.iter().map(|r| r.key().clone()).collect::<Vec<_>>());
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("卷 '{}' 不存在", volume),
                )
            })
    }

    pub(crate) fn db(&self) -> &Pool<SqliteConnectionManager> {
        &self.db_pool
    }
}