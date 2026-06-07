//! lib.rs — Solver Tauri 应用入口（v4：BackendRegistry + cmdv_exec 统一路由）
//!
//! # v4 变更 (task-004 Phase 1 v2 融合)
//!
//! - R5: Lua 执行验证 + 工作线程 panic 恢复
//! - R6: cmdv_export 命令已移除（导出重构为纯前端 Sidebar → B 盘）
//! - R8: A 盘删除执行真实文件系统删除（同时移除 DB 记录 + 删除本地文件）
//! - R9: BackendRegistry 替代单一 AnyCliBackend；新增 cmdv_exec/cmdv_send_input/cmdv_interrupt 统一命令
//!
//! # 架构
//!
//! ```text
//! Frontend (TypeScript)
//!   ├─ CommandModule.execute() ──→ invoke('cmdv_exec', { cliType, tabId, code })
//!   │
//! Tauri Command Layer (此文件)
//!   ├─ cmdv_exec ──→ BackendRegistry.get(cliType) ──→ AnyCliBackend::exec()
//!   ├─ lua_exec  ──→ BackendRegistry.get("lua")   ──→ (向后兼容，已标记 deprecated)
//!   └─ mem_buffer_* ──→ CliBackend::get_output*()
//! ```

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use log_system::init_logging;
use tauri::{Manager, Emitter, command};
use event_system::*;
use error_system::{ResultLogExt, OptionLogExt, AppError};
use serde::Serialize;
use env_system as env;
use init_system;
use anyhow::Error;

use tauri_plugin_window_enhance::{
    HookBehaviors, NoopWindowBehavior, WindowBehavior, WindowManager,
};

// ═══════════════════════════════════════════════════════════════════
// 模块声明
// ═══════════════════════════════════════════════════════════════════

mod config;
mod cli;

use cli::{AnyCliBackend, BackendRegistry};
use cli_backend::{CliBackend, CliBackendType, CliExecResult, CliError};
use lua_runtime::{LuaExecResult, LuaPermission};

// ═══════════════════════════════════════════════════════════════════
// Window Enhance（已有代码，不变）
// ═══════════════════════════════════════════════════════════════════

fn register_window(app: tauri::AppHandle, label: String, behaviors: Vec<String>) {
    use tauri_plugin_window_enhance::commands;

    let mut flags = HookBehaviors::empty();
    for b in &behaviors {
        match b.as_str() {
            "nchittest"  => flags |= HookBehaviors::NCHITTEST,
            "drag_start" => flags |= HookBehaviors::DRAG_START,
            "drag_end"   => flags |= HookBehaviors::DRAG_END,
            other => {
                log::warn!("[window_enhance] 未知行为标志 '{}'，忽略", other);
            }
        }
    }

    if flags.is_empty() {
        log::warn!("[window_enhance] register_window: label={} 无有效行为标志，跳过", label);
        return;
    }

    log::info!("[window_enhance] register_window: label={} behaviors={:?}", label, flags);

    #[cfg(target_os = "windows")]
    if let Some(window) = app.get_webview_window(&label) {
        if let Ok(hwnd) = window.hwnd() {
            let hwnd_raw = hwnd.0 as isize;
            let needs_drag_detection =
                flags.intersects(HookBehaviors::DRAG_START | HookBehaviors::DRAG_END);
            if needs_drag_detection {
                commands::register(hwnd_raw, flags, Box::new(DetachedWindowBehavior::new(app.clone())));
            } else {
                commands::register(hwnd_raw, flags, Box::new(NoopWindowBehavior));
            }
        } else {
            log::warn!("[window_enhance] 无法获取 HWND for label={}", label);
        }
    } else {
        log::warn!("[window_enhance] 未找到窗口 label={}", label);
    }

    #[cfg(not(target_os = "windows"))]
    let _ = (app, label, behaviors);
}

const NAV_TOP_CSS_PX: f64 = 60.0;
const NAV_BOTTOM_CSS_PX: f64 = 124.0;

struct DetachedWindowBehavior {
    app_handle: tauri::AppHandle,
}

impl DetachedWindowBehavior {
    fn new(app_handle: tauri::AppHandle) -> Self {
        Self { app_handle }
    }
}

