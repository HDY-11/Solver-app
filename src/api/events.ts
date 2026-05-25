// api/events.ts — Tauri 事件监听封装
//
// 封装 listen 调用，提供类型安全的事件监听与自动清理。

import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { error as logError } from '@tauri-apps/plugin-log';
import type { ScriptResultPayload } from '../types';

/**
 * 监听脚本运行结果事件。
 * 返回 UnlistenFn 用于取消监听（通常在 useEffect cleanup 中调用）。
 */
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
