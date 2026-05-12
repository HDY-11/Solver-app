//! DataFilePool —— 一个 BlobStore 卷的句柄池和偏移量分配器。
//!
//! 每个卷（C, D1, D2...）对应一个 DataFilePool。
//! 句柄池为 8 个 Slot<File>，并发借出。
//! 偏移量分配用 AtomicU64 保证无锁。

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use utils::{Slot, Lease};
use error_system::ResultLogExt;

pub struct DataFilePool {
    slots: Vec<Arc<Slot<File>>>,
    current_offset: AtomicU64,
    max_size: u64,
}

impl DataFilePool {
    /// 打开或创建 BlobStore 文件。
    pub fn open(path: &Path, capacity: usize, max_size: u64) -> io::Result<Self> {
        let path = path.to_path_buf();
        
        // 确保父目录存在
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)
            .inspect_log(format!("打开 BlobStore 文件失败: {}", path.display()))?;

        let current_size = file.metadata()
            .inspect_log("获取 BlobStore 文件元数据失败")?
            .len();

        let mut slots = Vec::with_capacity(capacity);
        for i in 0..capacity {
            let dup = if i == 0 {
                file.try_clone()
                    .inspect_log("复制文件描述符失败")?
            } else {
                file.try_clone()
                    .inspect_log("复制文件描述符失败")?
            };
            slots.push(Slot::new(dup));
        }

        log::info!(
            "BlobStore 卷已打开: {} (容量: {} MB, 当前: {} bytes)",
            path.display(),
            max_size / 1024 / 1024,
            current_size
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
        
        if new_pos > self.max_size {
            // 回退偏移量（尽力而为）
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
        loop {
            for slot in &self.slots {
                if let Some(lease) = slot.try_lease() {
                    return lease;
                }
            }
            std::hint::spin_loop();
        }
    }

    /// 在指定偏移量写入数据。
    ///
    /// 使用 pwrite，不同偏移量的写入由内核保证并发安全。
    pub fn write_at(&self, lease: &Lease<File>, offset: u64, data: &[u8]) -> io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileExt;
            lease.write_at(data, offset)
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::FileExt;
            lease.seek_write(data, offset).map(|_| ())
        }
    }

    /// 从指定偏移量读取数据。
    pub fn read_at(&self, lease: &Lease<File>, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileExt;
            lease.read_at(buf, offset)
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::FileExt;
            lease.seek_read(buf, offset)
        }
    }

    /// 当前文件大小。
    pub fn file_size(&self) -> u64 {
        self.current_offset.load(Ordering::Relaxed)
    }
}