//! DataFilePool —— 一个 BlobStore 卷的句柄池和偏移量分配器。
//!
//! 每个卷（C, D1, D2...）对应一个 DataFilePool。
//! 句柄池为 8 个 Slot<File>，并发借出。
//! 偏移量分配用 AtomicU64 保证无锁。
//!
//! pwrite / pread 通过 `LeaseFileExt` 扩展 trait 直接挂在 `Lease<File>` 上，
//! 不归 DataFilePool 管。

use std::fs::{File, OpenOptions};
use std::io::{self};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use utils::{Slot, Lease};
use error_system::ResultLogExt;

// ── Lease<File> 扩展：pwrite / pread ──────────────

/// 为 `Lease<File>` 提供跨平台的指定偏移量读写。
/// 替代原来挂在 `DataFilePool` 上的 `write_at` / `read_at`，
/// 让 Pool 职责纯粹为「偏移分配 + 句柄借出」。
pub trait LeaseFileExt {
    fn pwrite_at(&self, offset: u64, data: &[u8]) -> io::Result<()>;
    fn pread_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<usize>;
}

impl LeaseFileExt for Lease<File> {
    fn pwrite_at(&self, offset: u64, data: &[u8]) -> io::Result<()> {
        #[cfg(unix)]
        {
            std::os::unix::fs::FileExt::write_at(self, data, offset)
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::FileExt;
            self.seek_write(data, offset).map(|_| ())
        }
    }

    fn pread_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        #[cfg(unix)]
        {
            std::os::unix::fs::FileExt::read_at(self, buf, offset)
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::FileExt;
            self.seek_read(buf, offset)
        }
    }
}

// ── DataFilePool ─────────────────────────────────

pub struct DataFilePool {
    slots: Vec<Arc<Slot<File>>>,
    current_offset: AtomicU64,
    max_size: u64,
}

impl DataFilePool {
    /// 打开或创建 BlobStore 文件。
    pub fn open(path: &Path, capacity: usize, max_size: u64) -> io::Result<Self> {
        let path = path.to_path_buf();
        log::info!("[DataFilePool] open: path='{}', capacity={}, max_size={}MB", 
            path.display(), capacity, max_size / 1024 / 1024);
        
        // 确保父目录存在
        if let Some(parent) = path.parent() {
            log::debug!("[DataFilePool]   确保父目录存在: '{}'", parent.display());
            std::fs::create_dir_all(parent)
                .inspect_log(format!("创建 BlobStore 目录失败: {}", parent.display()))?;
        }

        log::debug!("[DataFilePool]   打开文件 (create=true, read=true, write=true)...");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)
            .inspect_log(format!("打开 BlobStore 文件失败: {}", path.display()))?;

        let current_size = file.metadata()
            .inspect_log("获取 BlobStore 文件元数据失败")?
            .len();

        log::debug!("[DataFilePool]   文件已打开, 当前大小: {} bytes", current_size);

        let mut slots = Vec::with_capacity(capacity);
        for i in 0..capacity {
            log::trace!("[DataFilePool]   创建句柄副本 {}/{}", i + 1, capacity);
            let dup = file.try_clone()
                .inspect_log(format!("复制文件描述符失败 (副本 {}/{})", i + 1, capacity))?;
            slots.push(Slot::new(dup));
        }

        log::debug!("[DataFilePool]   句柄池已创建: {} 个槽位", slots.len());

        log::info!(
            "[DataFilePool] BlobStore 卷已打开: {} (容量: {} MB, 当前: {} bytes, 句柄池: {})",
            path.display(),
            max_size / 1024 / 1024,
            current_size,
            slots.len()
        );

        Ok(Self {
            slots,
            current_offset: AtomicU64::new(current_size),
            max_size,
        })
    }

    /// 分配空间，返回起始偏移量。
    ///
    /// 原子操作，多线程安全。
    pub fn alloc(&self, size: usize) -> io::Result<u64> {
        let offset = self.current_offset.fetch_add(size as u64, Ordering::SeqCst);
        let new_pos = offset + size as u64;
        
        log::trace!("[DataFilePool] alloc: size={}, offset={}, new_pos={}, max={}", 
            size, offset, new_pos, self.max_size);
        
        if new_pos > self.max_size {
            // 回退偏移量（尽力而为）
            log::warn!("[DataFilePool] alloc: 卷容量不足! 需要={}, 当前偏移量={}, 最大={}MB",
                size, offset, self.max_size / 1024 / 1024);
            self.current_offset.store(offset, Ordering::SeqCst);
            return Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                format!(
                    "卷容量不足: 需要 {} 字节 (当前偏移量: {}, 最大: {} MB)",
                    size,
                    offset,
                    self.max_size / 1024 / 1024
                ),
            ));
        }

        Ok(offset)
    }

    /// 借出一个文件句柄。
    pub fn acquire(&self) -> Lease<File> {
        log::trace!("[DataFilePool] acquire: 尝试借出句柄...");
        loop {
            for (i, slot) in self.slots.iter().enumerate() {
                if let Some(lease) = slot.try_lease() {
                    log::trace!("[DataFilePool] acquire: 借出槽位 {}", i);
                    return lease;
                }
            }
            log::trace!("[DataFilePool] acquire: 所有槽位被占用，自旋等待...");
            std::hint::spin_loop();
        }
    }

    /// 当前文件大小（= 已分配偏移量）。
    pub fn file_size(&self) -> u64 {
        self.current_offset.load(Ordering::Relaxed)
    }
}