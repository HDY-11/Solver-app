//! lua-runtime — 多实例 Lua 5.4 VM 管理器（v3 → v4：实现 CliBackend trait）
//!
//! # R9: CliBackend trait 实现
//!
//! `VmManager` 实现 `cli_backend::CliBackend` trait，通过委托现有方法零开销适配。
//! 架构准备 Phase 2 Python 后端接入。

pub mod vm;

use std::collections::HashMap;
use std::sync::mpsc::{self, Sender, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use serde::Serialize;
pub use vm::LuaVm;
pub use cli_backend::{CliBackend, CliExecResult};

// =========================================================================
// 公共类型
// =========================================================================

/// Lua 执行权限等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LuaPermission {
    Admin,
    User,
}

impl LuaPermission {
    pub fn from_str(s: &str) -> Self {
        match s {
            "admin" => Self::Admin,
            _ => Self::User,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::User => "user",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LuaExecResult {
    pub output: String,
    #[serde(rename = "exitCode")]
    pub exit_code: i32,
    #[serde(rename = "isWaitingInput")]
    pub is_waiting_input: bool,
}

// R9: LuaExecResult → CliExecResult 零拷贝转换
impl From<LuaExecResult> for CliExecResult {
    #[inline]
    fn from(r: LuaExecResult) -> Self {
        CliExecResult {
            output: r.output,
            exit_code: r.exit_code,
            is_waiting_input: r.is_waiting_input,
        }
    }
}

// =========================================================================
// 内部：Lua 运行时（仅存在于工作线程中）
// =========================================================================

struct LuaRuntime {
    vms: HashMap<String, LuaVm>,
    permission: LuaPermission,
    /// R13: 输入请求通知 — app.read() 阻塞前写入 tab_id，lua_exec 轮询
    input_requests: Arc<Mutex<Vec<String>>>,
}

impl LuaRuntime {
    fn new(permission: LuaPermission, input_requests: Arc<Mutex<Vec<String>>>) -> Self {
        Self { vms: HashMap::new(), permission, input_requests }
    }

    fn update_permission(&mut self, permission: LuaPermission) {
        self.permission = permission;
        log::info!("[lua-runtime] 权限模式切换为: {}", permission.as_str());
    }

    fn get_or_create_vm(&mut self, tab_id: &str) -> &mut LuaVm {
        if !self.vms.contains_key(tab_id) {
            let vm = LuaVm::new(tab_id, self.permission, self.input_requests.clone());
            self.vms.insert(tab_id.to_string(), vm);
            log::info!("[lua-runtime] 创建 VM: tab_id={} permission={}", tab_id, self.permission.as_str());
        }
        self.vms.get_mut(tab_id).unwrap()
    }

    fn exec(&mut self, tab_id: &str, code: &str) -> LuaExecResult {
        let vm = self.get_or_create_vm(tab_id);
        let (input_tx, input_rx) = mpsc::channel::<String>();
        vm.set_input_channel(input_rx);
        register_input_sender(tab_id, input_tx);

        let (output, exit_code) = vm.execute(code);
        let is_waiting = vm.is_waiting_input();
        if !is_waiting {
            unregister_input_sender(tab_id);
        }

        LuaExecResult { output, exit_code, is_waiting_input: is_waiting }
    }

    fn interrupt(&mut self, tab_id: &str) {
        if let Some(vm) = self.vms.get_mut(tab_id) {
            vm.interrupt();
        }
        send_input_to_vm(tab_id, String::new());
    }

    fn get_output(&self, tab_id: &str) -> String {
        self.vms.get(tab_id).map(|vm| vm.get_output()).unwrap_or_default()
    }

    fn get_output_range(&self, tab_id: &str, start: usize, end: usize) -> String {
        self.vms.get(tab_id).map(|vm| vm.get_output_range(start, end)).unwrap_or_default()
    }

    fn get_output_since(&self, tab_id: &str, cursor: usize) -> (String, usize) {
        self.vms.get(tab_id).map(|vm| vm.get_output_since(cursor)).unwrap_or_default()
    }

    fn clear_output(&mut self, tab_id: &str) {
        if let Some(vm) = self.vms.get(tab_id) {
            vm.clear_output();
        }
    }

    fn destroy_vm(&mut self, tab_id: &str) {
        if self.vms.remove(tab_id).is_some() {
            log::info!("[lua-runtime] 销毁 VM: tab_id={}", tab_id);
        }
        unregister_input_sender(tab_id);
    }
}

// =========================================================================
// 全局 per-VM 输入通道表（lua_send_input 直接写入，绕过工作线程主队列）
// =========================================================================

static VM_INPUT_SENDERS: std::sync::LazyLock<Mutex<HashMap<String, mpsc::Sender<String>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

fn register_input_sender(tab_id: &str, tx: mpsc::Sender<String>) {
    VM_INPUT_SENDERS.lock().unwrap().insert(tab_id.to_string(), tx);
}

fn unregister_input_sender(tab_id: &str) {
    VM_INPUT_SENDERS.lock().unwrap().remove(tab_id);
}

pub fn send_input_to_vm(tab_id: &str, input: String) -> bool {
    if let Some(tx) = VM_INPUT_SENDERS.lock().unwrap().get(tab_id) {
        tx.send(input).is_ok()
    } else {
        log::warn!("[lua-runtime] send_input_to_vm: 未找到 VM 输入通道 tab_id={}", tab_id);
        false
    }
}

// =========================================================================
// 工作线程命令协议
// =========================================================================

enum VmAction {
    Execute { code: String },
    Interrupt,
    GetOutput,
    GetOutputRange { start: usize, end: usize },
    GetOutputSince { cursor: usize },
    ClearOutput,
    Destroy,
    UpdatePermission { permission: LuaPermission },
}

struct VmRequest {
    tab_id: String,
    action: VmAction,
    response_tx: mpsc::Sender<VmResponse>,
}

enum VmResponse {
    ExecResult(LuaExecResult),
    Output(String),
    OutputSince(String, usize),
    Ok,
}

// =========================================================================
// 公开 API：VmManager — Tauri State 持有
// =========================================================================

pub struct VmManager {
    cmd_tx: mpsc::Sender<VmRequest>,
    /// R13: app.read() 阻塞时写入的 tab_id 列表
    pub input_requests: Arc<Mutex<Vec<String>>>,
}

impl VmManager {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<VmRequest>();
        let input_requests = Arc::new(Mutex::new(Vec::new()));
        let ir = input_requests.clone();

        thread::Builder::new()
            .name("lua-worker".into())
            .spawn(move || { run_worker_loop(cmd_rx, ir); })
            .expect("[lua-runtime] 无法启动 Lua 工作线程");

        log::info!("[lua-runtime] Lua 工作线程已启动");
        Self { cmd_tx, input_requests }
    }

    pub fn exec(&self, tab_id: &str, code: &str) -> Result<LuaExecResult, String> {
        let (resp_tx, resp_rx) = mpsc::channel();
        self.cmd_tx.send(VmRequest {
            tab_id: tab_id.to_string(),
            action: VmAction::Execute { code: code.to_string() },
            response_tx: resp_tx,
        }).map_err(|e| format!("Lua 工作线程通道已关闭: {}", e))?;

        match resp_rx.recv() {
            Ok(VmResponse::ExecResult(r)) => Ok(r),
            Ok(_) => Err("意外的响应类型".into()),
            Err(e) => Err(format!("Lua 工作线程无响应: {}", e)),
        }
    }

    pub fn send_input(&self, tab_id: &str, input: &str) {
        send_input_to_vm(tab_id, input.to_string());
    }

    pub fn interrupt(&self, tab_id: &str) -> Result<(), String> {
        let (resp_tx, resp_rx) = mpsc::channel();
        self.cmd_tx.send(VmRequest {
            tab_id: tab_id.to_string(),
            action: VmAction::Interrupt,
            response_tx: resp_tx,
        }).map_err(|e| format!("通道关闭: {}", e))?;
        match resp_rx.recv() {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("无响应: {}", e)),
        }
    }

    pub fn get_output(&self, tab_id: &str) -> Result<String, String> {
        self.send_simple(VmAction::GetOutput, tab_id)
    }

    pub fn get_output_range(&self, tab_id: &str, start: usize, end: usize) -> Result<String, String> {
        let (resp_tx, resp_rx) = mpsc::channel();
        self.cmd_tx.send(VmRequest {
            tab_id: tab_id.to_string(),
            action: VmAction::GetOutputRange { start, end },
            response_tx: resp_tx,
        }).map_err(|e| format!("通道关闭: {}", e))?;
        match resp_rx.recv() {
            Ok(VmResponse::Output(s)) => Ok(s),
            _ => Err("无响应".into()),
        }
    }

    pub fn get_output_since(&self, tab_id: &str, cursor: usize) -> Result<(String, usize), String> {
        let (resp_tx, resp_rx) = mpsc::channel();
        self.cmd_tx.send(VmRequest {
            tab_id: tab_id.to_string(),
            action: VmAction::GetOutputSince { cursor },
            response_tx: resp_tx,
        }).map_err(|e| format!("通道关闭: {}", e))?;
        match resp_rx.recv() {
            Ok(VmResponse::OutputSince(data, new_cursor)) => Ok((data, new_cursor)),
            _ => Err("无响应".into()),
        }
    }

    pub fn destroy_vm(&self, tab_id: &str) -> Result<(), String> {
        let _ = self.send_simple(VmAction::Destroy, tab_id)?;
        Ok(())
    }

    pub fn update_permission(&self, permission: LuaPermission) -> Result<(), String> {
        let (resp_tx, resp_rx) = mpsc::channel();
        self.cmd_tx.send(VmRequest {
            tab_id: String::new(),
            action: VmAction::UpdatePermission { permission },
            response_tx: resp_tx,
        }).map_err(|e| format!("通道关闭: {}", e))?;
        match resp_rx.recv() {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("无响应: {}", e)),
        }
    }

