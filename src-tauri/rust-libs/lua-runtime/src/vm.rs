//! LuaVm — 单个 Lua 虚拟机封装（v3：权限模型替代沙箱）
//!
//! 每个标签页拥有一个独立的 LuaVm 实例。特性：
//! - 变量跨命令持久化（标签页存活期间）
//! - 输出通过 MemBuffer 缓冲（降低前端渲染压力）
//! - 权限模型：admin（完整 Lua + AppBridge）/ user（禁用 os.execute + io.popen）
//! - 交互式输入支持（通过 channel 实现 app.read()）
//! - 中断支持（手动 Ctrl+C，AtomicBool + mpsc 唤醒）

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use mem_buffer::MemBuffer;
use mlua::{Lua, Variadic};

use super::LuaPermission;

// =========================================================================
// 单个标签页的 Lua 虚拟机
// =========================================================================

pub struct LuaVm {
    tab_id: String,
    lua: Lua,
    output: Arc<Mutex<MemBuffer>>,
    /// 当前执行的输入通道（每次执行前设置，执行后清除）
    current_input_rx: Rc<RefCell<Option<Receiver<String>>>>,
    /// 是否正在等待交互式输入
    is_waiting: Arc<AtomicBool>,
    /// 是否已请求中断
    interrupted: Arc<AtomicBool>,
    /// 权限等级
    permission: LuaPermission,
}

impl LuaVm {
    /// 创建新的 Lua VM 实例。
    pub fn new(tab_id: &str, permission: LuaPermission) -> Self {
        let lua = Lua::new();
        let output = Arc::new(Mutex::new(MemBuffer::default()));

        let mut vm = Self {
            tab_id: tab_id.to_string(),
            lua,
            output,
            current_input_rx: Rc::new(RefCell::new(None)),
            is_waiting: Arc::new(AtomicBool::new(false)),
            interrupted: Arc::new(AtomicBool::new(false)),
            permission,
        };

        // 一次性静态环境设置
        vm.setup_print_redirect();
        vm.setup_app_bridge();
        vm.apply_permission_model();

        vm
    }

    /// 设置本次执行的输入通道（每次执行前调用）
    pub fn set_input_channel(&mut self, rx: Receiver<String>) {
        *self.current_input_rx.borrow_mut() = Some(rx);
    }

    /// 执行 Lua 代码并返回 (output, exit_code)
    ///
    /// exit_code: 0 = 成功, 1 = 运行时错误, 130 = 中断
    pub fn execute(&mut self, code: &str) -> (String, i32) {
        self.interrupted.store(false, Ordering::SeqCst);
        self.is_waiting.store(false, Ordering::SeqCst);

        // 为本次执行动态设置 app.read() 回调
        self.setup_dynamic_read();

        let result: mlua::Result<mlua::MultiValue> = self.lua.load(code).eval();

        // 清除本次执行的输入通道
        *self.current_input_rx.borrow_mut() = None;
        self.is_waiting.store(false, Ordering::SeqCst);

        match result {
            Ok(values) => {
                let mut out_buf = self.output.lock().unwrap();
                let ret_str = format_return_values(&values);
                if !ret_str.is_empty() {
                    let _ = write!(out_buf, "{}", ret_str);
                }
                drop(out_buf);
                let output = self.collect_output();
                (output, 0)
            }
            Err(err) => {
                let was_interrupted = self.interrupted.load(Ordering::SeqCst);
                let err_msg = if was_interrupted {
                    "[执行中断]".to_string()
                } else {
                    format!("[Lua 错误] {}", err)
                };
                {
                    let mut out_buf = self.output.lock().unwrap();
                    let _ = writeln!(out_buf, "{}", err_msg);
                }
                let output = self.collect_output();
                let exit_code = if was_interrupted { 130 } else { 1 };
                (output, exit_code)
            }
        }
    }

    /// 中断当前执行
    pub fn interrupt(&mut self) {
        self.interrupted.store(true, Ordering::SeqCst);
    }

    /// 是否正在等待交互式输入
    pub fn is_waiting_input(&self) -> bool {
        self.is_waiting.load(Ordering::SeqCst)
    }

    /// 获取输出缓冲区的全部内容
    pub fn get_output(&self) -> String {
        let buf = self.output.lock().unwrap();
        String::from_utf8_lossy(&buf.get_all()).to_string()
    }

    /// 获取输出缓冲区的指定范围
    pub fn get_output_range(&self, start: usize, end: usize) -> String {
        let buf = self.output.lock().unwrap();
        String::from_utf8_lossy(&buf.get_range(start, end)).to_string()
    }

    /// 增量读取：自 cursor 之后的新数据
    pub fn get_output_since(&self, cursor: usize) -> (String, usize) {
        let buf = self.output.lock().unwrap();
        let (data, new_cursor) = buf.read_since(cursor);
        (String::from_utf8_lossy(&data).to_string(), new_cursor)
    }

    /// 清空输出缓冲区
    pub fn clear_output(&self) {
        self.output.lock().unwrap().clear();
    }

    // ── 私有方法 ──

    fn collect_output(&self) -> String {
        let mut buf = self.output.lock().unwrap();
        let content = String::from_utf8_lossy(&buf.get_all()).to_string();
        buf.clear();
        content
    }

    /// 重定向 Lua 全局 print() 到 MemBuffer
    fn setup_print_redirect(&self) {
        let output = self.output.clone();
        let print_fn = self.lua.create_function(move |_, args: Variadic<String>| {
            let mut buf = output.lock().unwrap();
            let line = args.join("\t");
            let _ = writeln!(buf, "{}", line);
            Ok(())
        });

        if let Ok(f) = print_fn {
            let _ = self.lua.globals().set("print", f);
        }
    }

