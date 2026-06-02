use tauri::AppHandle;
use error_system::{ResultLogExt, AppError};
use once_cell::sync::OnceCell;

pub static GLOBAL_APPHANDLE: OnceCell<AppHandle> = OnceCell::new();

// ── 事件注册表（预留，后续按需扩展） ────────────────
pub mod event_registry {
    /// 所有有效事件名（编译期检查用）
    pub const VALID_EVENTS: &[&str] = &[
        "app-ready",
        "drag-release",
        "script-result",
        "run-output",
        "run-complete",
        "merge-request",
    ];

    /// 检查事件名是否在注册表中
    pub const fn is_valid(event: &str) -> bool {
        let mut i = 0;
        while i < VALID_EVENTS.len() {
            if const_str_equal(event, VALID_EVENTS[i]) {
                return true;
            }
            i += 1;
        }
        false
    }

    /// 编译期字符串相等比较（手动逐字节）
    pub(crate) const fn const_str_equal(a: &str, b: &str) -> bool {
        let a_bytes = a.as_bytes();
        let b_bytes = b.as_bytes();

        if a_bytes.len() != b_bytes.len() {
            return false;
        }

        let mut i = 0;
        while i < a_bytes.len() {
            if a_bytes[i] != b_bytes[i] {
                return false;
            }
            i += 1;
        }
        true
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn empty_registry_rejects_all() {
            assert!(!is_valid("any-event"));
        }
    }
}

// ── 目标注册表（emit_to! 编译期检查用） ────────────────
pub mod target_registry {
    /// 所有有效的目标 label（编译期检查用）
    /// 注意：此处列出的是静态已知的窗口/webview label，
    /// 动态创建的分离窗口 label 应通过 `dyn` 语法绕过检查。
    pub const VALID_TARGETS: &[&str] = &[
        "main",
    ];

    /// 检查目标 label 是否在注册表中
    pub const fn is_valid(target: &str) -> bool {
        let mut i = 0;
        while i < VALID_TARGETS.len() {
            if super::event_registry::const_str_equal(target, VALID_TARGETS[i]) {
                return true;
            }
            i += 1;
        }
        false
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn known_target_passes() {
            assert!(is_valid("main"));
        }

        #[test]
        fn unknown_target_fails() {
            assert!(!is_valid("nonexistent"));
        }
    }
}

/// 初始化事件系统（在 `setup` 中调用）
pub fn init_event_system(handle: AppHandle) -> Result<(), AppError> {
    GLOBAL_APPHANDLE
        .set(handle)
        .map_err(|_| AppError::EventNotInitialized)
        .inspect_log("Failed to initialize event system")?;
    Ok(())
}

// ── 宏实现 ──────────────────────────────────────────────────
/// 发射事件
///
/// ## 语法
/// - 静态：`emit!("event-name": payload)`
/// - 动态：`emit!(dyn "event-name": payload)` 或 `emit!(dyn expr: payload)`
/// - 批量：`emit!("a": pa, dyn "b": pb)`
#[macro_export]
macro_rules! emit {
    // 批量入口 - 拆分为静态和动态的分支
    // 动态字面量：dyn "event"
    ($(dyn $event:literal : $payload:expr),+ $(,)?) => {
        $(
            emit!(@single dyn $event : $payload);
        )+
    };
    // 动态表达式：dyn (expr)
    ($(dyn ($event:expr) : $payload:expr),+ $(,)?) => {
        $(
            emit!(@single dyn ($event) : $payload);
        )+
    };
    // 静态字面量（必须放最后，否则会抢占前面的匹配）
    ($($event:literal : $payload:expr),+ $(,)?) => {
        $(
            emit!(@single $event : $payload);
        )+
    };

    // ── 内部单事件分发 ────────────────────────
    // 静态：带注册表检查
    (@single $event:literal : $payload:expr) => {{
        const _: () = {
            if !$crate::event_registry::is_valid($event) {
                panic!("Unknown event type in emit!(). Add it to the event registry.");
            }
        };
        match $crate::GLOBAL_APPHANDLE.get() {
            Some(handle) => {
                let _ = tauri::Emitter::emit(handle, $event, $payload);
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[emit] not initialized: {}", $event);
            }
        }
    }};
    // 动态字面量：无编译期检查
    (@single dyn $event:literal : $payload:expr) => {{
        match $crate::GLOBAL_APPHANDLE.get() {
            Some(handle) => {
                let _ = tauri::Emitter::emit(handle, $event, $payload);
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[emit] not initialized: {}", $event);
            }
        }
    }};
    // 动态表达式：无编译期检查
    (@single dyn ($event:expr) : $payload:expr) => {{
        match $crate::GLOBAL_APPHANDLE.get() {
            Some(handle) => {
                let _ = tauri::Emitter::emit(handle, $event, $payload);
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[emit] not initialized: dynamic event");
            }
        }
    }};
}