    /// R13: 取走并清空输入请求列表（由 lua_exec Tauri command 调用）
    pub fn drain_input_requests(&self) -> Vec<String> {
        self.input_requests.lock().unwrap().drain(..).collect()
    }

    fn send_simple(&self, action: VmAction, tab_id: &str) -> Result<String, String> {
        let (resp_tx, resp_rx) = mpsc::channel();
        self.cmd_tx.send(VmRequest {
            tab_id: tab_id.to_string(),
            action,
            response_tx: resp_tx,
        }).map_err(|e| format!("通道关闭: {}", e))?;
        match resp_rx.recv() {
            Ok(VmResponse::Output(s)) => Ok(s),
            Ok(VmResponse::Ok) => Ok(String::new()),
            _ => Err("无响应".into()),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// R9: CliBackend trait 实现（委托 VmManager 现有方法，零开销）
// ═══════════════════════════════════════════════════════════════════

impl CliBackend for VmManager {
    fn exec(&self, tab_id: &str, code: &str) -> Result<CliExecResult, String> {
        // 委托原生 exec → 转换 LuaExecResult → CliExecResult
        self.exec(tab_id, code).map(Into::into)
    }

    fn send_input(&self, tab_id: &str, input: &str) {
        self.send_input(tab_id, input);
    }

    fn interrupt(&self, tab_id: &str) -> Result<(), String> {
        self.interrupt(tab_id)
    }

    fn get_output(&self, tab_id: &str) -> Result<String, String> {
        self.get_output(tab_id)
    }

    fn destroy(&self, tab_id: &str) -> Result<(), String> {
        self.destroy_vm(tab_id)
    }
}

// =========================================================================
// 工作线程主循环
// =========================================================================

fn run_worker_loop(rx: Receiver<VmRequest>, input_requests: Arc<Mutex<Vec<String>>>) {
    let mut runtime = LuaRuntime::new(LuaPermission::User, input_requests);

    for req in rx {
        let response = match req.action {
            VmAction::Execute { code } => {
                VmResponse::ExecResult(runtime.exec(&req.tab_id, &code))
            }
            VmAction::Interrupt => {
                runtime.interrupt(&req.tab_id);
                VmResponse::Ok
            }
            VmAction::GetOutput => {
                VmResponse::Output(runtime.get_output(&req.tab_id))
            }
            VmAction::GetOutputRange { start, end } => {
                VmResponse::Output(runtime.get_output_range(&req.tab_id, start, end))
            }
            VmAction::GetOutputSince { cursor } => {
                let (data, new_cursor) = runtime.get_output_since(&req.tab_id, cursor);
                VmResponse::OutputSince(data, new_cursor)
            }
            VmAction::ClearOutput => {
                runtime.clear_output(&req.tab_id);
                VmResponse::Ok
            }
            VmAction::Destroy => {
                runtime.destroy_vm(&req.tab_id);
                VmResponse::Ok
            }
            VmAction::UpdatePermission { permission } => {
                runtime.update_permission(permission);
                VmResponse::Ok
            }
        };
        let _ = req.response_tx.send(response);
    }

    log::info!("[lua-runtime] 工作线程退出");
}