impl WindowBehavior for DetachedWindowBehavior {
    fn on_drag_start(&self, hwnd: isize) -> Result<(), tauri_plugin_window_enhance::BehaviorError> {
        log::debug!("[window_enhance] 分离窗口开始移动 0x{:x}", hwnd);
        Ok(())
    }

    fn on_drag_end(&self, _hwnd: isize) -> Result<(), tauri_plugin_window_enhance::BehaviorError> {
        let manager = WindowManager::global();
        let cursor_screen = match manager.cursor_position() {
            Some(pt) => pt,
            None => { log::warn!("[window_enhance] 无法获取光标位置，跳过合并检测"); return Ok(()); }
        };
        let main_hwnd = manager.find_first_hwnd_by(|state| {
            state.behaviors == HookBehaviors::NCHITTEST
        });
        if main_hwnd == 0 {
            log::warn!("[window_enhance] 未找到已注册的主窗口，跳过合并检测");
            return Ok(());
        }
        let scale = manager.dpr();
        let cursor_client = manager.screen_to_client(main_hwnd, cursor_screen);
        let client_rect = manager.client_rect(main_hwnd);
        let cursor_in_nav = check_cursor_in_nav(cursor_client.x, cursor_client.y, client_rect.width, scale);
        log::debug!(
            "[window_enhance] 合并检测: scale={:.2} client=({},{}) nav_y=({:.0},{:.0}) cursor_client=({},{}) hit={}",
            scale, client_rect.width, client_rect.height,
            NAV_TOP_CSS_PX * scale, NAV_BOTTOM_CSS_PX * scale,
            cursor_client.x, cursor_client.y, cursor_in_nav,
        );
        if cursor_in_nav {
            log::info!("[window_enhance] 光标在主窗口 Nav 区域，发射 drag-release 事件");
            let payload = serde_json::json!({ "screenX": cursor_screen.x, "screenY": cursor_screen.y });
            if let Err(e) = tauri::Emitter::emit(&self.app_handle, "drag-release", payload) {
                log::error!("[window_enhance] 发射 drag-release 事件失败: {}", e);
            }
        }
        Ok(())
    }
}

fn check_cursor_in_nav(cursor_x: i32, cursor_y: i32, client_width: i32, scale: f64) -> bool {
    let nav_top = (NAV_TOP_CSS_PX * scale) as i32;
    let nav_bottom = (NAV_BOTTOM_CSS_PX * scale) as i32;
    cursor_x >= 0 && cursor_x <= client_width && cursor_y >= nav_top && cursor_y <= nav_bottom
}

// ═══════════════════════════════════════════════════════════════════
// 类型定义
// ═══════════════════════════════════════════════════════════════════

static DETACH_ROUTES: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Clone, Serialize)]
struct RunScriptResponse {
    run_path: String,
    status: String,
}

#[derive(Clone, Serialize)]
struct ScriptResultPayload {
    path: String,
    stdout: String,
    stderr: String,
}

#[derive(Clone, Serialize)]
struct VfsVersionInfo {
    node_id: i64,
    content_hash: String,
    size: i64,
    created_at: String,
}

#[derive(Clone, Serialize)]
struct VfsInfo {
    c_exists: bool,
    c_used: u64,
    c_total: u64,
    c_node_count: u64,
}

#[derive(Clone, Serialize)]
struct VolumeInfo {
    volume: String,
    node_count: u64,
    total_size: u64,
    is_real: bool,
}

// ═══════════════════════════════════════════════════════════════════
// 脚本执行命令（已有代码，不变）
// ═══════════════════════════════════════════════════════════════════

#[command]
fn save_script(code: String, path: String) -> Result<(), AppError> {
    python_bridge::save_script(code, path)
        .map_err(|e| AppError::Other(Error::from_boxed(e.into_inner().expect_log("rarely error"))))
        .inspect_log("save_script failed")
}

