//! cli.rs — AnyCliBackend 枚举 + BackendRegistry（R9-D1修复：Arc消除unsafe）
//!
//! BackendRegistry 使用 Arc<AnyCliBackend> 而非裸指针，
//! 消除 register() 覆盖时的 UAF 风险。

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use cli_backend::{CliBackend, CliBackendType, CliExecResult};
use lua_runtime::LuaBackend;

/// 控制台后端枚举（手动 dispatch）
pub enum AnyCliBackend {
    Lua(LuaBackend),
    // Python(PythonBackend), // Phase 2
}

impl CliBackend for AnyCliBackend {
    fn exec(&self, tab_id: &str, code: &str) -> Result<CliExecResult, String> {
        match self { Self::Lua(b) => b.exec(tab_id, code) }
    }
    fn send_input(&self, tab_id: &str, input: &str) {
        match self { Self::Lua(b) => b.send_input(tab_id, input) }
    }
    fn interrupt(&self, tab_id: &str) -> Result<(), String> {
        match self { Self::Lua(b) => b.interrupt(tab_id) }
    }
    fn get_output(&self, tab_id: &str) -> Result<String, String> {
        match self { Self::Lua(b) => b.get_output(tab_id) }
    }
    fn get_output_range(&self, tab_id: &str, start: usize, end: usize) -> Result<String, String> {
        match self { Self::Lua(b) => b.get_output_range(tab_id, start, end) }
    }
    fn get_output_since(&self, tab_id: &str, cursor: usize) -> Result<(String, usize), String> {
        match self { Self::Lua(b) => b.get_output_since(tab_id, cursor) }
    }
    fn clear_output(&self, tab_id: &str) -> Result<(), String> {
        match self { Self::Lua(b) => b.clear_output(tab_id) }
    }
    fn destroy(&self, tab_id: &str) -> Result<(), String> {
        match self { Self::Lua(b) => b.destroy(tab_id) }
    }
    fn drain_input_requests(&self) -> Vec<String> {
        match self { Self::Lua(b) => b.drain_input_requests() }
    }
    fn update_permission(&self, permission: &str) -> Result<(), String> {
        match self { Self::Lua(b) => b.update_permission(permission) }
    }
    fn backend_type(&self) -> CliBackendType {
        match self { Self::Lua(_) => CliBackendType::Lua }
    }
}

/// BackendRegistry — 按 cliType 路由到对应后端（R9-D1修复：Arc + RwLock）
pub struct BackendRegistry {
    backends: RwLock<HashMap<String, Arc<AnyCliBackend>>>,
}

impl BackendRegistry {
    pub fn new() -> Self {
        Self { backends: RwLock::new(HashMap::new()) }
    }

    /// 注册后端（Arc 共享，安全无 UAF）
    pub fn register(&self, name: &str, backend: AnyCliBackend) {
        self.backends.write().unwrap().insert(name.to_string(), Arc::new(backend));
        log::info!("[BackendRegistry] 注册后端: {}", name);
    }

    /// 获取后端（Arc clone，释放锁后仍持有引用）
    pub fn get(&self, name: &str) -> Option<Arc<AnyCliBackend>> {
        self.backends.read().unwrap().get(name).cloned()
    }

    /// 获取或默认（向后兼容：找不到时 fallback 到 Lua）
    pub fn get_or_default(&self, name: &str) -> Arc<AnyCliBackend> {
        self.get(name).unwrap_or_else(|| {
            self.get("lua").expect("Lua backend not registered")
        })
    }
}
