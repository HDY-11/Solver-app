//! lua-runtime — 多实例 Lua VM 管理器（v3：专用工作线程 + 双通道）
//!
//! # 架构
//!
//! ```text
//! Tauri Command Thread            Lua Worker Thread
//!        │                              │
//!        ├─ lua_exec ─────────────────→│ VmRequest channel（主命令队列）
//!        │                              ├─ LuaRuntime::exec()
//!        │←── VmResponse ──────────────┤ response channel
//!        │                              │
//!        ├─ lua_send_input ───────────→│ per-VM input channel（绕过主队列）
//!        │                              ├─ 唤醒阻塞的 app.read()
//!        │                              │
//!        ├─ lua_interrupt ────────────→│ VmRequest channel + per-VM input channel
//! ```
//!
//! # 关键设计决策
//!
//! - **专用工作线程**：mlua::Lua 是 !Send + !Sync，所有 Lua 操作在单一线程中执行
//! - **双通道设计**：主命令通道 + per-VM 输入通道，避免 app.read() 阻塞死锁
//! - **每标签页独立 VM**：变量跨命令保持，关闭标签页时销毁
//! - **权限模型（v3）**：PowerShell 风格双权限 — admin（完整 Lua + AppBridge）/ user（禁用 os.execute + io.popen）

mod vm;

use std::collections::HashMap;
use std::sync::mpsc::{self, Sender, Receiver};
use std::sync::{Arc, Mutex, LazyLock};
use std::thread;
use serde::Serialize;
pub use vm::LuaVm;

// =========================================================================
// 公共类型
// =========================================================================

/// Lua 执行权限等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LuaPermission {
    /// 完整 Lua 访问 + AppBridge API
    Admin,
    /// 禁用 os.execute / io.popen，其余可用
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

// =========================================================================
// 内部：Lua 运行时（仅存在于工作线程中）
// =========================================================================

struct LuaRuntime {
    vms: HashMap<String, LuaVm>,
    permission: LuaPermission,
}

impl LuaRuntime {
    fn new(permission: LuaPermission) -> Self {
        Self { vms: HashMap::new(), permission }
    }

    fn update_permission(&mut self, permission: LuaPermission) {
        self.permission = permission;
        log::info!("[lua-runtime] 权限模式切换为: {}", permission.as_str());
    }

    fn get_or_create_vm(&mut self, tab_id: &str) -> &mut LuaVm {
        if !self.vms.contains_key(tab_id) {
            let vm = LuaVm::new(tab_id, self.permission);
            self.vms.insert(tab_id.to_string(), vm);
            log::info!("[lua-runtime] 创建 VM: tab_id={} permission={}", tab_id, self.permission.as_str());
        }
        self.vms.get_mut(tab_id).unwrap()
    }

    fn exec(&mut self, tab_id: &str, code: &str) -> LuaExecResult {
        let vm = self.get_or_create_vm(tab_id);

        // 创建 per-VM 输入通道（每次执行一个独立通道）
        let (input_tx, input_rx) = mpsc::channel::<String>();
        vm.set_input_channel(input_rx);

        // 注册到全局输入通道表，供 lua_send_input 直接写入
        register_input_sender(tab_id, input_tx);

        let (output, exit_code) = vm.execute(code);

        // 检查是否仍在等待输入
        let is_waiting = vm.is_waiting_input();
        if !is_waiting {
            unregister_input_sender(tab_id);
        }

        LuaExecResult { output, exit_code, is_waiting_input: is_waiting }
    }

    fn interrupt(&mut self, tab_id: &str) {
        if let Some(vm) = self.vms.get_mut(tab_id) {
            vm.interrupt();
            log::info!("[lua-runtime] 中断 VM: tab_id={}", tab_id);
        }
        // 向输入通道发送空串唤醒可能的阻塞 recv()
        send_input_to_vm(tab_id, String::new());
    }

    fn get_output(&self, tab_id: &str) -> String {
        self.vms.get(tab_id).map(|vm| vm.get_output()).unwrap_or_default()
    }

    fn get_output_range(&self, tab_id: &str, start: usize, end: usize) -> String {
        self.vms.get(tab_id).map(|vm| vm.get_output_range(start, end)).unwrap_or_default()
    }

    /// 增量读取：返回自 cursor 之后的新数据
    fn get_output_since(&self, tab_id: &str, cursor: usize) -> (String, usize) {
        self.vms.get(tab_id)
            .map(|vm| vm.get_output_since(cursor))
            .unwrap_or_default()
    }

    fn destroy_vm(&mut self, tab_id: &str) {
        if self.vms.remove(tab_id).is_some() {
            log::info!("[lua-runtime] 销毁 VM: tab_id={}", tab_id);
        }
        unregister_input_sender(tab_id);
    }

    fn clear_output(&mut self, tab_id: &str) {
        if let Some(vm) = self.vms.get(tab_id) {
            vm.clear_output();
        }
    }

