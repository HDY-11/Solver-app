// api/events.ts — Tauri 事件监听封装

import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { error as logError } from '@tauri-apps/plugin-log';
import type { ScriptResultPayload, RunOutputPayload } from '../types';

/** 监听脚本运行结果事件 */
export async function onScriptResult(
  handler: (payload: ScriptResultPayload) => void,
): Promise<UnlistenFn> {
  try {
    return await listen<ScriptResultPayload>('script-result', (event) => {
      handler(event.payload);
    });
  } catch (err) {
    logError(`[events] 监听 script-result 失败: ${err}`);
    throw err;
  }
}

/** 监听脚本实时输出事件（sdk.print） */
export async function onRunOutput(
  handler: (payload: RunOutputPayload) => void,
): Promise<UnlistenFn> {
  try {
    return await listen<RunOutputPayload>('run-output', (event) => {
      handler(event.payload);
    });
  } catch (err) {
    logError(`[events] 监听 run-output 失败: ${err}`);
    throw err;
  }
}

/** 监听运行完成事件 */
export async function onRunComplete(
  handler: (payload: { run_path: string; error?: string }) => void,
): Promise<UnlistenFn> {
  try {
    return await listen<{ run_path: string; error?: string }>('run-complete', (event) => {
      handler(event.payload);
    });
  } catch (err) {
    logError(`[events] 监听 run-complete 失败: ${err}`);
    throw err;
  }
}
