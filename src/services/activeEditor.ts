// services/activeEditor.ts — 活跃编辑器引用（对标 VS Code IEditorService.activeEditor）
//
// 维护当前活跃编辑器的操作句柄。PythonEditor 挂载时设置，卸载时清除。
// 命令系统通过此引用找到当前应操作的编辑器实例，无需按 URL 过滤事件。

interface ActiveEditorOps {
  save: () => Promise<void>;
  run: () => Promise<void>;
  /** 触发编辑器内查找 */
  find?: () => void;
  /** 编辑器的 VFS 路径，用于判断是否当前活跃 */
  vfsPath: string | null;
}

type OpsChangeCallback = (ops: ActiveEditorOps | null) => void;

class ActiveEditorRegistry {
  private current: ActiveEditorOps | null = null;
  private listeners = new Set<OpsChangeCallback>();

  /** 编辑器挂载时调用，注册自身为活跃编辑器 */
  setActive(ops: ActiveEditorOps): () => void {
    this.current = ops;
    this.notify();
    return () => {
      if (this.current?.vfsPath === ops.vfsPath) {
        this.current = null;
        this.notify();
      }
    };
  }

  /** 获取当前活跃编辑器操作句柄 */
  get active(): ActiveEditorOps | null {
    return this.current;
  }

  /** 订阅活跃编辑器变化 */
  onChange(cb: OpsChangeCallback): () => void {
    this.listeners.add(cb);
    return () => { this.listeners.delete(cb); };
  }

  private notify() {
    this.listeners.forEach(cb => cb(this.current));
  }
}

export const activeEditor = new ActiveEditorRegistry();
