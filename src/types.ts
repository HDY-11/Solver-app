// types.ts — 共享类型定义（v4：新增 CmdvSession.cliType + CommandModule 类型）
//
// 修改类型: 修改 — R9: CmdvSession 增加可选 cliType 字段

// =========================================================================
// .cmdv 控制台文件相关类型
// =========================================================================

/** .cmdv 文件中的单条会话记录 */
export interface CmdvRecord {
  /** 用户输入的代码 */
  input: string;
  /** 执行输出（stdout 合并 stderr） */
  output: string;
  /** ISO 8601 时间戳 */
  timestamp: string;
  /** 退出码（0=成功, undefined=未执行完成） */
  exitCode?: number;
}

/**
 * .cmdv 文件的完整会话结构。
 *
 * v4 新增 (R9):
 * - cliType?: 'lua' | 'python' — 可选字段，默认值为 'lua'（向前兼容）
 *   旧 .cmdv 文件不含此字段时，自动识别为 Lua 控制台。
 */
export interface CmdvSession {
  /** 历史记录列表（按时间升序） */
  records: CmdvRecord[];
  /** 会话创建时间 */
  createdAt: string;
  /**
   * CLI 后端类型（R9 新增）。
   * - 'lua': Lua VM（默认值，向前兼容）
   * - 'python': Python REPL 子进程（Phase 2）
   * 旧文件不含此字段时，读取方应默认按 'lua' 处理。
   */
  cliType?: 'lua' | 'python';
}

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

/** SDK 实时输出的单条消息 */
export interface RunOutputPayload {
  run_path: string;
  content: string;
  timestamp: string;
}

/** .run 文件中存储的运行记录内容 */
export interface RunRecordContent {
  stdout: string;
  stderr: string;
  outputs?: RunOutputPayload[];
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

// =========================================================================
// R9: CommandModule + CLI 类型（统一入口）
// =========================================================================

/** CLI 后端类型 */
export type CliType = 'lua' | 'python';

/** 执行结果（与后端 CliExecResult 对齐） */
export interface ExecResult {
  output: string;
  exitCode: number;
  isWaitingInput: boolean;
}

/** 向后兼容别名 */
export type LuaExecResult = ExecResult;
