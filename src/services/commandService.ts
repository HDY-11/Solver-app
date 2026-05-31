// services/commandService.ts — 命令系统（对标 VS Code ICommandService）
//
// 职责：集中注册和执行命令。组件通过 registerCommand 注册处理函数，
// 通过 executeCommand 触发。全局唯一实例，支持 keybinding 注册。

type CommandHandler = (...args: unknown[]) => void | Promise<void>;
type UnregisterFn = () => void;

interface Keybinding {
  key: string;       // 如 'Ctrl+s', 'Ctrl+Enter'
  command: string;   // 如 'editor.save'
  when?: () => boolean; // 可选前置条件
}

class CommandService {
  private handlers = new Map<string, CommandHandler>();
  private keybindings: Keybinding[] = [];
  private keydownHandler: ((e: KeyboardEvent) => void) | null = null;

  /** 注册命令处理函数，返回取消注册函数 */
  registerCommand(id: string, handler: CommandHandler): UnregisterFn {
    this.handlers.set(id, handler);
    return () => { this.handlers.delete(id); };
  }

  /** 执行命令 */
  executeCommand(id: string, ...args: unknown[]): void {
    const handler = this.handlers.get(id);
    if (handler) {
      handler(...args);
    } else {
      console.warn(`[CommandService] 未注册的命令: ${id}`);
    }
  }

  /** 检查命令是否已注册 */
  hasCommand(id: string): boolean {
    return this.handlers.has(id);
  }

  /** 注册快捷键映射（全局键盘监听） */
  registerKeybinding(keybinding: Keybinding): UnregisterFn {
    this.keybindings.push(keybinding);
    this.ensureKeyListener();
    return () => {
      const idx = this.keybindings.indexOf(keybinding);
      if (idx >= 0) this.keybindings.splice(idx, 1);
    };
  }

  /** 批量注册快捷键 */
  registerKeybindings(bindings: Keybinding[]): UnregisterFn {
    bindings.forEach(b => this.keybindings.push(b));
    this.ensureKeyListener();
    return () => {
      bindings.forEach(b => {
        const idx = this.keybindings.indexOf(b);
        if (idx >= 0) this.keybindings.splice(idx, 1);
      });
    };
  }

  private ensureKeyListener() {
    if (this.keydownHandler) return;
    this.keydownHandler = (e: KeyboardEvent) => {
      const chord = this.eventToChord(e);
      for (const kb of this.keybindings) {
        if (kb.key === chord) {
          if (kb.when && !kb.when()) continue;
          e.preventDefault();
          this.executeCommand(kb.command);
          return;
        }
      }
    };
    window.addEventListener('keydown', this.keydownHandler);
  }

  private eventToChord(e: KeyboardEvent): string {
    const parts: string[] = [];
    if (e.ctrlKey || e.metaKey) parts.push('Ctrl');
    if (e.altKey) parts.push('Alt');
    if (e.shiftKey) parts.push('Shift');
    if (e.key === 'Enter') parts.push('Enter');
    else if (e.key === 's' || e.key === 'S') parts.push('s');
    else parts.push(e.key);
    return parts.join('+');
  }
}

/** 全局命令服务单例 */
export const commandService = new CommandService();

// ── 内置命令 ID 常量 ──
export const Commands = {
  EDITOR_SAVE: 'editor.save',
  EDITOR_RUN: 'editor.run',
  EDITOR_FIND: 'editor.find',
} as const;
