use tauri::{AppHandle, Emitter, EventId, Listener};
use std::collections::HashMap;

pub struct EventManager {
    handle: AppHandle,
    listeners: HashMap<String, EventId>,
}

impl EventManager {
    pub fn new(handle: AppHandle) -> Self {
        Self {
            handle,
            listeners: HashMap::new(),
        }
    }

    pub fn on<T, F>(&mut self, event: &str, callback: F)
    where
        T: serde::de::DeserializeOwned + 'static,
        F: Fn(T) + Send + 'static,
    {
        let id = self.handle.listen(event, move |e| {
            if let Ok(data) = serde_json::from_str::<T>(e.payload()) {
                callback(data);
            }
        });
        self.listeners.insert(event.to_string(), id);
    }

    /// 取消某个事件监听
    pub fn off(&mut self, event: &str) {
        if let Some(id) = self.listeners.remove(event) {
            self.handle.unlisten(id);
        }
    }

    /// 触发事件
    pub fn emit<S: serde::Serialize + Clone>(&self, event: &str, payload: S) {
        let _ = self.handle.emit(event, payload);
    }
}

/// 清理所有监听
impl Drop for EventManager {
    fn drop(&mut self) {
        for id in self.listeners.values() {
            self.handle.unlisten(*id);
        }
    }
}