/// 向指定目标发射事件
///
/// ## 语法
/// - 静态目标 + 静态事件：`emit_to!("main" => "event-name": payload)`
/// - 动态目标（字面量）：`emit_to!(dyn "main" => "event-name": payload)`
/// - 动态目标（表达式）：`emit_to!(dyn (target_expr) => "event-name": payload)`
/// - 动态事件：`emit_to!("main" => dyn "event-name": payload)`
/// - 批量：`emit_to!("main" => "a": pa, "b": pb)`
/// - 混合批量：`emit_to!("main" => "a": pa, dyn "b": pb)`
///
/// ## target 注册表
/// 静态目标字面量（不含 `dyn` 前缀）必须在 `target_registry::VALID_TARGETS`
/// 中注册，否则编译失败。动态创建的分离窗口请使用 `dyn` 语法绕过检查。
#[macro_export]
macro_rules! emit_to {
    // ═══════════════════════════════════════════════════════
    // 批量入口（事件类型必须一致）
    // ═══════════════════════════════════════════════════════

    // ── 静态 target，批量静态事件 ─────────────────────
    ($target:literal => $($event:literal : $payload:expr),+ $(,)?) => {
        $(
            emit_to!(@single $target : $event : $payload);
        )+
    };
    // ── 静态 target，批量动态字面量事件 ───────────────
    ($target:literal => $(dyn $event:literal : $payload:expr),+ $(,)?) => {
        $(
            emit_to!(@single $target : dyn $event : $payload);
        )+
    };
    // ── 静态 target，批量动态表达式事件 ───────────────
    ($target:literal => $(dyn ($event:expr) : $payload:expr),+ $(,)?) => {
        $(
            emit_to!(@single $target : dyn ($event) : $payload);
        )+
    };

    // ── 动态字面量 target，批量静态事件 ───────────────
    (dyn $target:literal => $($event:literal : $payload:expr),+ $(,)?) => {
        $(
            emit_to!(@single dyn $target : $event : $payload);
        )+
    };
    // ── 动态字面量 target，批量动态字面量事件 ─────────
    (dyn $target:literal => $(dyn $event:literal : $payload:expr),+ $(,)?) => {
        $(
            emit_to!(@single dyn $target : dyn $event : $payload);
        )+
    };
    // ── 动态字面量 target，批量动态表达式事件 ─────────
    (dyn $target:literal => $(dyn ($event:expr) : $payload:expr),+ $(,)?) => {
        $(
            emit_to!(@single dyn $target : dyn ($event) : $payload);
        )+
    };

    // ── 动态表达式 target，批量静态事件 ───────────────
    (dyn ($target:expr) => $($event:literal : $payload:expr),+ $(,)?) => {
        $(
            emit_to!(@single dyn ($target) : $event : $payload);
        )+
    };
    // ── 动态表达式 target，批量动态字面量事件 ─────────
    (dyn ($target:expr) => $(dyn $event:literal : $payload:expr),+ $(,)?) => {
        $(
            emit_to!(@single dyn ($target) : dyn $event : $payload);
        )+
    };
    // ── 动态表达式 target，批量动态表达式事件 ─────────
    (dyn ($target:expr) => $(dyn ($event:expr) : $payload:expr),+ $(,)?) => {
        $(
            emit_to!(@single dyn ($target) : dyn ($event) : $payload);
        )+
    };

    // ═══════════════════════════════════════════════════════
    // 单事件入口（防止与批量冲突：必须放批量之后）
    // ═══════════════════════════════════════════════════════

    // ── 静态 target，静态 event ──────────────────────
    ($target:literal => $event:literal : $payload:expr) => {
        emit_to!(@single $target : $event : $payload);
    };
    // ── 静态 target，动态字面量 event ────────────────
    ($target:literal => dyn $event:literal : $payload:expr) => {
        emit_to!(@single $target : dyn $event : $payload);
    };
    // ── 静态 target，动态表达式 event ────────────────
    ($target:literal => dyn ($event:expr) : $payload:expr) => {
        emit_to!(@single $target : dyn ($event) : $payload);
    };

    // ── 动态字面量 target，静态 event ────────────────
    (dyn $target:literal => $event:literal : $payload:expr) => {
        emit_to!(@single dyn $target : $event : $payload);
    };
    // ── 动态字面量 target，动态字面量 event ──────────
    (dyn $target:literal => dyn $event:literal : $payload:expr) => {
        emit_to!(@single dyn $target : dyn $event : $payload);
    };
    // ── 动态字面量 target，动态表达式 event ──────────
    (dyn $target:literal => dyn ($event:expr) : $payload:expr) => {
        emit_to!(@single dyn $target : dyn ($event) : $payload);
    };

    // ── 动态表达式 target，静态 event ────────────────
    (dyn ($target:expr) => $event:literal : $payload:expr) => {
        emit_to!(@single dyn ($target) : $event : $payload);
    };
    // ── 动态表达式 target，动态字面量 event ──────────
    (dyn ($target:expr) => dyn $event:literal : $payload:expr) => {
        emit_to!(@single dyn ($target) : dyn $event : $payload);
    };
    // ── 动态表达式 target，动态表达式 event ──────────
    (dyn ($target:expr) => dyn ($event:expr) : $payload:expr) => {
        emit_to!(@single dyn ($target) : dyn ($event) : $payload);
    };

    // ═══════════════════════════════════════════════════════
    // 内部单事件分发
    // ═══════════════════════════════════════════════════════

    // ── 静态 target + 静态 event：双编译期检查 ───────
    (@single $target:literal : $event:literal : $payload:expr) => {{
        const _: () = {
            if !$crate::target_registry::is_valid($target) {
                panic!("Unknown target label in emit_to!(). Add it to the target registry.");
            }
            if !$crate::event_registry::is_valid($event) {
                panic!("Unknown event type in emit_to!(). Add it to the event registry.");
            }
        };
        match $crate::GLOBAL_APPHANDLE.get() {
            Some(handle) => {
                let _ = tauri::Emitter::emit_to(handle, $target, $event, $payload);
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[emit_to] not initialized: {} -> {}", $target, $event);
            }
        }
    }};

    // ── 静态 target + 动态字面量 event ───────────────
    (@single $target:literal : dyn $event:literal : $payload:expr) => {{
        const _: () = {
            if !$crate::target_registry::is_valid($target) {
                panic!("Unknown target label in emit_to!(). Add it to the target registry.");
            }
        };
        match $crate::GLOBAL_APPHANDLE.get() {
            Some(handle) => {
                let _ = tauri::Emitter::emit_to(handle, $target, $event, $payload);
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[emit_to] not initialized: {} -> {}", $target, $event);
            }
        }
    }};

    // ── 静态 target + 动态表达式 event ───────────────
    (@single $target:literal : dyn ($event:expr) : $payload:expr) => {{
        const _: () = {
            if !$crate::target_registry::is_valid($target) {
                panic!("Unknown target label in emit_to!(). Add it to the target registry.");
            }
        };
        match $crate::GLOBAL_APPHANDLE.get() {
            Some(handle) => {
                let _ = tauri::Emitter::emit_to(handle, $target, $event, $payload);
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[emit_to] not initialized: {} -> dynamic event", $target);
            }
        }
    }};

    // ── 动态字面量 target + 静态 event ───────────────
    (@single dyn $target:literal : $event:literal : $payload:expr) => {{
        const _: () = {
            if !$crate::event_registry::is_valid($event) {
                panic!("Unknown event type in emit_to!(). Add it to the event registry.");
            }
        };
        match $crate::GLOBAL_APPHANDLE.get() {
            Some(handle) => {
                let _ = tauri::Emitter::emit_to(handle, $target, $event, $payload);
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[emit_to] not initialized: {} -> {}", $target, $event);
            }
        }
    }};

    // ── 动态字面量 target + 动态字面量 event ─────────
    (@single dyn $target:literal : dyn $event:literal : $payload:expr) => {{
        match $crate::GLOBAL_APPHANDLE.get() {
            Some(handle) => {
                let _ = tauri::Emitter::emit_to(handle, $target, $event, $payload);
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[emit_to] not initialized: {} -> {}", $target, $event);
            }
        }
    }};

    // ── 动态字面量 target + 动态表达式 event ─────────
    (@single dyn $target:literal : dyn ($event:expr) : $payload:expr) => {{
        match $crate::GLOBAL_APPHANDLE.get() {
            Some(handle) => {
                let _ = tauri::Emitter::emit_to(handle, $target, $event, $payload);
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[emit_to] not initialized: {} -> dynamic event", $target);
            }
        }
    }};

    // ── 动态表达式 target + 静态 event ───────────────
    (@single dyn ($target:expr) : $event:literal : $payload:expr) => {{
        const _: () = {
            if !$crate::event_registry::is_valid($event) {
                panic!("Unknown event type in emit_to!(). Add it to the event registry.");
            }
        };
        match $crate::GLOBAL_APPHANDLE.get() {
            Some(handle) => {
                let _ = tauri::Emitter::emit_to(handle, $target, $event, $payload);
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[emit_to] not initialized: dynamic target -> {}", $event);
            }
        }
    }};

    // ── 动态表达式 target + 动态字面量 event ─────────
    (@single dyn ($target:expr) : dyn $event:literal : $payload:expr) => {{
        match $crate::GLOBAL_APPHANDLE.get() {
            Some(handle) => {
                let _ = tauri::Emitter::emit_to(handle, $target, $event, $payload);
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[emit_to] not initialized: dynamic target -> {}", $event);
            }
        }
    }};

    // ── 动态表达式 target + 动态表达式 event ─────────
    (@single dyn ($target:expr) : dyn ($event:expr) : $payload:expr) => {{
        match $crate::GLOBAL_APPHANDLE.get() {
            Some(handle) => {
                let _ = tauri::Emitter::emit_to(handle, $target, $event, $payload);
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[emit_to] not initialized: dynamic target -> dynamic event");
            }
        }
    }};
}

