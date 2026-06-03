//! state.rs — 窗口增强系统的类型定义
//!
//! ## 职责
//! - [`WindowKind`] 枚举：区分主窗口 / 分离窗口，驱动 hook 的策略分发
//! - [`WindowState`]：单个窗口的运行时状态（由 [`WindowManager`](super::manager::WindowManager) 持有）
//! - [`HashMapExt`] trait：为 HashMap 扩展声明式 `insert_with` 方法，
//!   在插入时通过无状态 hook 闭包 match [`WindowKind`] 自动分发到正确的初始化策略

use std::collections::HashMap;
use std::hash::Hash;

// ═══════════════════════════════════════════════════════════════════
// WindowKind — 窗口类型枚举（驱动策略分发）
// ═══════════════════════════════════════════════════════════════════

/// 窗口类型。
///
/// 在 `insert_with` 的 hook 闭包中被 match，自动分发到不同的窗口过程：
/// - [`WindowKind::Main`] → 安装 [`main_window_proc`](super::subclass::main_window_proc)
/// - [`WindowKind::Detached`] → 安装 [`detached_window_proc`](super::subclass::detached_window_proc)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowKind {
    /// 主窗口 — 应用启动时创建的唯一主窗口，作为合并目标
    Main,
    /// 分离窗口 — 从主窗口拖拽分离出的独立窗口，可触发拖拽合并
    Detached,
}

// ═══════════════════════════════════════════════════════════════════
// WindowState — 单个窗口的运行时状态
// ═══════════════════════════════════════════════════════════════════

/// 单个被子类化窗口的完整状态。
///
/// 由 [`WindowManager`](super::manager::WindowManager) 内部持有，
/// 以 `hwnd_raw` (isize) 为键存储在 `HashMap` 中。
#[derive(Clone)]
pub struct WindowState {
    /// 原始窗口过程地址（用于 `CallWindowProcW` 转发未处理消息）
    pub original_proc: isize,
    /// 自定义标题栏区域列表：`(x, y, width, height, kind)`
    ///
    /// 坐标均为物理像素，相对于窗口客户区左上角。
    /// `kind` 为 `"maxbutton"` / `"minbutton"` / `"closebutton"` / 其他（视为拖拽区）。
    pub regions: Vec<(i32, i32, i32, i32, String)>,
    /// 窗口类型
    pub kind: WindowKind,
}

// ═══════════════════════════════════════════════════════════════════
// HashMapExt — 声明式插入 + 策略分发
// ═══════════════════════════════════════════════════════════════════

/// 为 [`HashMap`] 扩展 `insert_with` 方法。
///
/// ## 设计意图
///
/// `insert_with` 提供声明式的「插入即初始化」语义：
/// 调用者只需提供 key（HWND）、val（窗口状态，含 [`WindowKind`]）、
/// 以及一个**无状态 hook 闭包**。
///
/// Hook 在插入**之前**被调用，接收对 key 和 val 的不可变引用。
/// 在 hook 内部通过 `match val.kind` 将不同窗口类型分发到对应的窗口过程安装逻辑。
///
/// 这种设计使调用者无需关心窗口类型的具体初始化细节——
/// 它们由 hook + 类型系统自动处理。
///
/// ## 无状态性
///
/// Hook 闭包自身不应捕获可变状态。它应仅通过 `(&K, &V)` 参数获取上下文，
/// 并委托给无副作用的纯函数。
pub trait HashMapExt<K, V> {
    /// 插入键值对，并在**插入前**调用 `hook` 闭包执行初始化。
    ///
    /// `hook` 接收对 key 和 val 的不可变引用，用于执行窗口过程安装等初始化副作用。
    ///
    /// 若 `key` 已存在，旧值将被替换，hook 仍会对新值执行。
    fn insert_with<F>(&mut self, key: K, val: V, hook: F) -> Option<V>
    where
        F: FnOnce(&K, &V);
}

impl<K: Eq + Hash, V> HashMapExt<K, V> for HashMap<K, V> {
    fn insert_with<F>(&mut self, key: K, val: V, hook: F) -> Option<V>
    where
        F: FnOnce(&K, &V),
    {
        // 先执行 hook 做初始化（如安装窗口过程），再插入 Map。
        // 保证即使 hook 内部触发消息回调，Map 中不会找到未完全初始化的条目。
        hook(&key, &val);
        self.insert(key, val)
    }
}

// ═══════════════════════════════════════════════════════════════════
// 测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_with_calls_hook_before_insert() {
        let mut map: HashMap<i32, String> = HashMap::new();

        map.insert_with(1, "hello".to_string(), |_key, _val| {
            // 无状态 hook：仅通过参数获取上下文，不捕获可变状态
        });

        assert_eq!(map.get(&1).map(|s| s.as_str()), Some("hello"));
    }

    #[test]
    fn insert_with_replaces_and_calls_hook() {
        let mut map: HashMap<i32, String> = HashMap::new();
        map.insert(1, "old".to_string());

        let mut hook_called = false;
        let hook_called_ref = &mut hook_called;
        map.insert_with(1, "new".to_string(), move |key, val| {
            assert_eq!(*key, 1);
            assert_eq!(val, "new");
            *hook_called_ref = true;
        });

        assert!(hook_called);
        assert_eq!(map.get(&1).map(|s| s.as_str()), Some("new"));
    }
}
