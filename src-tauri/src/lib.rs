use std::cell::RefCell;
use dirs::data_dir;
use log_system::init_logging;
use tauri::{Manager, command};
use event_system::*;
use python_bridge;
use error_system::ResultLogExt;
use serde::Serialize; // 需要 serde 用于事件负载序列化

// 定义脚本运行结果的事件负载
#[derive(Clone, Serialize)]
struct ScriptResultPayload {
    path: String,
    stdout: String,
    stderr: String,
}

#[command]
fn save_script(code: String, path: String) -> Result<(), String> {
    python_bridge::save_script(code, path).map_err(|e| format!("{}", e))
}

#[command]
async fn run_script(path: String) -> Result<String, String> {
    // 调用脚本执行，获得分离的 stdout 和 stderr
    let (stdout, stderr) = python_bridge::run_script(&path)
        .await
        .map_err(|e| format!("执行失败: {}", e))?;

    // 构造并发射事件，让前端能够接收详细结果
    let payload = ScriptResultPayload {
        path: path.clone(),
        stdout: stdout.clone(),
        stderr: stderr.clone(),
    };
    // 这里直接用你的 emit! 宏发送给所有监听 "script-result" 的窗口
    emit!(dyn "script-result": payload);

    // 仍然返回 stdout 作为兼容
    Ok(stdout)
}

#[command]
fn read_script(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("读取文件失败: {}", e))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    eprintln!("[MAIN] 初始化日志系统...路径为{:?}", data_dir().unwrap());
    let (log_ctrl, log_handle) =
        init_logging(data_dir().unwrap(), data_dir().unwrap(), 4096).expect("初始化日志失败");

    let mut l_c = Some(log_ctrl);
    log::info!("日志系统已初始化，准备启动 Tauri 应用...");
    eprintln!("[MAIN] 启动 Tauri 应用...");

    tauri::Builder::default()
        .manage(log_handle.clone())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_log::Builder::default()
            .skip_logger()
            .level(tauri_plugin_log::log::LevelFilter::Debug)
            .targets([tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Webview)])
            .build()
        )
        .invoke_handler(tauri::generate_handler![
            run_script,
            save_script,
            read_script,
        ])
        .setup(move |app| {
            init_event_system(app.handle().clone()).unwrap_log();

            let window = app.get_webview_window("main").expect("获取窗口句柄失败");
            let log_ctrl = RefCell::new(l_c.take().expect("LogCtrl 已经被使用过了"));
            
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    log_ctrl.borrow_mut().shutdown();
                    std::process::exit(0);
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}