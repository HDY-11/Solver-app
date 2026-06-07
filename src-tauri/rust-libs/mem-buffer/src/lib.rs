//! mem-buffer — 环形内存缓冲区
//!
//! 为每个标签页提供独立的输出缓冲区，实现 `std::io::Write` 和 `std::io::Read` trait。
//! 设计目标：降低前端渲染压力，避免每次 println 都通过事件系统推送到前端。
//!
//! # 架构
//!
//! ```text
//! MemBuffer
//!   ├── Write trait → Lua print() 写入（追加到环形缓冲）
//!   ├── Read trait  → 前端按需拉取（mem_buffer_read / mem_buffer_get_all）
//!   └── 导出       → cmdv_export 读取全部内容写入文件
//! ```
//!
//! # 复杂度
//! - write: O(n) 逐字节写入环形缓冲
//! - get_all: O(n) 复制有效数据到连续 Vec
//! - get_range: O(n) 同 get_all
//! - read: O(min(buf.len(), data_len)) 消费者端读取
//! - clear: O(1)
//!
//! # 使用示例
//!
//! ```ignore
//! use mem_buffer::MemBuffer;
//! use std::io::Write;
//!
//! let mut buf = MemBuffer::new(64 * 1024);
//! write!(buf, "Hello, {}!", "Lua")?;
//! assert!(String::from_utf8_lossy(&buf.get_all()).contains("Hello, Lua!"));
//! ```

use std::io::{self, Read, Write};

/// 环形内存缓冲区。
///
/// 写满后自动覆盖最早的内容（环形行为），保证内存占用有上界。
/// 同时实现 `Write`（Lua print 写入端）和 `Read`（前端读取端）。
pub struct MemBuffer {
    /// 环形存储
    buffer: Vec<u8>,
    /// 写入位置（下一个字节将写入此处）
    write_pos: usize,
    /// 读取位置（下一次 Read 从此处开始，get_all 也从这里开始）
    read_pos: usize,
    /// 当前有效数据长度（字节）
    len: usize,
    /// 缓冲区总容量
    capacity: usize,
}

impl MemBuffer {
    /// 创建指定容量的环形缓冲区。O(1) 分配。
    ///
    /// `capacity_bytes` 建议值：64KB（默认），最大 16MB。
    /// 输入为 0 时自动使用默认 64KB。
    pub fn new(capacity_bytes: usize) -> Self {
        let cap = if capacity_bytes == 0 {
            64 * 1024
        } else {
            capacity_bytes.min(16 * 1024 * 1024) // 硬上限 16MB
        };
        Self {
            buffer: vec![0u8; cap],
            write_pos: 0,
            read_pos: 0,
            len: 0,
            capacity: cap,
        }
    }

    /// 创建默认 64KB 容量的缓冲区。
    pub fn default() -> Self {
        Self::new(64 * 1024)
    }

    /// 获取当前缓冲区中的全部有效内容（从最旧到最新）。O(n)。
    ///
    /// 返回从最旧到最新的有序字节序列。
    pub fn get_all(&self) -> Vec<u8> {
        if self.len == 0 {
            return Vec::new();
        }

        let mut result = Vec::with_capacity(self.len);
        if self.read_pos + self.len <= self.capacity {
            // 单段连续 — 一次 memcpy
            result.extend_from_slice(&self.buffer[self.read_pos..self.read_pos + self.len]);
        } else {
            // 两段：尾部 + 头部
            let first_part = self.capacity - self.read_pos;
            result.extend_from_slice(&self.buffer[self.read_pos..]);
            result.extend_from_slice(&self.buffer[..self.len - first_part]);
        }
        result
    }

    /// 获取指定范围的内容（start..end，字节偏移，从最旧数据算起）。O(n)。
    ///
    /// 超出范围时截断。start >= data_len 时返回空。
    pub fn get_range(&self, start: usize, end: usize) -> Vec<u8> {
        if start >= self.len {
            return Vec::new();
        }
        let actual_end = end.min(self.len);
        let range_len = actual_end - start;

        let mut result = Vec::with_capacity(range_len);
        let abs_start = (self.read_pos + start) % self.capacity;
        let abs_end = (self.read_pos + actual_end) % self.capacity;

        if abs_start < abs_end {
            result.extend_from_slice(&self.buffer[abs_start..abs_end]);
        } else {
            let first_part = self.capacity - abs_start;
            result.extend_from_slice(&self.buffer[abs_start..]);
            result.extend_from_slice(&self.buffer[..abs_end]);
        }
        result
    }

    /// 增量读取：返回自上次 cursor 位置之后的新数据 + 新 cursor。O(n)。
    ///
    /// 用于前端增量拉取，避免每次全量传输。
    /// cursor 是逻辑偏移（0 表示最旧数据）。
    /// 返回 (新数据字节, 新 cursor)。
    pub fn read_since(&self, cursor: usize) -> (Vec<u8>, usize) {
        let cursor = cursor.min(self.len);
        if cursor >= self.len {
            return (Vec::new(), self.len);
        }
        let data = self.get_range(cursor, self.len);
        let new_cursor = self.len;
        (data, new_cursor)
    }

