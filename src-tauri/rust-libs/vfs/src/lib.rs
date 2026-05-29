pub mod pool;
pub mod query;
pub mod vir_file;
pub mod vfs_core;

pub use vir_file::VirFile;
pub use vir_file::VfsNodeInfo;
pub use query::NodeVersionMeta;
pub use vfs_core::VirtualFileSystem;

pub fn init_vfs(db_path: &std::path::Path, volumes: &[(&str, u64)]) -> std::io::Result<()> {
    VirtualFileSystem::init(db_path, volumes)
}

/// 获取全局 VFS 单例引用
pub fn get_vfs() -> &'static VirtualFileSystem {
    VirtualFileSystem::get()
}

/// 按 linked_files 模式查询所有 run 节点（封装 DB 访问）
pub fn query_run_nodes_by_linked(pattern: &str) -> std::io::Result<Vec<query::NodeMeta>> {
    let vfs = get_vfs();
    let conn = vfs.db_pool.get()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    query::query_run_nodes_by_linked_files(&conn, pattern)
}