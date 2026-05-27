// =========================================================================
// VFS 类型（与后端 Rust VfsNodeInfo / VfsInfo 对应）
// =========================================================================

/** 后端 vfs_list_dir 返回的节点信息 */
export interface VfsNode {
  id: number;
  name: string;
  node_type: 'file' | 'folder' | 'run';
  size: number | null;
  modified_at: string;
  /** 版本号，每次写入递增（格式 MAJOR.MINOR.PATCH） */
  version: string;
}

/** 后端 vfs_info 返回的 VFS 状态 */
export interface VfsInfo {
  c_exists: boolean;
  c_used: number;
  c_total: number;
  c_node_count: number;
}

// =========================================================================
// 脚本执行类型
// =========================================================================

/** 后端 script-result 事件的 payload */
export interface ScriptResultPayload {
  path: string;
  stdout: string;
  stderr: string;
}

/** .run 文件中存储的运行记录内容 */
export interface RunRecordContent {
  script_path: string;
  script_version: string;
  stdout: string;
  stderr: string;
}

// =========================================================================
// 工具函数
// =========================================================================

/** 格式化字节大小为可读字符串 */
export function fmtSize(bytes: number | null | undefined): string {
  if (bytes == null) return '';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

// =========================================================================
// 主题类型
// =========================================================================

export type Theme = 'light' | 'dark';

// =========================================================================
// 版本时间线
// =========================================================================

/** 后端 vfs_list_versions 返回的版本信息 */
export interface VfsVersion {
  node_id: number;
  content_hash: string;
  size: number;
  created_at: string;
}