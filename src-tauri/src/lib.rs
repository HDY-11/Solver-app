use std::cell::RefCell;
use log_system::init_logging;
use tauri::{Manager, command};
use event_system::*;
use error_system::{ResultLogExt, OptionLogExt, AppError};
use serde::Serialize;
use env_system as env;
use anyhow::Error;

mod config;

#[derive(Clone, Serialize)]
struct RunScriptResponse {
    run_path: String,
    /// "cached" = 已有结果直接显示, "running" = 后台执行中
    status: String,
}

#[derive(Clone, Serialize)]
struct ScriptResultPayload {
    path: String,
    stdout: String,
    stderr: String,
}

#[command]
fn save_script(code: String, path: String) -> Result<(), AppError> {
    python_bridge::save_script(code, path)
        .map_err(|e| AppError::Other(Error::from_boxed(e.into_inner().expect_log("rarely error"))))
        .inspect_log("save_script failed")
}

#[command]
async fn run_script(path: String) -> Result<RunScriptResponse, AppError> {
    use sha2::{Sha256, Digest};

    // ── 1. 读取脚本内容 + 版本号 + 计算哈希 ──
    let (content, script_version) = if env::vfs_path::is_vfs(std::path::Path::new(&path)) {
        let mut vf = vfs::VirFile::open(&path)
            .map_err(|e| AppError::Io(e))
            .inspect_log("从 VFS 打开脚本失败")?;
        let version = vf.version().unwrap_or_else(|_| "0.1.0".to_string());
        let mut c = String::new();
        std::io::Read::read_to_string(&mut vf, &mut c)
            .map_err(|e| AppError::Io(e))
            .inspect_log("从 VFS 读取脚本失败")?;
        (c, version)
    } else {
        let c = std::fs::read_to_string(&path)
            .map_err(|e| AppError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        (c, "0.1.0".to_string())
    };

    let script_hash = {
        let mut h = Sha256::new();
        h.update(content.as_bytes());
        hex::encode(h.finalize())
    };

    let script_name = std::path::Path::new(&path)
        .file_name().and_then(|n| n.to_str()).unwrap_or("unnamed");
    // 提取卷名，区分 C 盘 / B 盘同名脚本
    let volume = env::vfs_volume(std::path::Path::new(&path)).unwrap_or_else(|| "C".to_string());
    let run_name = format!("{}.run", script_name);
    let run_dir = format!("(vfs)/{}/运行记录", volume);
    let run_path = format!("{}/{}", run_dir, run_name);

    // ── 2. 去重查询（JSON 解析，非字符串 contains）──
    let linked_pattern = format!("%\"script_hash\":\"{}\"%", script_hash);
    {
        let candidates = vfs::query_run_nodes_by_linked(&linked_pattern)
            .map_err(|e| AppError::Io(e))?;

        for c in &candidates {
            if let Some(ref lf) = c.linked_files {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(lf) {
                    let sp = parsed["script_path"].as_str().unwrap_or("");
                    let sv = parsed["script_version"].as_str().unwrap_or("");
                    if sp == path && sv == script_version {
                        log::info!("[run_script] 精确命中缓存: {}", run_path);
                        return Ok(RunScriptResponse { run_path, status: "cached".into() });
                    }
                }
            }
        }

        // 部分匹配：哈希相同，复用 BLOB
        if let Some(src) = candidates.first() {
            if let (Some(off), Some(sz), Some(ref ch)) =
                (src.storage_offset, src.size, &src.content_hash)
            {
                let lf = serde_json::json!({
                    "script_hash": &script_hash,
                    "script_path": &path,
                    "script_version": &script_version,
                    "volume": &volume,
                }).to_string();
                vfs::VirFile::create_run_node_from_source(
                    &run_name, &lf, off, sz, ch, &volume, &run_dir,
                ).map_err(|e| AppError::Io(e))?;
                log::info!("[run_script] 部分命中，复用 BLOB: {}", run_path);
                return Ok(RunScriptResponse { run_path, status: "cached".into() });
            }
        }
    }

    // ── 3. 无缓存 → 创建空节点 + 后台执行 ──
    let lf = serde_json::json!({
        "script_hash": &script_hash,
        "script_path": &path,
        "script_version": &script_version,
        "volume": &volume,
    }).to_string();
    vfs::VirFile::create_run_node(&run_name, &lf, &volume, &run_dir)
        .map_err(|e| AppError::Io(e))?;

    // 提取到临时文件
    let ext = std::path::Path::new(&path)
        .extension().and_then(|e| e.to_str()).unwrap_or("py");
    let tmp_path = std::env::temp_dir().join(format!(
        "solver_script_{}_{}.{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_nanos(),
        ext,
    ));
    std::fs::write(&tmp_path, &content)
        .map_err(|e| AppError::Io(e))
        .inspect_log("写入临时脚本失败")?;

    let run_path_clone = run_path.clone();
    let path_clone = path.clone();

    // 后台执行
    python_bridge::begin_run(&run_path);
    tauri::async_runtime::spawn(async move {
        let result = python_bridge::run_script(&tmp_path.to_string_lossy()).await;
        match result {
            Ok(r) => {
                let run_content = serde_json::json!({
                    "stdout": r.stdout,
                    "stderr": r.stderr,
                    "outputs": r.outputs,
                }).to_string();

                if let Ok(mut f) = vfs::VirFile::open(&run_path_clone)
                    .or_else(|_| vfs::VirFile::create(&run_path_clone))
                {
                    if let Err(e) = std::io::Write::write_all(&mut f, run_content.as_bytes()) {
                        log::error!("[run_script] BLOB 写入失败 ({}): {}", run_path_clone, e);
                    }
                } else {
                    log::error!("[run_script] 无法打开 .run 文件写入: {}", run_path_clone);
                }

                let payload = ScriptResultPayload {
                    path: path_clone,
                    stdout: r.stdout,
                    stderr: r.stderr,
                };
                emit!("script-result": payload);
                emit!("run-complete": serde_json::json!({"run_path": &run_path_clone}));
            }
            Err(e) => {
                log::error!("[run_script] 后台执行失败: {:?}", e);
                emit!("run-complete": serde_json::json!({"run_path": &run_path_clone, "error": format!("{:?}", e)}));
            }
        }
    });

    Ok(RunScriptResponse { run_path, status: "running".into() })
}

#[command]
fn read_script(path: String) -> Result<String, AppError> {
    std::fs::read_to_string(&path)
        .map_err(|e| AppError::Other(Error::from_boxed(e.into_inner().expect_log("rarely error"))))
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
    // 如果是 .py 文件，级联删除对应卷中的 .run 文件
    if path.ends_with(".py") {
        let volume = env::vfs_volume(std::path::Path::new(&path)).unwrap_or_else(|| "C".to_string());
        if let Some(name) = std::path::Path::new(&path).file_name().and_then(|n| n.to_str()) {
            let run_path = format!("(vfs)/{}/运行记录/{}.run", volume, name);
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
        let volume = env::vfs_volume(std::path::Path::new(&path)).unwrap_or_else(|| "C".to_string());
        let old_run = format!("(vfs)/{}/运行记录/{}.run", volume, old_name);
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
async fn detach_window(app: tauri::AppHandle, url_path: String, title: String) -> Result<String, AppError> {
    let label = format!("detached-{}", uuid::Uuid::new_v4().to_string().replace('-', "_"));
    // 使用 initialization_script 注入路由，比 URL 查询参数更可靠
    // 通过 JSON 序列化确保所有 JS 特殊字符被正确转义
    let init_script = format!("window.__DETACH_ROUTE__ = {};", serde_json::to_string(&url_path).unwrap());
    tauri::WebviewWindowBuilder::new(
        &app, &label, tauri::WebviewUrl::App("index.html".into())
    )
    .title(&title)
    .decorations(false)
    .initialization_script(&init_script)
    .inner_size(800.0, 600.0)
    .build()
    .map_err(|e| AppError::Other(anyhow::Error::from(e)))
    .inspect_log("创建分离窗口失败")?;
    log::info!("[detach_window] 已创建: label={}, route={}", label, url_path);
    Ok(label)
}

/// 分离窗口请求合并回主窗口（转发事件）
#[command]
fn emit_merge_request(path: String, label: String, icon: String) {
    use serde_json::json;
    emit!("merge-request": json!({ "path": path, "label": label, "icon": icon }));
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

/// 同步 B 盘（扫描 vault 目录 → 更新 DB）
#[command]
fn sync_vault() -> Result<String, AppError> {
    let pool = &vfs::get_vfs().db_pool;
    vfs::real_fs::sync_real_volume(pool, "B")
        .map(|_| "同步完成".to_string())
        .map_err(|e| AppError::Io(e))
}

/// 获取 B 盘真实文件夹路径
#[command]
fn get_vault_path() -> Result<String, AppError> {
    Ok(env_system::vault_dir().to_string_lossy().to_string())
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
    // 确保数据目录存在（避免后续写入失败）
    std::fs::create_dir_all(env::app_data_dir()).unwrap_or_else(|e| {
        eprintln!("[MAIN] 创建数据目录失败: {}", e);
    });
    std::fs::create_dir_all(env::config_dir()).unwrap_or_else(|e| {
        eprintln!("[MAIN] 创建配置目录失败: {}", e);
    });
    std::fs::create_dir_all(env::vault_dir()).unwrap_or_else(|e| {
        eprintln!("[MAIN] 创建资料目录失败: {}", e);
    });
    eprintln!("[MAIN] 数据目录: {}", env::app_data_dir().display());
    vfs::init_vfs(
        &env::database_path(),
        &[("C", 64 * 1024 * 1024), ("B", 64 * 1024 * 1024)],
    )
    .expect("VFS 初始化失败");
    eprintln!("[MAIN] VFS 初始化完成");

    // 同步 B 盘（真实文件 → DB）
    if let Err(e) = vfs::real_fs::sync_real_volume(
        &vfs::get_vfs().db_pool,
        "B",
    ) {
        eprintln!("[MAIN] B盘同步失败: {}", e);
    } else {
        eprintln!("[MAIN] B盘同步完成");
    }

    tauri::Builder::default()
        .manage(log_handle.clone())
        .plugin(tauri_plugin_titlebar::init())
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
            detach_window,
            emit_merge_request,
            vfs_info,
            vfs_list_versions,
            vfs_read_version,
            config::read_settings,
            config::write_settings,
            config::reset_settings,
            sync_vault,
            get_vault_path,
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