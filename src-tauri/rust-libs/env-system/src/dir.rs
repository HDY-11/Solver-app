use camino::{Utf8Path, Utf8PathBuf};
use std::fmt;
use std::ops::Add;
use std::path::Path;
use std::io::{Error, ErrorKind};
use std::fs::File;

/// 表示一个**目录**路径（非文件）。
///
/// 内部保证路径以分隔符结尾，或是一个干净的目录形式。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Dir {
    path: Utf8PathBuf,
}

impl Dir {
    /// 从任意 UTF-8 路径创建一个 `Dir`。
    ///
    /// 如果路径不以分隔符结尾，会自动添加。
    pub fn new(path: impl Into<Utf8PathBuf>) -> Self {
        let mut p = path.into();
        if !p.as_str().ends_with(std::path::MAIN_SEPARATOR) {
            p.push("");
        }
        Self { path: p }
    }

    /// 返回内部 `Utf8Path`。
    pub fn as_path(&self) -> &Utf8Path {
        &self.path
    }

    /// 返回内部 `Path`（用于标准库文件操作）。
    pub fn as_std_path(&self) -> &Path {
        self.path.as_std_path()
    }

    /// 获取父目录。
    /// 如果已经到达文件系统根目录，返回 `None`。
    pub fn parent(&self) -> Option<Self> {
        self.path.parent().map(Self::new)
    }

    /// 向上跳转指定层级。
    ///
    /// 例如 `dir.up(2)` 返回祖目录。如果层数超出，返回`None`。
    pub fn up(&self, levels: u32) -> Option<Self> {
        let mut p = self.path.clone();
        for _ in 0..levels {
            p = p.parent()?.to_path_buf();
        }
        Some(Self::new(p))
    }

    /// 拼接子目录（纯路径操作，不检查文件系统）。
    ///
    /// 返回新的 `Dir`，路径以分隔符结尾。
    pub fn join(&self, segment: impl AsRef<Utf8Path>) -> Self {
        let new_path = self.path.join(segment.as_ref());
        Self::new(new_path)
    }

    /// 拼接子目录，并在**路径已存在**时才返回 `Ok`。
    ///
    /// 如果路径不存在，返回 `ErrorKind::NotFound` 错误。
    pub fn join_existing(&self, segment: impl AsRef<Utf8Path>) -> std::io::Result<Self> {
        let new = self.join(segment);
        if new.as_std_path().exists() {
            Ok(new)
        } else {
            Err(Error::new(ErrorKind::NotFound, "路径不存在"))
        }
    }

    /// 直接拼接路径（不保证结果仍是目录，用于创建文件等）。
    pub fn join_raw(&self, segment: impl AsRef<Utf8Path>) -> Utf8PathBuf {
        self.path.join(segment.as_ref())
    }

    /// 检查目录是否存在于文件系统
    pub fn exists(&self) -> bool {
        self.as_std_path().exists()
    }

    /// 检查路径是否存在且为目录
    pub fn is_dir(&self) -> bool {
        self.as_std_path().is_dir()
    }

    /// 调用 `std::fs::create_dir_all` 创建自身及其父目录。
    pub fn create_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.as_std_path())
    }

    /// 在此目录下加载一个文件，如果不存在就创建，并返回对应的 `File`。
    pub fn create_file(&self, file_name: impl AsRef<str>) -> std::io::Result<File> {
        let full_path = self.join_raw(file_name.as_ref());
        let file = std::fs::File::create(full_path.as_std_path())?;
        Ok(file)
    }

    /// 打开（只读）目录下的一个文件。
    pub fn open_file(&self, file_name: impl AsRef<str>) -> std::io::Result<std::fs::File> {
        let full_path = self.join_raw(file_name.as_ref());
        std::fs::File::open(full_path.as_std_path())
    }


}

// 运算符重载：Dir + &str => Dir（纯拼接，不检查存在）
impl Add<&str> for Dir {
    type Output = Self;

    fn add(self, rhs: &str) -> Self::Output {
        self.join(rhs)
    }
}

impl Add<&String> for Dir {
    type Output = Self;
    fn add(self, rhs: &String) -> Self::Output {
        self.join(rhs.as_str())
    }
}

impl Add<String> for Dir {
    type Output = Self;
    fn add(self, rhs: String) -> Self::Output {
        self.join(rhs)
    }
}

impl fmt::Display for Dir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path)
    }
}

impl From<&str> for Dir {
    fn from(s: &str) -> Self {
        Dir::new(s)
    }
}

impl From<String> for Dir {
    fn from(s: String) -> Self {
        Dir::new(s)
    }
}

impl From<Utf8PathBuf> for Dir {
    fn from(p: Utf8PathBuf) -> Self {
        Dir::new(p)
    }
}