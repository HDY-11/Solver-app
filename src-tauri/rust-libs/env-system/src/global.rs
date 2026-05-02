use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use crate::dir::Dir;

static DIRS: OnceLock<RwLock<HashMap<String, Dir>>> = OnceLock::new();

/// 初始化全局目录表。
///
/// 传入一个闭包，该闭包接收一个 `&mut Env` 对象，用于构建目录映射。
/// 应在程序入口处调用，且只能调用一次；重复调用将 panic。
pub fn init_dirs(builder: impl FnOnce(&mut Env)) {
    let mut env = Env::new();
    builder(&mut env);

    let map = env
        .directories
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();

    DIRS
        .set(RwLock::new(map))
        .expect("DIRS has already been initialized");
}

/// 获取全局目录表的只读锁。
pub fn dirs() -> Option<&'static RwLock<HashMap<String, Dir>>> {
    DIRS.get()
}

/// 环境构建器，在初始化闭包中用于设置目录映射。
pub struct Env {
    directories: HashMap<&'static str, Dir>,
}

impl Env {
    fn new() -> Self {
        Self {
            directories: HashMap::new(),
        }
    }

    /// 插入一个目录别名。
    pub fn add_dir(&mut self, key: &'static str, dir: Dir) {
        self.directories.insert(key, dir);
    }

    /// 根据别名获取目录（内部使用）。
    pub fn get_dir(&self, key: &str) -> Option<&Dir> {
        self.directories.get(key)
    }
}

/// 便捷函数：根据别名获取目录，返回克隆后的 `Dir`。
pub fn get_dir(key: &str) -> Option<Dir> {
    dirs()?
        .read()
        .expect("DIRS lock poisoned")
        .get(key)
        .cloned()
}