#[macro_export]
macro_rules! listen {
    // ── 静态监听：编译期检查 ─────────────────────────────
    ($event:literal : $handler:expr) => {
        listen!(@single $event : $handler);
    };
    // ── 动态监听：字面量 + dyn ──────────────────────────
    (dyn $event:literal : $handler:expr) => {
        listen!(@single dyn $event : $handler);
    };
    // ── 动态监听：表达式 + dyn（括号包裹） ───────────────
    (dyn ($event:expr) : $handler:expr) => {
        listen!(@single dyn ($event) : $handler);
    };

    // ── 内部实现 ────────────────────────────────────────
    // 静态：带注册表检查
    (@single $event:literal : $handler:expr) => {{
        const _: () = {
            if !$crate::event_registry::is_valid($event) {
                panic!("Unknown event type in listen!(). Add it to the event registry.");
            }
        };
        match $crate::GLOBAL_APPHANDLE.get() {
            Some(handle) => {
                Some(tauri::Listener::listen(handle, $event, $handler))
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[listen] Event system not initialized, cannot register listener for: {}", $event);
                None
            }
        }
    }};
    // 动态字面量：无编译期检查
    (@single dyn $event:literal : $handler:expr) => {{
        match $crate::GLOBAL_APPHANDLE.get() {
            Some(handle) => {
                Some(tauri::Listener::listen(handle, $event, $handler))
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[listen] Event system not initialized, cannot register listener for: {}", $event);
                None
            }
        }
    }};
    // 动态表达式：无编译期检查
    (@single dyn ($event:expr) : $handler:expr) => {{
        match $crate::GLOBAL_APPHANDLE.get() {
            Some(handle) => {
                Some(tauri::Listener::listen(handle, &$event, $handler))
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[listen] Event system not initialized, cannot register dynamic listener");
                None
            }
        }
    }};
}

