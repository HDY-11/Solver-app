// api/script.ts — 脚本操作统一封装

import { invoke } from '@tauri-apps/api/core';
import { error as logError, info as logInfo } from '@tauri-apps/plugin-log';

export interface RunScriptResponse {
  run_path: string;
  status: 'cached' | 'running';
}

/** 运行脚本，返回运行记录路径和状态 */
export async function runScript(path: string): Promise<RunScriptResponse> {
  logInfo(`[script] runScript: ${path}`);
  try {
    return await invoke<RunScriptResponse>('run_script', { path });
  } catch (err) {
    logError(`[script] runScript 失败 (${path}): ${err}`);
    throw err;
  }
}

/** 保存脚本到磁盘路径 */
export async function saveScript(code: string, path: string): Promise<void> {
  logInfo(`[script] saveScript: ${path} (${code.length} 字符)`);
  try {
    await invoke('save_script', { code, path });
  } catch (err) {
    logError(`[script] saveScript 失败 (${path}): ${err}`);
    throw err;
  }
}

/** 从磁盘读取脚本内容 */
export async function readScript(path: string): Promise<string> {
  logInfo(`[script] readScript: ${path}`);
  try {
    return await invoke<string>('read_script', { path });
  } catch (err) {
    logError(`[script] readScript 失败 (${path}): ${err}`);
    throw err;
  }
}
