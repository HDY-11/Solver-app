use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use chrono::Local;
use utils::{Lent, Token};
use std::io::Seek;
/// 轮换日志文件管理器，用两个 `Token` 分别管理缓冲写入器和当前路径。
///
/// 通过 [`split`] 可安全地同时借出 `writer` 和 `path` 的独立守卫，
/// 编译期阻止任何并发访问。`save_as` 和 `clear` 等需要所有权转移的操作
/// 则通过 [`Token::replace`] 在同一 `&mut self` 中完成。
pub struct RotatingLogFile {
    writer: Token<BufWriter<File>>,
    current_path: Token<PathBuf>,
    /// 日志文件目录（固定不变，直接共享引用即可）
    dir: PathBuf,
    max_keep: usize,
    buf_capacity: usize,
}

/// 由 `RotatingLogFile::split()` 返回的组合守卫，包含独立可用的写入器和路径。
pub struct RotatingFileGuard<'a> {
    pub writer: Lent<'a, BufWriter<File>>,
    pub path: Lent<'a, PathBuf>,
}

impl RotatingLogFile {
    /// 创建新的轮换日志文件。
    ///
    /// 自动在 `dir` 中创建时间戳文件，并清理其他自动命名文件（保留最近 `max_keep` 个）。
    pub fn new(dir: impl Into<PathBuf>, max_keep: usize, buf_capacity: usize) -> io::Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;

        // 轮换旧的时间戳文件
        rotate_auto_files(&dir, max_keep)?;

        let filename = format!("{}.log", Local::now().format("%Y-%m-%d %H_%M_%S"));
        let path = dir.join(&filename);
        let file = File::create(&path)?;
        let writer = BufWriter::with_capacity(buf_capacity, file);

        Ok(Self {
            writer: Token::new(writer),
            current_path: Token::new(path),
            dir,
            max_keep,
            buf_capacity,
        })
    }

    /// 将写入器和路径同时借出，返回一个组合守卫。
    ///
    /// 编译器保证在守卫存活期间无法再次调用 `split` 或其他 `&mut self` 方法。
    pub fn split(&mut self) -> RotatingFileGuard<'_> {
        RotatingFileGuard {
            writer: self.writer.lend(),
            path: self.current_path.lend(),
        }
    }

    pub fn lend_writer(&mut self) -> Lent<'_, BufWriter<File>>{
        self.writer.lend()
    }
}

// ─── 分离后的组合操作（实现在守卫上） ───────────────
impl RotatingFileGuard<'_> {
    /// 写入一行，自动追加换行。
    pub fn writeln(&mut self, line: &str) -> io::Result<()> {
        writeln!(self.writer, "{}", line)
    }

    /// 刷新缓冲区（不强制 sync）。
    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    /// 强制同步到磁盘。
    pub fn sync_all(&mut self) -> io::Result<()> {
        self.writer.flush()?;
        self.writer.get_ref().sync_all()
    }
}

// ─── 需要所有权替换的方法（直接实现在 `RotatingLogFile` 上） ───
impl RotatingLogFile {
    /// 清空当前文件（截断，重建 BufWriter）。
    pub fn clear(&mut self) -> io::Result<()> {
        // 1. 取出旧 writer，关闭文件（所有权 drop）
        let old_writer = self.writer.replace(BufWriter::with_capacity(
            self.buf_capacity,
            tempfile::tempfile()?, // 临时占位，立即会被替换
        ));
        // 2. 提取出 File 并截断
        let mut file = old_writer.into_inner()?;
        file.set_len(0)?;
        file.seek(io::SeekFrom::Start(0))?;
        // 3. 重新包装并放回 Token
        let new_writer = BufWriter::with_capacity(self.buf_capacity, file);
        // 替换掉临时占位的 writer
        self.writer.replace(new_writer);
        Ok(())
    }

    /// 将当前活动文件另存为指定名称（不参与轮换），并创建新的活动文件。
    pub fn save_as(&mut self, new_name: impl AsRef<Path>) -> io::Result<()> {
        // 1. 先确保数据落盘
        {
            let mut guard = self.split();
            guard.flush()?;
            guard.sync_all()?;
        } // Lent 归还

        // 2. 取出旧的 writer 和 path（所有权）
        let old_writer = self.writer.replace(
            // 临时占位，稍后用真正的 writer 替换
            BufWriter::with_capacity(0, tempfile::tempfile()?),
        );
        let old_path = self.current_path.replace(PathBuf::new());

        // 3. 关闭旧文件（drop old_writer）
        drop(old_writer);

        // 4. 重命名
        let new_path = self.dir.join(new_name.as_ref());
        fs::rename(&old_path, &new_path)?;

        // 5. 创建新文件并放回 Token
        let filename = format!("{}.log", Local::now().format("%Y-%m-%d_%H_%M_%S"));
        let fresh_path = self.dir.join(&filename);
        let file = File::create(&fresh_path)?;
        let new_writer = BufWriter::with_capacity(self.buf_capacity, file);

        self.writer.replace(new_writer);          // 替换掉临时占位
        self.current_path.replace(fresh_path);    // 替换掉空 PathBuf

        Ok(())
    }
}

// ─── 辅助函数 ─────────────────────────────
fn rotate_auto_files(dir: &Path, max_keep: usize) -> io::Result<()> {
    let mut auto_files: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|s| is_timestamp_filename(s))
                .unwrap_or(false)
        })
        .map(|e| e.path())
        .collect();

    auto_files.sort();
    while auto_files.len() > max_keep {
        if let Some(oldest) = auto_files.first() {
            fs::remove_file(oldest)?;
            auto_files.remove(0);
        }
    }
    Ok(())
}

fn is_timestamp_filename(name: &str) -> bool {
    let name = name.strip_suffix(".log").unwrap_or(name);
    chrono::NaiveDateTime::parse_from_str(name, "%Y-%m-%d %H_%M_%S").is_ok()
}