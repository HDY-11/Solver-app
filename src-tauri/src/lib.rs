use log_system::init_logging;
use std::cell::RefCell;
use tauri::{command, Manager};
use crossbeam_channel::bounded;
use serde_json::Value;

use python_bridge;

/* 核心内容 */
#[command]
fn save_script(code: String, path: String) -> Result<(), String> {
    python_bridge::save_script(code, path).map_err(|e| format!("{}", e))
}

#[command]
async fn run_script(path: String) -> Result<String, String> {
    python_bridge::run_script(path)
        .await
        .map_err(|e| format!("{}", e))
}

#[command]
fn read_script(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("读取文件失败: {}", e))
}
/*
#[command]
fn start_solver_event_loop(app_handle: AppHandle){
    let (tx, rx) = bounded(64);

}
*/
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    eprintln!("[MAIN] 初始化日志系统...");
    let (log_ctrl, log_handle) =
        init_logging("./logs/high.log", "./logs/low.log", 4096).expect("初始化日志失败");
    log::set_max_level(log::LevelFilter::Debug);
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
            .targets([tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Webview),])
            .build()
        )
        .invoke_handler(tauri::generate_handler![
            run_script,
            save_script,
            read_script,
        ])
        .setup(move |app| {

            let window = app.get_webview_window("main").expect("获取窗口句柄失败");

            let log_ctrl = RefCell::new(l_c.take().expect("LogCtrl 已经被使用过了"));

            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    // 阻止窗口直接关闭
                    api.prevent_close();

                    log_ctrl.borrow_mut().shutdown(); // 先关闭日志系统，确保日志完整写入
                    std::process::exit(0); // 然后退出程序
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