#[macro_export]
macro_rules! async_listen {
    // ── 静态异步监听：编译期检查 ─────────────────────────
    ($event:literal : $handler:expr) => {
        async_listen!(@single $event : $handler);
    };
    // ── 动态异步监听：字面量 + dyn ──────────────────────
    (dyn $event:literal : $handler:expr) => {
        async_listen!(@single dyn $event : $handler);
    };
    // ── 动态异步监听：表达式 + dyn ─────────────────────
    (dyn ($event:expr) : $handler:expr) => {
        async_listen!(@single dyn ($event) : $handler);
    };

    // ── 内部实现 ────────────────────────────────────────
    // 静态
    (@single $event:literal : $handler:expr) => {
        const _: () = {
            if !$crate::event_registry::is_valid($event) {
                panic!("Unknown event type in async_listen!(). Add it to the event registry.");
            }
        };
        match $crate::GLOBAL_APPHANDLE.get() {
            Some(handle) => {
                let handle_clone = handle.clone();
                let _ = tauri::Listener::listen(handle, $event, move |event| {
                    let handle = handle_clone.clone();
                    tauri::async_runtime::spawn(async move {
                        $handler(event).await;
                    });
                });
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[async_listen] Event system not initialized, cannot register async listener for: {}", $event);
            }
        }
    };
    // 动态字面量
    (@single dyn $event:literal : $handler:expr) => {
        match $crate::GLOBAL_APPHANDLE.get() {
            Some(handle) => {
                let handle_clone = handle.clone();
                let _ = tauri::Listener::listen(handle, $event, move |event| {
                    let handle = handle_clone.clone();
                    tauri::async_runtime::spawn(async move {
                        $handler(event).await;
                    });
                });
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[async_listen] Event system not initialized, cannot register async listener for: {}", $event);
            }
        }
    };
    // 动态表达式
    (@single dyn ($event:expr) : $handler:expr) => {
        match $crate::GLOBAL_APPHANDLE.get() {
            Some(handle) => {
                let handle_clone = handle.clone();
                let _ = tauri::Listener::listen(handle, &$event, move |event| {
                    let handle = handle_clone.clone();
                    tauri::async_runtime::spawn(async move {
                        $handler(event).await;
                    });
                });
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[async_listen] Event system not initialized, cannot register dynamic async listener");
            }
        }
    };
}