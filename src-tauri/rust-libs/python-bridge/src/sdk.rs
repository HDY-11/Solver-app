//! 给 Python 脚本使用的 SDK 模块
//!
//! 通过 `import sdk; sdk.print("msg")` 向宿主应用推送实时输出事件。
//! 使用内联 PyModule 方式注入 sys.modules，无需编译 .pyd。

use pyo3::prelude::*;
use event_system::emit;

/// 从 Python 脚本推送的输出消息负载
#[derive(Clone, serde::Serialize)]
pub struct RunOutputPayload {
    pub run_path: String,
    pub content: String,
    pub timestamp: String,
}

/// `sdk.print(msg)` — 向宿主应用发送实时输出，自动附带当前 run_path 防止串台
#[pyfunction]
fn sdk_print(msg: String) -> PyResult<()> {
    let payload = {
        let ctx = crate::CURRENT_RUN.lock().unwrap_or_else(|e| e.into_inner());
        let run_path = ctx.as_ref().map(|c| c.run_path.clone()).unwrap_or_default();
        RunOutputPayload {
            run_path,
            content: msg,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    };
    // 推送到当前执行的收集缓冲区
    crate::push_sdk_output(payload.clone());
    // 同时通过 Tauri 事件推送到前端（附带 run_path）
    emit!("run-output": payload);
    Ok(())
}

/// 将 `sdk` 模块注入 Python 的 `sys.modules`
///
/// 在脚本执行前调用，使 `import sdk` 可直接使用。
pub fn register_sdk(py: Python<'_>) -> PyResult<()> {
    let m = PyModule::new(py, "sdk")?;
    m.add_function(wrap_pyfunction!(sdk_print, &m)?)?;
    let sys_modules = py.import("sys")?.getattr("modules")?;
    sys_modules.set_item("sdk", m)?;
    Ok(())
}
