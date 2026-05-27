use std::cell::RefCell;
use std::io::Write;
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
    // VFS 路径需要先从 VFS 读取内容 → 写入临时文件 → 再用真实路径执行
    let exec_path = if env::vfs_path::is_vfs(std::path::Path::new(&path)) {
        let mut vf = vfs::VirFile::open(&path)
            .map_err(|e| AppError::Io(e))
            .inspect_log("从 VFS 打开脚本失败")?;
        let mut content = String::new();
        std::io::Read::read_to_string(&mut vf, &mut content)
            .map_err(|e| AppError::Io(e))
            .inspect_log("从 VFS 读取脚本失败")?;

        let ext = std::path::Path::new(&path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("py");
        let tmp_path = std::env::temp_dir()
            .join(format!("solver_script_{}_{}.{}", std::process::id(), 
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default().as_nanos(), ext));
        let mut tmp_file = std::fs::File::create(&tmp_path)
            .map_err(|e| AppError::Io(e))
            .inspect_log("创建临时脚本文件失败")?;
        tmp_file.write_all(content.as_bytes())
            .map_err(|e| AppError::Io(e))
            .inspect_log("写入临时脚本失败")?;
        tmp_path.to_string_lossy().to_string()
    } else {
        path.clone()
    };

    let (stdout, stderr) = python_bridge::run_script(&exec_path)
        .await
        .map_err(|e| AppError::Python(e))
        .inspect_log("run_script failed")?;

    // 保存运行结果为 .run 文件到 (vfs)/C/运行记录/
    // 获取脚本当前版本号
    let script_version = if env::vfs_path::is_vfs(std::path::Path::new(&path)) {
        vfs::VirFile::open(&path)
            .and_then(|vf| vf.version())
            .unwrap_or_else(|_| "0.1.0".to_string())
    } else {
        "0.1.0".to_string()
    };

    let run_record = serde_json::json!({
        "script_path": &path,
        "script_version": &script_version,
        "stdout": &stdout,
        "stderr": &stderr,
    });
    let run_content = run_record.to_string();

    let script_name = std::path::Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unnamed");
    let run_path = format!("(vfs)/C/运行记录/{}.run", script_name);

    // 写入 .run 文件（VirFile::write 内部已做哈希去重，内容未变则跳过）
    if let Ok(mut f) = vfs::VirFile::open(&run_path)
        .or_else(|_| vfs::VirFile::create(&run_path))
    {
        let _ = std::io::Write::write_all(&mut f, run_content.as_bytes());
    }

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
    // 先尝试打开已有文件；不存在则创建（避免直接 create 导致 UNIQUE 冲突）
    let mut f = match vfs::VirFile::open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            vfs::VirFile::create(&path)
                .map_err(|e| AppError::Io(e))
                .inspect_log("创建文件失败")?
        }
        Err(e) => return Err(AppError::Io(e)),
    };
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
    // 如果是 .py 文件，级联删除对应的 .run 文件
    if path.ends_with(".py") {
        if let Some(name) = std::path::Path::new(&path).file_name().and_then(|n| n.to_str()) {
            let run_path = format!("(vfs)/C/运行记录/{}.run", name);
            if vfs::VirFile::exists(&run_path).unwrap_or(false) {
                if let Ok(f) = vfs::VirFile::open(&run_path) {
                    let _ = f.delete();
                }
            }
        }
    }
    vfs::VirFile::open(&path)
        .and_then(|f| f.delete())
        .map_err(|e| AppError::Io(e))
        .inspect_log("删除失败")
}

#[command]
fn vfs_rename(path: String, new_name: String) -> Result<(), AppError> {
    let old_name = std::path::Path::new(&path)
        .file_name().and_then(|n| n.to_str()).unwrap_or("");
    vfs::VirFile::open(&path)
        .and_then(|f| f.rename(&new_name))
        .map_err(|e| AppError::Io(e))
        .inspect_log("重命名失败")?;

    // 级联重命名 .run 文件
    if old_name.ends_with(".py") {
        let old_run = format!("(vfs)/C/运行记录/{}.run", old_name);
        let new_run_name = format!("{}.run", new_name);
        if vfs::VirFile::exists(&old_run).unwrap_or(false) {
            if let Ok(f) = vfs::VirFile::open(&old_run) {
                if let Err(e) = f.rename(&new_run_name) {
                    log::warn!("级联重命名 .run 失败 ({} → {}): {}", old_run, new_run_name, e);
                }
            }
        }
    }
    Ok(())
}

#[command]
fn vfs_set_version(path: String, new_version: String) -> Result<(), AppError> {
    vfs::VirFile::open(&path)
        .and_then(|f| f.set_version(&new_version))
        .map_err(|e| AppError::Io(e))
        .inspect_log("设置版本号失败")
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
struct VfsVersionInfo {
    node_id: i64,
    content_hash: String,
    size: i64,
    created_at: String,
}

#[command]
fn vfs_list_versions(path: String) -> Result<Vec<VfsVersionInfo>, AppError> {
    let versions: Vec<vfs::NodeVersionMeta> = vfs::VirFile::list_versions(&path)
        .map_err(|e| AppError::Io(e))
        .inspect_log("列出版本历史失败")?;
    Ok(versions.iter().map(|v| VfsVersionInfo {
        node_id: v.node_id,
        content_hash: v.content_hash.clone(),
        size: v.size,
        created_at: v.created_at.clone(),
    }).collect())
}

#[command]
fn vfs_read_version(path: String, content_hash: String) -> Result<String, AppError> {
    let bytes = vfs::VirFile::read_version(&path, &content_hash)
        .map_err(|e| AppError::Io(e))
        .inspect_log("读取历史版本失败")?;
    String::from_utf8(bytes)
        .map_err(|e| AppError::Other(anyhow::Error::from(e)))
        .inspect_log("解码历史版本 UTF-8 失败")
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
            vfs_rename,
            vfs_set_version,
            vfs_info,
            vfs_list_versions,
            vfs_read_version,
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