    /// 获取全部内容为字符串（UTF-8 安全，无效字节替换为 �）。O(n)。
    pub fn get_all_string(&self) -> String {
        String::from_utf8_lossy(&self.get_all()).into_owned()
    }

    /// 清空缓冲区。O(1)。
    pub fn clear(&mut self) {
        self.write_pos = 0;
        self.read_pos = 0;
        self.len = 0;
    }

    /// 当前有效数据大小（字节）。O(1)。
    pub fn data_len(&self) -> usize {
        self.len
    }

    /// 缓冲区总容量（字节）。O(1)。
    #[allow(dead_code)]
    pub fn total_capacity(&self) -> usize {
        self.capacity
    }
}

// =========================================================================
// std::io::Write trait 实现 — Lua print() 写入端
// =========================================================================

impl Write for MemBuffer {
    /// 写入数据到环形缓冲。O(n)，n = buf.len() 上限 capacity。
    ///
    /// 写满后自动覆盖最早的数据（read_pos 前移）。
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let write_len = buf.len().min(self.capacity);

        for &byte in &buf[..write_len] {
            self.buffer[self.write_pos] = byte;
            self.write_pos = (self.write_pos + 1) % self.capacity;

            if self.len < self.capacity {
                self.len += 1;
            } else {
                // 缓冲区已满：read_pos 向前推进（丢弃最旧数据）
                self.read_pos = (self.read_pos + 1) % self.capacity;
            }
        }

        Ok(write_len)
    }

    fn flush(&mut self) -> io::Result<()> {
        // 内存缓冲区无需 flush
        Ok(())
    }
}

// =========================================================================
// std::io::Read trait 实现 — 前端消费端
// =========================================================================

impl Read for MemBuffer {
    /// 从缓冲区读取数据（消费语义）。O(min(buf.len(), data_len))。
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.len == 0 {
            return Ok(0);
        }

        let to_read = buf.len().min(self.len);
        let mut bytes_read = 0;

        while bytes_read < to_read {
            buf[bytes_read] = self.buffer[self.read_pos];
            self.read_pos = (self.read_pos + 1) % self.capacity;
            self.len -= 1;
            bytes_read += 1;
        }

        Ok(bytes_read)
    }
}

// =========================================================================
// 测试
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn write_and_read_basic() {
        let mut buf = MemBuffer::new(1024);
        write!(buf, "hello").unwrap();
        assert_eq!(buf.data_len(), 5);
        assert_eq!(String::from_utf8(buf.get_all()).unwrap(), "hello");
    }

    #[test]
    fn ring_buffer_wraparound() {
        let mut buf = MemBuffer::new(8);
        write!(buf, "12345678").unwrap(); // 填满
        write!(buf, "90").unwrap();       // 覆盖 "12"
        let all = String::from_utf8(buf.get_all()).unwrap();
        assert_eq!(all, "34567890");
    }

    #[test]
    fn get_range_partial() {
        let mut buf = MemBuffer::new(1024);
        write!(buf, "ABCDEFGH").unwrap();
        let slice = String::from_utf8(buf.get_range(2, 5)).unwrap();
        assert_eq!(slice, "CDE");
    }

    #[test]
    fn read_since_incremental() {
        let mut buf = MemBuffer::new(1024);
        write!(buf, "hello").unwrap();
        let (data, cursor) = buf.read_since(0);
        assert_eq!(String::from_utf8(data).unwrap(), "hello");
        assert_eq!(cursor, 5);

        write!(buf, " world").unwrap();
        let (data2, cursor2) = buf.read_since(cursor);
        assert_eq!(String::from_utf8(data2).unwrap(), " world");
        assert_eq!(cursor2, 11);
    }

    #[test]
    fn zero_capacity_defaults_to_64k() {
        let buf = MemBuffer::new(0);
        assert_eq!(buf.total_capacity(), 64 * 1024);
    }

    #[test]
    fn max_capacity_capped() {
        let buf = MemBuffer::new(32 * 1024 * 1024);
        assert_eq!(buf.total_capacity(), 16 * 1024 * 1024);
    }

    #[test]
    fn clear_resets_state() {
        let mut buf = MemBuffer::new(1024);
        write!(buf, "test data").unwrap();
        buf.clear();
        assert_eq!(buf.data_len(), 0);
        assert!(buf.get_all().is_empty());
    }

    #[test]
    fn empty_get_range() {
        let buf = MemBuffer::new(1024);
        assert!(buf.get_range(0, 10).is_empty());
    }

    #[test]
    fn get_range_beyond_length() {
        let mut buf = MemBuffer::new(1024);
        write!(buf, "ABC").unwrap();
        let slice = String::from_utf8(buf.get_range(1, 100)).unwrap();
        assert_eq!(slice, "BC"); // 截断到实际长度
    }

    #[test]
    fn read_consumer() {
        let mut buf = MemBuffer::new(1024);
        write!(buf, "hello").unwrap();
        let mut out = [0u8; 3];
        let n = buf.read(&mut out).unwrap();
        assert_eq!(n, 3);
        assert_eq!(&out[..3], b"hel");
        assert_eq!(buf.data_len(), 2);
    }
}