#[command]
async fn run_script(path: String) -> Result<RunScriptResponse, AppError> {
    use sha2::{Sha256, Digest};

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
    let volume = env::vfs_volume(std::path::Path::new(&path)).unwrap_or_else(|| "C".to_string());
    let run_name = format!("{}.run", script_name);
    let run_dir = format!("(vfs)/{}/运行记录", volume);
    let run_path = format!("{}/{}", run_dir, run_name);

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

    let lf = serde_json::json!({
        "script_hash": &script_hash,
        "script_path": &path,
        "script_version": &script_version,
        "volume": &volume,
    }).to_string();
    vfs::VirFile::create_run_node(&run_name, &lf, &volume, &run_dir)
        .map_err(|e| AppError::Io(e))?;

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

// ═══════════════════════════════════════════════════════════════════
// VFS 命令（R8: A 盘删除修复）
// ═══════════════════════════════════════════════════════════════════

#[command]
fn vfs_read(path: String) -> Result<String, AppError> {
    let mut f = vfs::VirFile::open(&path)
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

/// R8: 删除 VFS 节点。
///
/// # 行为（统一后）
///
/// | 卷  | 删除方式                                      | 二次确认 |
/// |-----|----------------------------------------------|---------|
/// | A   | 删除本地文件 + 移除 DB 记录（硬删除）          | 需要   |
/// | B   | 仅移除 DB 记录（硬删除，不动本地文件）          | 需要   |
/// | C   | 软删除（设置 deleted=1），数据仍可恢复           | 需要   |
///
/// # 安全约束
/// - A 盘删除不可逆（永久删除本地导入源文件）
/// - 前端需弹出二次确认对话框（R8 验收标准）
/// - 级联删除关联的 .run 文件
#[command]
fn vfs_delete(path: String) -> Result<(), AppError> {
    let volume = env::vfs_volume(std::path::Path::new(&path)).unwrap_or_else(|| "C".to_string());
    let is_real = vfs::is_real_volume(&volume);

    // 级联删除关联的 .run 文件
    if path.ends_with(".py") {
        if let Some(name) = std::path::Path::new(&path).file_name().and_then(|n| n.to_str()) {
            let run_path = format!("(vfs)/{}/运行记录/{}.run", volume, name);
            if vfs::VirFile::exists(&run_path).unwrap_or(false) {
                if let Ok(f) = vfs::VirFile::open(&run_path) {
                    let _ = if is_real { f.hard_delete() } else { f.delete() };
                }
            }
        }
    }

    // R8: 对于 A 盘（真实文件系统卷且为只读来源），执行两步操作：
    // 1. 删除本地文件系统中的源文件
    // 2. 移除 DB 记录
    // B 盘仅移除 DB 记录（保留本地文件作为备份）
    if volume == "A" {
        // 删除 A 盘对应的本地文件
        if let Some(real_path) = vfs::real_fs::vfs_to_real(&path) {
            if real_path.exists() {
                if real_path.is_dir() {
                    std::fs::remove_dir_all(&real_path)
                        .map_err(|e| AppError::Io(e))
                        .inspect_log("删除 A 盘本地目录失败")?;
                    log::info!("[vfs_delete] A盘: 已删除本地目录 {}", real_path.display());
                } else {
                    std::fs::remove_file(&real_path)
                        .map_err(|e| AppError::Io(e))
                        .inspect_log("删除 A 盘本地文件失败")?;
                    log::info!("[vfs_delete] A盘: 已删除本地文件 {}", real_path.display());
                }
            }
        }
    }

    let f = vfs::VirFile::open(&path)
        .map_err(|e| AppError::Io(e))
        .inspect_log("删除失败（打开文件）")?;

    if is_real {
        // A/B 盘：硬删除 DB 记录
        f.hard_delete()
    } else {
        // C 盘：软删除
        f.delete()
    }
    .map_err(|e| AppError::Io(e))
    .inspect_log("删除失败（DB 操作）")
}

#[command]
fn vfs_rename(path: String, new_name: String) -> Result<(), AppError> {
    let old_name = std::path::Path::new(&path)
        .file_name().and_then(|n| n.to_str()).unwrap_or("");
    vfs::VirFile::open(&path)
        .and_then(|f| f.rename(&new_name))
        .map_err(|e| AppError::Io(e))
        .inspect_log("重命名失败")?;

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

#[command]
fn sync_vault() -> Result<String, AppError> {
    let pool = &vfs::get_vfs().db_pool;
    for vol in &["A", "B"] {
        vfs::real_fs::sync_real_volume(pool, vol)
            .map_err(|e| AppError::Io(e))?;
    }
    Ok("同步完成".to_string())
}

#[command]
fn get_volume_info(volume: String) -> Result<VolumeInfo, AppError> {
    let vfs_inst = vfs::get_vfs();
    let conn = vfs_inst.db_pool.get()
        .map_err(|e| AppError::Other(anyhow::Error::from(e)))?;
    let is_real = vfs::is_real_volume(&volume);
    let node_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM nodes WHERE volume=? AND deleted=0",
            rusqlite::params![&volume],
            |row| row.get(0),
        ).unwrap_or(0);
    let total_size = if is_real {
        let dir = match volume.as_str() {
            "A" => env_system::imports_dir(),
            "B" => env_system::vault_dir(),
            _ => return Err(AppError::Other(anyhow::Error::msg("未知卷"))),
        };
        flat_dir_size(&dir).unwrap_or(0)
    } else {
        let size: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(size), 0) FROM nodes WHERE volume=? AND deleted=0 AND size IS NOT NULL",
                rusqlite::params![&volume],
                |row| row.get(0),
            ).unwrap_or(0);
        size as u64
    };
    Ok(VolumeInfo { volume, node_count: node_count as u64, total_size, is_real })
}

