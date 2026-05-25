// api/script.ts — 脚本操作统一封装
//
// 封装 save_script / run_script / read_script 三个后端命令，
// 统一错误处理与日志。

import { invoke } from '@tauri-apps/api/core';
import { error as logError, info as logInfo } from '@tauri-apps/plugin-log';

/**
 * 保存脚本到磁盘路径。
 * 与 vfs_write 不同，此函数写入的是宿主机真实文件系统。
 */
export async function saveScript(code: string, path: string): Promise<void> {
  logInfo(`[script] saveScript: ${path} (${code.length} 字符)`);
  try {
    await invoke('save_script', { code, path });
  } catch (err) {
    logError(`[script] saveScript 失败 (${path}): ${err}`);
    throw err;
  }
}

/**
 * 运行 Python 脚本，返回 stdout 字符串。
 * 运行完成后后端会通过事件 script-result 广播详细结果（含 stderr）。
 */
export async function runScript(path: string): Promise<string> {
  logInfo(`[script] runScript: ${path}`);
  try {
    return await invoke<string>('run_script', { path });
  } catch (err) {
    logError(`[script] runScript 失败 (${path}): ${err}`);
    throw err;
  }
}

/**
 * 从磁盘读取脚本内容。
 * 与 vfs_read 不同，此函数读取的是宿主机真实文件系统。
 */
export async function readScript(path: string): Promise<string> {
  logInfo(`[script] readScript: ${path}`);
  try {
    return await invoke<string>('read_script', { path });
  } catch (err) {
    logError(`[script] readScript 失败 (${path}): ${err}`);
    throw err;
  }
}
