//! behaviors.rs — 可组合的行为声明
//!
//! ## 架构角色
//!
//! [`HookBehaviors`] 是业务层声明"我对哪些窗口消息感兴趣"的机制。
//! 替代旧的 `WindowKind` 枚举，实现：
//! - **可组合**：`NCHITTEST | DRAG_END` 自由按位组合
//! - **可扩展**：新增行为只需添加一个 flag，核心代码无改动
//! - **自文档化**：每个 flag 精确对应一种 Windows 消息

use bitflags::bitflags;

bitflags! {
    /// 窗口消息兴趣声明集。
    ///
    /// 业务层通过位组合声明要拦截的 Windows 消息类型，
    /// 配合 [`WindowBehavior`] trait 注入对应的处理逻辑。
    ///
    /// ## 标志与消息映射
    ///
    /// | 标志         | 对应消息              | 含义                        |
    /// |--------------|-----------------------|-----------------------------|
    /// | `NCHITTEST`  | `WM_NCHITTEST`        | 自定义标题栏区域命中测试    |
    /// | `DRAG_START` | `WM_ENTERSIZEMOVE`    | 窗口开始拖拽/调整大小       |
    /// | `DRAG_END`   | `WM_EXITSIZEMOVE`     | 窗口结束拖拽/调整大小       |
    ///
    /// ## 使用示例
    ///
    /// ```ignore
    /// // 仅需命中测试（如主窗口）
    /// let flags = HookBehaviors::NCHITTEST;
    ///
    /// // 命中测试 + 拖拽检测（如分离窗口）
    /// let flags = HookBehaviors::NCHITTEST | HookBehaviors::DRAG_START | HookBehaviors::DRAG_END;
    /// ```
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct HookBehaviors: u32 {
        /// 拦截 `WM_NCHITTEST` — 自定义标题栏按钮命中测试
        const NCHITTEST  = 1 << 0;
        /// 拦截 `WM_ENTERSIZEMOVE` — 窗口移动/调整大小开始
        const DRAG_START = 1 << 1;
        /// 拦截 `WM_EXITSIZEMOVE` — 窗口移动/调整大小结束
        const DRAG_END   = 1 << 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_contains_nothing() {
        let b = HookBehaviors::empty();
        assert!(!b.contains(HookBehaviors::NCHITTEST));
        assert!(!b.contains(HookBehaviors::DRAG_START));
        assert!(!b.contains(HookBehaviors::DRAG_END));
    }

    #[test]
    fn composition_via_or() {
        let b = HookBehaviors::NCHITTEST | HookBehaviors::DRAG_END;
        assert!(b.contains(HookBehaviors::NCHITTEST));
        assert!(!b.contains(HookBehaviors::DRAG_START));
        assert!(b.contains(HookBehaviors::DRAG_END));
    }

    #[test]
    fn all_flag() {
        let b = HookBehaviors::all();
        assert!(b.contains(HookBehaviors::NCHITTEST));
        assert!(b.contains(HookBehaviors::DRAG_START));
        assert!(b.contains(HookBehaviors::DRAG_END));
    }
}
