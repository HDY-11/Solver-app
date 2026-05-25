// api/vfs.ts — VFS 操作统一封装
//
// 所有 vfs_* 命令的调用入口，统一处理：
// - 路径前缀 (vfs)/C 的拼接
// - 错误日志记录
// - 返回类型安全

import { invoke } from '@tauri-apps/api/core';
import { error as logError, info as logInfo } from '@tauri-apps/plugin-log';
import type { VfsNode, VfsInfo } from '../types';

/** VFS 路径前缀，后端 vfs_* 命令要求此格式 */
const VFS_PREFIX = '(vfs)/C';

// =========================================================================
// 目录操作
// =========================================================================

/** 列出目录下的所有子节点 */
export async function listDir(dirPath: string): Promise<VfsNode[]> {
  const fullPath = buildPath(dirPath);
  logInfo(`[vfs] listDir: ${fullPath}`);
  try {
    return await invoke<VfsNode[]>('vfs_list_dir', { path: fullPath });
  } catch (err) {
    logError(`[vfs] listDir 失败 (${fullPath}): ${err}`);
    throw err;
  }
}

/** 创建目录（自动创建父目录） */
export async function createDir(dirPath: string): Promise<void> {
  const fullPath = buildPath(dirPath);
  logInfo(`[vfs] createDir: ${fullPath}`);
  try {
    await invoke('vfs_create_dir', { path: fullPath });
  } catch (err) {
    logError(`[vfs] createDir 失败 (${fullPath}): ${err}`);
    throw err;
  }
}

// =========================================================================
// 文件操作
// =========================================================================

/** 读取文件内容 */
export async function readFile(filePath: string): Promise<string> {
  const fullPath = buildPath(filePath);
  try {
    return await invoke<string>('vfs_read', { path: fullPath });
  } catch (err) {
    logError(`[vfs] readFile 失败 (${fullPath}): ${err}`);
    throw err;
  }
}

/** 写入/创建文件，content 为空字符串时创建空文件 */
export async function writeFile(filePath: string, content: string): Promise<void> {
  const fullPath = buildPath(filePath);
  logInfo(`[vfs] writeFile: ${fullPath} (${content.length} 字符)`);
  try {
    await invoke('vfs_write', { path: fullPath, content });
  } catch (err) {
    logError(`[vfs] writeFile 失败 (${fullPath}): ${err}`);
    throw err;
  }
}

// =========================================================================
// 通用操作
// =========================================================================

/** 检查路径是否存在 */
export async function exists(filePath: string): Promise<boolean> {
  const fullPath = buildPath(filePath);
  try {
    return await invoke<boolean>('vfs_exists', { path: fullPath });
  } catch (err) {
    logError(`[vfs] exists 失败 (${fullPath}): ${err}`);
    throw err;
  }
}

/** 删除节点（软删除） */
export async function deleteNode(filePath: string): Promise<void> {
  const fullPath = buildPath(filePath);
  logInfo(`[vfs] deleteNode: ${fullPath}`);
  try {
    await invoke('vfs_delete', { path: fullPath });
  } catch (err) {
    logError(`[vfs] deleteNode 失败 (${fullPath}): ${err}`);
    throw err;
  }
}

/** 获取 VFS 状态信息 */
export async function getInfo(): Promise<VfsInfo> {
  try {
    return await invoke<VfsInfo>('vfs_info');
  } catch (err) {
    logError(`[vfs] getInfo 失败: ${err}`);
    throw err;
  }
}

// =========================================================================
// 内部工具
// =========================================================================

/**
 * 构建 VFS 完整路径。
 * 如果传入路径已包含 VFS_PREFIX，则直接返回，避免双重前缀。
 */
function buildPath(path: string): string {
  if (path.startsWith(VFS_PREFIX)) return path;
  // 去掉开头的 / 避免双斜杠
  const clean = path.replace(/^\/+/, '');
  return `${VFS_PREFIX}/${clean}`;
}
