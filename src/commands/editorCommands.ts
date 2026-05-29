// commands/editorCommands.ts — 编辑器命令注册（对标 VS Code editor contributions）
//
// 注册 editor.save / editor.run 等命令，这些命令通过 activeEditor
// 查找当前活跃编辑器并调用其操作。Toolbar 和快捷键都通过命令系统触发。

import { commandService, Commands } from '../services/commandService';
import { activeEditor } from '../services/activeEditor';

/** 注册编辑器核心命令（应用启动时调用一次） */
export function registerEditorCommands(): () => void {
  const unreg1 = commandService.registerCommand(Commands.EDITOR_SAVE, async () => {
    await activeEditor.active?.save();
  });

  const unreg2 = commandService.registerCommand(Commands.EDITOR_RUN, async () => {
    await activeEditor.active?.run();
  });

  // 快捷键映射（全局注册一次，不需要 per-instance 的 URL 检查）
  const unreg3 = commandService.registerKeybindings([
    { key: 'Ctrl+s', command: Commands.EDITOR_SAVE },
    { key: 'Ctrl+Enter', command: Commands.EDITOR_RUN },
  ]);

  return () => { unreg1(); unreg2(); unreg3(); };
}
