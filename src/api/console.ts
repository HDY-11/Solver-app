// api/console.ts — .cmdv console backend commands (v3)
//
// All lua_* / mem_buffer_* IPC calls.

import { invoke } from '@tauri-apps/api/core';
import { error as logError } from '@tauri-apps/plugin-log';
import type { LuaExecResult } from '../types';

export async function luaExec(tabId: string, code: string): Promise<LuaExecResult> {
  try { return await invoke<LuaExecResult>('lua_exec', { tabId, code }); }
  catch (err) { logError(`[console] lua_exec failed (${tabId}): ${err}`); throw err; }
}

export async function luaSendInput(tabId: string, input: string): Promise<void> {
  try { await invoke('lua_send_input', { tabId, input }); }
  catch (err) { logError(`[console] lua_send_input failed (${tabId}): ${err}`); throw err; }
}

export async function luaInterrupt(tabId: string): Promise<void> {
  try { await invoke('lua_interrupt', { tabId }); }
  catch (err) { logError(`[console] lua_interrupt failed (${tabId}): ${err}`); throw err; }
}

export async function memBufferRead(tabId: string, start: number, end: number): Promise<string> {
  try { return await invoke<string>('mem_buffer_read', { tabId, start, end }); }
  catch (err) { logError(`[console] mem_buffer_read failed (${tabId}): ${err}`); throw err; }
}

export async function memBufferReadSince(tabId: string, cursor: number): Promise<{ data: string; cursor: number }> {
  try { return await invoke<{ data: string; cursor: number }>('mem_buffer_read_since', { tabId, cursor }); }
  catch (err) { logError(`[console] mem_buffer_read_since failed (${tabId}): ${err}`); throw err; }
}

export async function memBufferGetAll(tabId: string): Promise<string> {
  try { return await invoke<string>('mem_buffer_get_all', { tabId }); }
  catch (err) { logError(`[console] mem_buffer_get_all failed (${tabId}): ${err}`); throw err; }
}

export async function cmdvExport(tabId: string, format: 'html' | 'md' | 'txt'): Promise<void> {
  try { await invoke('cmdv_export', { tabId, format }); }
  catch (err) { logError(`[console] cmdv_export failed (${tabId}, ${format}): ${err}`); throw err; }
}
