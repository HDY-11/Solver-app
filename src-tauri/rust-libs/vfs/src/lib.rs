pub mod pool;
pub mod query;
pub mod vir_file;
mod vfs_core;

pub use vir_file::VirFile;
pub use vir_file::VfsNodeInfo;

pub fn init_vfs(db_path: &std::path::Path, volumes: &[(&str, u64)]) -> std::io::Result<()> {
    vfs_core::VirtualFileSystem::init(db_path, volumes)
}