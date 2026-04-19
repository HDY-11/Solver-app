use tauri::command;


use python_bridge;
/* 核心内容 */
#[command]
fn save_script(code: String, path: String) -> Result<(), String> {
    python_bridge::save_script(code, path)
        .map_err(|e| format!("保存脚本失败: {}", e))
}

#[command]
async fn run_script(path: String) -> Result<String, String> {
    python_bridge::run_script(path)
        .await
        .map_err(|e| format!("Python 执行失败: {}", e))
}

#[command]
fn read_script(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path)
        .map_err(|e| format!("读取文件失败: {}", e))
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            run_script,
            save_script,
            read_script,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
