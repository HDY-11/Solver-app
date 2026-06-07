//! 应用配置模块 — lua_permission（PowerShell 权限模型）

use env_system::AppConfig;
use serde::{Deserialize, Serialize};

/// 应用设置（与前端 `AppSettings` 接口对齐）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default = "default_font_size")]
    pub font_size: u32,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_tab_size")]
    pub tab_size: u32,
    #[serde(default = "default_auto_save")]
    pub auto_save: bool,
    /// Lua 权限等级："admin" | "user"（默认 user）
    #[serde(default = "default_lua_permission")]
    pub lua_permission: String,
}

fn default_font_size() -> u32 { 14 }
fn default_theme() -> String { "dark".into() }
fn default_tab_size() -> u32 { 4 }
fn default_auto_save() -> bool { true }
fn default_lua_permission() -> String { "user".into() }

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            font_size: default_font_size(),
            theme: default_theme(),
            tab_size: default_tab_size(),
            auto_save: default_auto_save(),
            lua_permission: default_lua_permission(),
        }
    }
}

/// 配置访问器：绑定 `settings.toml` 路径
pub struct SolverConfig;

impl AppConfig<AppSettings> for SolverConfig {
    fn config_path() -> std::path::PathBuf {
        env_system::paths::config_file_path()
    }
}

// ── Tauri 命令 ─────────────────────────────────

#[tauri::command]
pub fn read_settings() -> Result<AppSettings, String> {
    SolverConfig::read_config().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn write_settings(settings: AppSettings) -> Result<(), String> {
    SolverConfig::write_config(&settings).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reset_settings() -> Result<AppSettings, String> {
    let default = AppSettings::default();
    SolverConfig::write_config(&default).map_err(|e| e.to_string())?;
    Ok(default)
}