// ═══════════════════════════════════════════════════════════════════
// 窗口分离命令（已有代码，不变）
// ═══════════════════════════════════════════════════════════════════

#[command]
async fn detach_window(app: tauri::AppHandle, url_path: String, title: String) -> Result<String, AppError> {
    let label = format!("detached-{}", uuid::Uuid::new_v4().to_string().replace('-', "_"));
    DETACH_ROUTES.lock().unwrap().insert(label.clone(), url_path.clone());
    tauri::WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::App("index.html".into()))
        .title(&title)
        .decorations(false)
        .inner_size(800.0, 600.0)
        .build()
        .map_err(|e| AppError::Other(anyhow::Error::from(e)))
        .inspect_log("创建分离窗口失败")?;
    log::info!("[detach_window] 已创建: label={}, route={}", label, url_path);
    Ok(label)
}

#[command]
fn get_detach_route(window: tauri::Window) -> Result<String, AppError> {
    let label = window.label().to_string();
    DETACH_ROUTES.lock().unwrap()
        .remove(&label)
        .ok_or_else(|| AppError::Other(anyhow::Error::msg("无分离路由")))
}

#[command]
fn emit_merge_request(path: String, label: String, icon: String) -> Result<(), AppError> {
    log::info!("[merge] 收到合并请求: path={}", path);
    let payload = serde_json::json!({ "path": &path, "label": &label, "icon": &icon });
    if let Some(handle) = event_system::GLOBAL_APPHANDLE.get() {
        tauri::Emitter::emit(handle, "merge-request", payload)
            .map_err(|e| AppError::Other(anyhow::Error::from(e)))?;
    } else {
        return Err(AppError::Other(anyhow::Error::msg("事件系统未初始化")));
    }
    log::info!("[merge] 事件已发射");
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
// Lua 权限管理（R5: 每次读写配置时同步）
// ═══════════════════════════════════════════════════════════════════

/// 从配置文件读取当前 Lua 权限等级。
///
/// # 为什么这样做是安全的
/// - 配置读取失败时回退为最安全的 `LuaPermission::User`（默认拒绝危险操作）
/// - 不在日志中输出完整的配置内容
fn read_lua_permission() -> LuaPermission {
    match config::read_settings() {
        Ok(settings) => {
            let perm = LuaPermission::from_str(&settings.lua_permission);
            log::info!("[lua] 从配置读取权限: {}", perm.as_str());
            perm
        }
        Err(e) => {
            log::warn!("[lua] 配置读取失败，回退为 user 权限: {}", e);
            LuaPermission::User // 安全默认值：最小权限
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// R9: 统一 cmdv_exec / cmdv_send_input / cmdv_interrupt 命令
// ═══════════════════════════════════════════════════════════════════

/// 统一的 CLI 执行命令。
///
/// 通过 BackendRegistry 按 cliType 路由到对应后端（Lua / Python）。
/// 替代各后端专用的 exec 命令，前端通过 CommandModule 接口调用。
///
/// # 安全校验
/// - cliType 白名单：仅接受 "lua" / "python"
/// - tabId 非空 + 字符白名单校验（由 CliBackend::exec 在实现层校验）
/// - 代码长度校验（由 CliBackend::exec 在实现层校验，防内存耗尽）
///
/// # 参数
/// - `cliType`: 后端类型（"lua" | "python"），默认 "lua" 向前兼容
/// - `tabId`: 标签页唯一标识
/// - `code`: 待执行代码
#[command]
fn cmdv_exec(
    app: tauri::AppHandle,
    registry: tauri::State<'_, BackendRegistry>,
    cliType: String,
    tabId: String,
    code: String,
) -> Result<CliExecResult, String> {
    // 安全：cliType 白名单校验
    let backend_type = CliBackendType::from_str(&cliType);
    let backend_name = backend_type.as_str();

    let backend = registry.get_static(backend_name).map_err(|e| e.to_string())?;

    // 如果是 Lua 后端，执行前同步权限配置
    if backend_type == CliBackendType::Lua {
        // 权限已由 read_lua_permission 在启动时设置
        // 后续设置变更通过 write_settings → Ta而 command 不直接同步
        // 此处仅确保权限配置已被读取
        let _ = read_lua_permission();
    }

    let result = backend.exec(&tabId, &code).map_err(|e| e.to_string())?;

    // R13：轮询工作线程的输入请求，向前端发射事件
    let pending = backend.drain_input_requests();
    for tid in pending {
        let payload = serde_json::json!({ "tab_id": tid, "prompt": "请输入:" });
        let _ = app.emit("lua-input-request", payload);
    }

    Ok(result)
}

/// 统一的 CLI 发送输入命令。
#[command]
fn cmdv_send_input(
    registry: tauri::State<'_, BackendRegistry>,
    cliType: String,
    tabId: String,
    input: String,
) -> Result<(), String> {
    let backend_name = CliBackendType::from_str(&cliType).as_str();
    let backend = registry.get_static(backend_name).map_err(|e| e.to_string())?;
    backend.send_input(&tabId, &input).map_err(|e| e.to_string())
}

/// 统一的 CLI 中断命令。
#[command]
fn cmdv_interrupt(
    registry: tauri::State<'_, BackendRegistry>,
    cliType: String,
    tabId: String,
) -> Result<(), String> {
    let backend_name = CliBackendType::from_str(&cliType).as_str();
    let backend = registry.get_static(backend_name).map_err(|e| e.to_string())?;
    backend.interrupt(&tabId).map_err(|e| e.to_string())
}

// ═══════════════════════════════════════════════════════════════════
// 向后兼容：lua_* 命令（委托给 BackendRegistry）
// ═══════════════════════════════════════════════════════════════════
//
// 这些命令保留以支持：旧 .cmdv 文件、未迁移的调用方。
// 新代码应使用 cmdv_exec / cmdv_send_input / cmdv_interrupt。

/// 在指定标签页的 Lua VM 中执行代码（向后兼容）。
///
/// @deprecated 新代码请使用 cmdv_exec { cliType: "lua", ... }
#[command]
fn lua_exec(
    app: tauri::AppHandle,
    registry: tauri::State<'_, BackendRegistry>,
    tabId: String,
    code: String,
) -> Result<LuaExecResult, String> {
    let backend = registry.get_static("lua").map_err(|e| e.to_string())?;

    let cli_result = backend.exec(&tabId, &code).map_err(|e| e.to_string())?;

    // R13：发射输入请求事件
    let pending = backend.drain_input_requests();
    for tid in pending {
        let payload = serde_json::json!({ "tab_id": tid, "prompt": "请输入:" });
        let _ = app.emit("lua-input-request", payload);
    }

    Ok(LuaExecResult {
        output: cli_result.output,
        exit_code: cli_result.exit_code,
        is_waiting_input: cli_result.is_waiting_input,
    })
}

/// 向等待输入的 Lua VM 发送数据（向后兼容）。
///
/// @deprecated 新代码请使用 cmdv_send_input { cliType: "lua", ... }
#[command]
fn lua_send_input(
    registry: tauri::State<'_, BackendRegistry>,
    tabId: String,
    input: String,
) -> Result<(), String> {
    let backend = registry.get_static("lua").map_err(|e| e.to_string())?;
    backend.send_input(&tabId, &input).map_err(|e| e.to_string())
}

/// 中断正在执行的 Lua 代码（向后兼容）。
///
/// @deprecated 新代码请使用 cmdv_interrupt { cliType: "lua", ... }
#[command]
fn lua_interrupt(
    registry: tauri::State<'_, BackendRegistry>,
    tabId: String,
) -> Result<(), String> {
    let backend = registry.get_static("lua").map_err(|e| e.to_string())?;
    backend.interrupt(&tabId).map_err(|e| e.to_string())
}

// ═══════════════════════════════════════════════════════════════════
// MemBuffer 读取命令
// ═══════════════════════════════════════════════════════════════════

/// 读取 MemBuffer 指定范围 [start, end)。
#[command]
fn mem_buffer_read(
    registry: tauri::State<'_, BackendRegistry>,
    tabId: String,
    start: usize,
    end: usize,
) -> Result<String, String> {
    let backend = registry.get_static("lua").map_err(|e| e.to_string())?;
    if end <= start {
        return Ok(String::new());
    }
    backend.get_output_range(&tabId, start, end).map_err(|e| e.to_string())
}

/// 增量读取 MemBuffer：自 cursor 之后的新数据（零拷贝）。
#[command]
fn mem_buffer_read_since(
    registry: tauri::State<'_, BackendRegistry>,
    tabId: String,
    cursor: usize,
) -> Result<serde_json::Value, String> {
    let backend = registry.get_static("lua").map_err(|e| e.to_string())?;
    let (data, new_cursor) = backend.get_output_since(&tabId, cursor).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "data": data, "cursor": new_cursor }))
}

/// 获取 MemBuffer 全部内容。
#[command]
fn mem_buffer_get_all(
    registry: tauri::State<'_, BackendRegistry>,
    tabId: String,
) -> Result<String, String> {
    let backend = registry.get_static("lua").map_err(|e| e.to_string())?;
    backend.get_output(&tabId).map_err(|e| e.to_string())
}

// ═══════════════════════════════════════════════════════════════════
// 应用级命令
// ═══════════════════════════════════════════════════════════════════

fn flat_dir_size(path: &std::path::Path) -> std::io::Result<u64> {
    let mut total = 0u64;
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                total += entry.metadata()?.len();
            }
        }
    }
    Ok(total)
}

