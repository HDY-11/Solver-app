// api/config.ts — 应用配置读写（后端 settings.toml）
//
// 替代 localStorage，配置持久化到文件系统，重启不丢失。
// 配置变更会立即写入磁盘。

import { invoke } from '@tauri-apps/api/core';
import { error as logError } from '@tauri-apps/plugin-log';

/** 应用设置（与 Rust AppSettings 对齐） */
export interface AppSettings {
  font_size: number;
  theme: 'light' | 'dark';
  tab_size: number;
  auto_save: boolean;
}

/** 读取配置（文件不存在时自动创建默认配置） */
export async function readSettings(): Promise<AppSettings> {
  try {
    return await invoke<AppSettings>('read_settings');
  } catch (err) {
    logError(`[config] readSettings 失败: ${err}`);
    throw err;
  }
}

/** 写入配置 */
export async function writeSettings(settings: AppSettings): Promise<void> {
  try {
    await invoke('write_settings', { settings });
  } catch (err) {
    logError(`[config] writeSettings 失败: ${err}`);
    throw err;
  }
}

/** 重置为默认配置 */
export async function resetSettings(): Promise<AppSettings> {
  try {
    return await invoke<AppSettings>('reset_settings');
  } catch (err) {
    logError(`[config] resetSettings 失败: ${err}`);
    throw err;
  }
}

// ── 默认值（供组件初始化使用，与 Rust 侧保持一致）──

export const DEFAULT_SETTINGS: AppSettings = {
  font_size: 14,
  theme: 'dark',
  tab_size: 4,
  auto_save: true,
};