    fn is_waiting(&self, tab_id: &str) -> bool {
        self.vms.get(tab_id).map(|vm| vm.is_waiting_input()).unwrap_or(false)
    }
}

// =========================================================================
// 全局 per-VM 输入通道表
// =========================================================================

static VM_INPUT_SENDERS: LazyLock<Mutex<HashMap<String, mpsc::Sender<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn register_input_sender(tab_id: &str, tx: mpsc::Sender<String>) {
    VM_INPUT_SENDERS.lock().unwrap().insert(tab_id.to_string(), tx);
}

fn unregister_input_sender(tab_id: &str) {
    VM_INPUT_SENDERS.lock().unwrap().remove(tab_id);
}

/// 外部调用：向指定 VM 的输入通道发送数据（绕过 Lua 工作线程主队列）
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

/// VmManager — 持有到 Lua 工作线程的发送端
///
/// 本身是 Send + Sync，可安全置于 Tauri State 中。
/// 所有 Lua 操作通过 mpsc channel 转发到专用工作线程。
pub struct VmManager {
    cmd_tx: mpsc::Sender<VmRequest>,
}

impl VmManager {
    /// 启动 Lua 工作线程并返回管理器
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<VmRequest>();

        thread::Builder::new()
            .name("lua-worker".into())
            .spawn(move || {
                run_worker_loop(cmd_rx);
            })
            .expect("[lua-runtime] 无法启动 Lua 工作线程");

        log::info!("[lua-runtime] Lua 工作线程已启动");
        Self { cmd_tx }
    }

    /// 执行 Lua 代码（同步阻塞等待结果）
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

    /// 向等待输入的 VM 发送数据（绕过主命令队列）
    pub fn send_input(&self, tab_id: &str, input: &str) {
        send_input_to_vm(tab_id, input.to_string());
    }

    /// 中断指定 VM 的执行
    pub fn interrupt(&self, tab_id: &str) -> Result<(), String> {
        let (resp_tx, resp_rx) = mpsc::channel();
        self.cmd_tx.send(VmRequest {
            tab_id: tab_id.to_string(),
            action: VmAction::Interrupt,
            response_tx: resp_tx,
        }).map_err(|e| format!("通道关闭: {}", e))?;
        match resp_rx.recv() { Ok(_) => Ok(()), Err(e) => Err(format!("无响应: {}", e)) }
    }

    /// 获取输出缓冲区全部内容
    pub fn get_output(&self, tab_id: &str) -> Result<String, String> {
        self.send_get_output(|req| req.action = VmAction::GetOutput, tab_id)
    }

    /// 获取输出缓冲区指定范围
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

    /// 增量读取：自 cursor 之后的新数据
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

    /// 销毁指定 VM
    pub fn destroy_vm(&self, tab_id: &str) -> Result<(), String> {
        let (resp_tx, resp_rx) = mpsc::channel();
        self.cmd_tx.send(VmRequest {
            tab_id: tab_id.to_string(),
            action: VmAction::Destroy,
            response_tx: resp_tx,
        }).map_err(|e| format!("通道关闭: {}", e))?;
        match resp_rx.recv() { Ok(_) => Ok(()), Err(e) => Err(format!("无响应: {}", e)) }
    }

    /// 更新全局权限模式
    pub fn update_permission(&self, permission: LuaPermission) -> Result<(), String> {
        let (resp_tx, resp_rx) = mpsc::channel();
        self.cmd_tx.send(VmRequest {
            tab_id: String::new(),
            action: VmAction::UpdatePermission { permission },
            response_tx: resp_tx,
        }).map_err(|e| format!("通道关闭: {}", e))?;
        match resp_rx.recv() { Ok(_) => Ok(()), Err(e) => Err(format!("无响应: {}", e)) }
    }

    // ── 私有辅助 ──

    fn send_get_output(&self, set_action: impl FnOnce(&mut VmRequest), tab_id: &str) -> Result<String, String> {
        let (resp_tx, resp_rx) = mpsc::channel();
        let mut req = VmRequest {
            tab_id: tab_id.to_string(),
            action: VmAction::GetOutput,
            response_tx: resp_tx,
        };
        set_action(&mut req);
        self.cmd_tx.send(req).map_err(|e| format!("通道关闭: {}", e))?;

        match resp_rx.recv() {
            Ok(VmResponse::Output(s)) => Ok(s),
            _ => Err("无响应".into()),
        }
    }
}

// =========================================================================
// 工作线程主循环
// =========================================================================

fn run_worker_loop(rx: Receiver<VmRequest>) {
    let mut runtime = LuaRuntime::new(LuaPermission::User);

    for req in rx {
        let response = match req.action {
            VmAction::Execute { code } => {
                let result = runtime.exec(&req.tab_id, &code);
                VmResponse::ExecResult(result)
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