    /// 设置 app 全局表（AppBridge trait V1 + 交互式输入）
    ///
    /// - `app.read()` — 交互式输入（所有权限可用）
    /// - `app.vfs_tree()` — VFS 目录树（仅 admin）
    fn setup_app_bridge(&self) {
        // ── app.read() — 从输入通道同步等待（每次执行前通过 setup_dynamic_read 动态设置）──
        // 此处预置空表，app.read() 在 setup_dynamic_read 中注入

        // ── app.vfs_tree() — admin only ──
        let vfs_tree_fn = self.lua.create_function(move |lua, (): ()| {
            // 检查权限
            let permission: Option<String> = lua
                .globals()
                .get::<mlua::Table>("__permission")
                .ok()
                .and_then(|t| t.get::<String>("level").ok());

            let is_admin = permission.as_deref() == Some("admin");
            if !is_admin {
                return Err(mlua::Error::external(
                    "app.vfs_tree() 仅管理员权限可用。请在设置中切换为 admin 模式。"
                ));
            }

            // 返回占位 — 实际 VFS 集成由 Tauri 命令层完成
            Ok("[VFS 目录树 — 请通过 Tauri invoke 获取完整数据]".to_string())
        });

        let app_table = self.lua.create_table();
        if let Ok(t) = &app_table {
            if let Ok(f) = vfs_tree_fn {
                let _ = t.set("vfs_tree", f);
            }
            // app.read() 在 setup_dynamic_read 中动态设置
            let _ = self.lua.globals().set("app", t.clone());
        }
    }

    /// 为本次执行动态设置 app.read() 回调
    fn setup_dynamic_read(&mut self) {
        let cell = self.current_input_rx.clone();
        let is_waiting = self.is_waiting.clone();
        let interrupted = self.interrupted.clone();

        let read_fn = self.lua.create_function(move |_, (): ()| {
            is_waiting.store(true, Ordering::SeqCst);

            let recv_result = {
                let rx_guard = cell.borrow();
                match rx_guard.as_ref() {
                    Some(rx) => rx.recv(),
                    None => {
                        is_waiting.store(false, Ordering::SeqCst);
                        return Err(mlua::Error::external(
                            "app.read() 不可用：非交互模式。请在交互式上下文中调用。"
                        ));
                    }
                }
            }; // rx_guard 在此释放

            is_waiting.store(false, Ordering::SeqCst);

            match recv_result {
                Ok(input) => {
                    if interrupted.load(Ordering::SeqCst) {
                        Err(mlua::Error::external("执行被中断"))
                    } else {
                        Ok(input)
                    }
                }
                Err(_) => Err(mlua::Error::external("输入通道已关闭")),
            }
        });

        if let Ok(f) = read_fn {
            if let Ok(app) = self.lua.globals().get::<mlua::Table>("app") {
                let _ = app.set("read", f);
            }
        }
    }

    /// 根据权限模型启用/禁用 Lua 功能
    ///
    /// - Admin: 完整 Lua + AppBridge API — 不做任何限制
    /// - User: 禁用 os.execute + io.popen，其余模块保留
    fn apply_permission_model(&self) {
        // 在 Lua 全局表中存储权限信息，供 app.vfs_tree() 等 API 检查
        let perm_table = self.lua.create_table();
        if let Ok(t) = &perm_table {
            let _ = t.set("level", self.permission.as_str());
            let _ = self.lua.globals().set("__permission", t.clone());
        }

        match self.permission {
            LuaPermission::Admin => {
                log::info!("[lua-runtime] VM {} 以 Admin 权限启动 — 完整 Lua + AppBridge", self.tab_id);
                // 不做任何限制
            }
            LuaPermission::User => {
                log::info!("[lua-runtime] VM {} 以 User 权限启动 — 禁用 os.execute / io.popen", self.tab_id);

                // 禁用 os.execute
                if let Ok(os) = self.lua.globals().get::<mlua::Table>("os") {
                    let _ = os.set("execute", mlua::Value::Nil);
                }

                // 禁用 io.popen
                if let Ok(io) = self.lua.globals().get::<mlua::Table>("io") {
                    let _ = io.set("popen", mlua::Value::Nil);
                }
            }
        }
    }
}

// =========================================================================
// 返回值格式化
// =========================================================================

fn format_return_values(values: &mlua::MultiValue) -> String {
    let parts: Vec<String> = values.iter().map(value_to_string).collect();
    if parts.is_empty() {
        String::new()
    } else {
        parts.join("\t")
    }
}

fn value_to_string(value: &mlua::Value) -> String {
    match value {
        mlua::Value::Nil => "nil".to_string(),
        mlua::Value::Boolean(b) => b.to_string(),
        mlua::Value::Integer(i) => i.to_string(),
        mlua::Value::Number(n) => n.to_string(),
        mlua::Value::String(s) => s.to_string_lossy(),
        mlua::Value::Table(_) => "<table>".to_string(),
        mlua::Value::Function(_) => "<function>".to_string(),
        mlua::Value::Thread(_) => "<thread>".to_string(),
        mlua::Value::UserData(_) => "<userdata>".to_string(),
        mlua::Value::LightUserData(_) => "<lightuserdata>".to_string(),
        mlua::Value::Error(e) => format!("<error: {}>", e),
        _ => "<unknown>".to_string(),
    }
}

// =========================================================================
// Drop
// =========================================================================

impl Drop for LuaVm {
    fn drop(&mut self) {
        log::info!("[lua-runtime] VM 已销毁: tab_id={}", self.tab_id);
    }
}