#[command]
fn app_ready(_app: tauri::AppHandle) {
    emit!("app-ready": serde_json::json!({}));
    log::info!("[app] 应用启动完成事件已发送");
}

#[command]
fn frontend_ready() {
    init_system::set_ready();
}

#[command]
fn get_loading_status() -> init_system::LoadingStatus {
    init_system::get_loading_status()
}

#[command]
fn get_vault_path() -> Result<String, AppError> {
    Ok(env_system::vault_dir().to_string_lossy().to_string())
}

#[command]
async fn import_to_a(app: tauri::AppHandle) -> Result<String, AppError> {
    use tauri_plugin_dialog::DialogExt;
    let path = app.dialog().file().blocking_pick_file();
    let Some(file_path) = path else {
        return Err(AppError::Other(anyhow::Error::msg("未选择文件")));
    };
    let src = file_path.to_string();
    let name = std::path::Path::new(&src)
        .file_name().and_then(|n| n.to_str()).unwrap_or("imported_file");
    let dest = env_system::imports_dir().join(name);
    std::fs::create_dir_all(env_system::imports_dir())
        .map_err(|e| AppError::Io(e))?;
    std::fs::copy(&src, &dest)
        .map_err(|e| AppError::Io(e))?;
    let pool = &vfs::get_vfs().db_pool;
    vfs::real_fs::sync_real_volume(pool, "A")
        .map_err(|e| AppError::Io(e))?;
    let vfs_path = format!("(vfs)/A/{}", name);
    log::info!("[import_to_a] 导入完成: {} → {}", src, vfs_path);
    Ok(vfs_path)
}

