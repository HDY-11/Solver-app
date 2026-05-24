use std::cell::RefCell;
use log_system::init_logging;
use tauri::{Manager, command};
use event_system::*;
use error_system::{ResultLogExt, OptionLogExt, AppError};
use serde::Serialize;
use env_system as env;
use anyhow::Error;

#[derive(Clone, Serialize)]
struct ScriptResultPayload {
    path: String,
    stdout: String,
    stderr: String,
}

#[command]
fn save_script(code: String, path: String) -> Result<(), AppError> {
    python_bridge::save_script(code, path)
        .map_err(|e| AppError::Other(Error::from_boxed(e.into_inner().expect_log("raraly error"))))
        .inspect_log("save_script failed")
}

#[command]
async fn run_script(path: String) -> Result<String, AppError> {
    let (stdout, stderr) = python_bridge::run_script(&path)
        .await
        .map_err(|e| AppError::Python(e))
        .inspect_log("run_script failed")?;

    let payload = ScriptResultPayload {
        path: path.clone(),
        stdout: stdout.clone(),
        stderr: stderr.clone(),
    };
    emit!(dyn "script-result": payload);
    Ok(stdout)
}

#[command]
fn read_script(path: String) -> Result<String, AppError> {
    std::fs::read_to_string(&path)
        .map_err(|e| AppError::Other(Error::from_boxed(e.into_inner().expect_log("raraly error"))))
        .inspect_log("read_script failed")
}

#[command]
fn vfs_read(path: String) -> Result<String, AppError> {
    let mut f = vfs::VirFile::open(&path)   // ← open，不是 create
        .map_err(|e| AppError::Io(e))
        .inspect_log("打开文件失败")?;
    let mut content = String::new();
    std::io::Read::read_to_string(&mut f, &mut content)
        .map_err(|e| AppError::Io(e))
        .inspect_log("读取文件失败")?;
    Ok(content)
}

#[command]
fn vfs_write(path: String, content: String) -> Result<(), AppError> {
    let mut f = vfs::VirFile::create(&path)
        .map_err(|e| AppError::Io(e))
        .inspect_log("创建文件失败")?;
    std::io::Write::write_all(&mut f, content.as_bytes())
        .map_err(|e| AppError::Io(e))
        .inspect_log("写入文件失败")?;
    Ok(())
}
#[command]
fn vfs_list_dir(path: String) -> Result<Vec<vfs::VfsNodeInfo>, AppError> {
    vfs::VirFile::list_children(&path)
        .map_err(|e| AppError::Io(e))
        .inspect_log("列出目录失败")
}

#[command]
fn vfs_exists(path: String) -> Result<bool, AppError> {
    vfs::VirFile::exists(&path)
        .map_err(|e| AppError::Io(e))
        .inspect_log("查询失败")
}

#[command]
fn vfs_delete(path: String) -> Result<(), AppError> {
    vfs::VirFile::delete(&path)
        .map_err(|e| AppError::Io(e))
        .inspect_log("删除失败")
}

#[command]
fn vfs_create_dir(path: String) -> Result<(), AppError> {
    vfs::VirFile::create_dir(&path)
        .map_err(|e| AppError::Io(e))
        .inspect_log("创建目录失败")
}

#[command]
fn vfs_info() -> Result<VfsInfo, AppError> {
    let c_exists = vfs::VirFile::exists("(vfs)/C").unwrap_or(false);
    let c_children = if c_exists {
        vfs::VirFile::list_children("(vfs)/C")
            .map_err(|e| AppError::Io(e))
            .inspect_log("列出目录失败")?
    } else {
        vec![]
    };
    let used = c_children.iter().filter_map(|n| n.size).sum::<u64>();
    
    Ok(VfsInfo {
        c_exists,
        c_used: used,
        c_total: 64 * 1024 * 1024,
        c_node_count: c_children.len() as u64,
    })
}

#[derive(Clone, Serialize)]
struct VfsInfo {
    c_exists: bool,
    c_used: u64,
    c_total: u64,
    c_node_count: u64,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    eprintln!("[MAIN] 初始化日志系统...");
    let (log_ctrl, log_handle) =
        init_logging(env::log_dir(), env::log_dir(), 4096).expect("初始化日志失败");

    let mut l_c = Some(log_ctrl);
    log::info!("日志系统已初始化，准备启动 Tauri 应用...");

    eprintln!("[MAIN] 初始化 VFS...");
    vfs::init_vfs(
        &env::database_path(),
        &[("C", 64 * 1024 * 1024)],
    )
    .expect("VFS 初始化失败");
    eprintln!("[MAIN] VFS 初始化完成");

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
            vfs_write,
            vfs_read,
            vfs_list_dir,
            vfs_exists,
            vfs_delete,
            vfs_create_dir,
            vfs_info,
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