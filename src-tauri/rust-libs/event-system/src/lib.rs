use tauri::{AppHandle, Emitter};
use error_system::{ResultLogExt, AppError};
use once_cell::sync::OnceCell;

pub static GLOBAL_APPHANDLE: OnceCell<AppHandle> = OnceCell::new();

// ── 事件注册表（预留，后续按需扩展） ────────────────
pub mod event_registry {
    /// 所有有效事件名（编译期检查用）
    pub const VALID_EVENTS: &[&str] = &[
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
    const fn const_str_equal(a: &str, b: &str) -> bool {
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
    // 静态
    (@single $event:literal : $payload:expr) => {
        // 编译期检查...
        match $crate::GLOBAL_APPHANDLE.get() {
            Some(handle) => {
                let _ = tauri::Emitter::emit(handle, $event, $payload);
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[emit] not initialized: {}", $event);
            }
        }
    };
    // 动态字面量
    (@single dyn $event:literal : $payload:expr) => {
        match $crate::GLOBAL_APPHANDLE.get() {
            Some(handle) => {
                let _ = tauri::Emitter::emit(handle, $event, $payload);
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[emit] not initialized: {}", $event);
            }
        }
    };
    // 动态表达式
    (@single dyn ($event:expr) : $payload:expr) => {
        match $crate::GLOBAL_APPHANDLE.get() {
            Some(handle) => {
                let _ = tauri::Emitter::emit(handle, $event, $payload);
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[emit] not initialized: dynamic event");
            }
        }
    };
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