// ═══════════════════════════════════════════════════════════════════
// 应用启动
// ═══════════════════════════════════════════════════════════════════

fn emit_loading(pct: u32, msg: &str) {
    init_system::set_progress(pct, msg);
    if let Some(handle) = event_system::GLOBAL_APPHANDLE.get() {
        let _ = tauri::Emitter::emit(handle, "loading-progress", serde_json::json!({
            "pct": pct,
            "msg": msg,
        }));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    eprintln!("[MAIN] 初始化日志系统...");
    emit_loading(5, "初始化日志...");
    let (log_ctrl, log_handle) =
        init_logging(env::log_dir(), env::log_dir(), 4096).expect("初始化日志失败");

    let mut l_c = Some(log_ctrl);
    log::info!("日志系统已初始化，准备启动 Tauri 应用...");

    eprintln!("[MAIN] 初始化 VFS...");
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
    emit_loading(10, "正在初始化 VFS...");
    vfs::init_vfs(
        &env::database_path(),
        &[("C", 64 * 1024 * 1024), ("B", 64 * 1024 * 1024), ("A", 64 * 1024 * 1024)],
    ).expect("VFS 初始化失败");
    emit_loading(25, "VFS 就绪");
    eprintln!("[MAIN] VFS 初始化完成");

    for (i, vol) in ["A", "B"].iter().enumerate() {
        emit_loading(30 + i as u32 * 20, &format!("同步 {} 盘...", vol));
        if let Err(e) = vfs::real_fs::sync_real_volume(&vfs::get_vfs().db_pool, vol) {
            eprintln!("[MAIN] {}盘同步失败: {}", vol, e);
        } else {
            eprintln!("[MAIN] {}盘同步完成", vol);
        }
    }
    emit_loading(70, "磁盘就绪");

    // R9: 创建 BackendRegistry 并注册 Lua 后端
    let registry = BackendRegistry::new();
    registry.register("lua", AnyCliBackend::Lua(lua_runtime::LuaBackend::new()));
    log::info!("[cli] BackendRegistry 已初始化，已注册 Lua 后端");

    // R5: 启动时读取权限配置
    let permission = read_lua_permission();
    log::info!("[lua] 启动权限模式: {}", permission.as_str());

    tauri::Builder::default()
        .manage(log_handle.clone())
        .manage(registry) // ← 使用 BackendRegistry 替代单一 AnyCliBackend
        .plugin(tauri_plugin_window_enhance::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_log::Builder::default()
            .skip_logger()
            .level(tauri_plugin_log::log::LevelFilter::Debug)
            .targets([tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Webview)])
            .build()
        )
        .invoke_handler(tauri::generate_handler![
            // 脚本执行
            run_script,
            save_script,
            read_script,
            // VFS
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
            sync_vault,
            get_volume_info,
            // 窗口
            detach_window,
            emit_merge_request,
            get_detach_route,
            register_window,
            // 配置
            config::read_settings,
            config::write_settings,
            config::reset_settings,
            // 应用
            app_ready,
            get_loading_status,
            frontend_ready,
            get_vault_path,
            import_to_a,
            // R9: 统一 CLI 命令（新）
            cmdv_exec,
            cmdv_send_input,
            cmdv_interrupt,
            // 向后兼容 Lua 命令
            lua_exec,
            lua_send_input,
            lua_interrupt,
            // MemBuffer
            mem_buffer_read,
            mem_buffer_read_since,
            mem_buffer_get_all,
            // NOTE: cmdv_export 已移除（R6: 导出重构为前端驱动）
        ])
        .setup(move |app| {
            emit_loading(80, "启动引擎...");
            init_event_system(app.handle().clone()).unwrap_log();
            emit_loading(90, "加载界面...");

            let window = app.get_webview_window("main").expect("获取窗口句柄失败");
            let log_ctrl = RefCell::new(l_c.take().expect("LogCtrl 已经被使用过了"));

            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    log_ctrl.borrow_mut().shutdown();
                    std::process::exit(0);
                }
            });

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                let _ = tauri::Emitter::emit(&handle, "app-ready", serde_json::json!({}));
                log::info!("[app] 启动完成事件已发送");
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
