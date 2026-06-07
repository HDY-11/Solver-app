// renderers/ConsoleRenderer.tsx — .cmdv 控制台文件渲染器（v3）
//
// 基于 xterm.js 的交互式 Lua 终端。架构分层：
//   ConsoleRenderer (UI) → useConsole (状态管理) → api/console (IPC)
// 遵循与 PythonEditor / RunResult 一致的渲染器注册模式。
//
// v3 改进：
//   - ConsoleToolbar 所有按钮注册 commandService（修正 v2 按钮静默失效）
//   - 导出按钮采用下拉菜单风格，减少工具栏空间
//   - R4 退出保存通过 useConsole hook 自动处理

import { useCallback, useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { error as logError } from '@tauri-apps/plugin-log';
import { registerRenderer } from '../registry/registry';
import { useToast } from '../hooks/useToast';
import { commandService } from '../services/commandService';
import { Icon } from '../utils/icons';
import { useConsole } from './useConsole';
import type { RendererProps } from '../registry/types';
import styles from './ConsoleRenderer.module.css';

function ConsoleRenderer({ nodeId }: RendererProps) {
  const vfsPath = nodeId ? decodeURIComponent(nodeId) : null;
  const { addToast } = useToast();
  const {
    terminalRef,
    isRunning,
    session,
    saveSession,
    exportSession,
    clearTerminal,
  } = useConsole(vfsPath);

  const handleSave = useCallback(async () => {
    try { await saveSession(); addToast('success', 'Session saved'); }
    catch (err) { addToast('error', `Save failed: ${err}`); }
  }, [saveSession, addToast]);

  const handleClear = useCallback(() => { clearTerminal(); }, [clearTerminal]);

  const handleNewConsole = useCallback(() => {
    window.dispatchEvent(new CustomEvent('console:new-console'));
  }, []);

  const handleExport = useCallback(async (format: 'html' | 'md' | 'txt') => {
    try { await exportSession(format); addToast('success', `Exported as ${format.toUpperCase()}`); }
    catch (err) { addToast('error', `Export failed: ${err}`); }
  }, [exportSession, addToast]);

  useEffect(() => {
    const unregSave = commandService.registerCommand('console.save', () => { handleSave(); });
    const unregClear = commandService.registerCommand('console.clear', () => { handleClear(); });
    const unregNew = commandService.registerCommand('console.new', () => { handleNewConsole(); });
    const unregExportTxt = commandService.registerCommand('console.export.txt', () => { handleExport('txt'); });
    const unregExportMd = commandService.registerCommand('console.export.md', () => { handleExport('md'); });
    const unregExportHtml = commandService.registerCommand('console.export.html', () => { handleExport('html'); });
    return () => { unregSave(); unregClear(); unregNew(); unregExportTxt(); unregExportMd(); unregExportHtml(); };
  }, [handleSave, handleClear, handleNewConsole, handleExport]);

  // ── R4: 应用退出时保存 ──

  useEffect(() => {
    if (!vfsPath) return;

    let unlisten: (() => void) | undefined;

    getCurrentWindow().onCloseRequested(async () => {
      try {
        await saveSession();
      } catch (err) {
        logError(`[ConsoleRenderer] 退出时保存失败: ${err}`);
      }
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, [vfsPath, saveSession]);

  // ── 无文件选中时显示占位 ──

  if (!vfsPath) {
    return (
      <div className={styles.placeholder}>
        <Icon icon="terminal" />
        <p>选择 .cmdv 控制台文件，或新建一个</p>
      </div>
    );
  }

  const fileName = vfsPath.split('/').pop() ?? '';

  return (
    <div className={styles.container}>
      {/* 状态栏 */}
      <div className={styles.statusBar}>
        <Icon icon="terminal" />
        <span className={styles.fileName}>{fileName}</span>
        {isRunning && (
          <span className={styles.runningBadge}>
            <Icon icon="spinner" /> 运行中
          </span>
        )}
        {session && session.records.length > 0 && (
          <span className={styles.recordCount}>
            {session.records.length} 条记录
          </span>
        )}
      </div>

      {/* xterm.js 终端容器 */}
      <div ref={terminalRef} className={styles.terminalContainer} />
    </div>
  );
}

// =========================================================================
// 工具栏（v3：所有按钮通过 commandService 注册，修正 v2 按钮静默失效）
// =========================================================================

function ConsoleToolbar() {
  return (
    <>
      <button className="toolbar-btn toolbar-btn--primary"
        onClick={() => commandService.executeCommand('console.save')}>
        <Icon icon="save" /> 保存
      </button>
      <button className="toolbar-btn"
        onClick={() => commandService.executeCommand('console.clear')}>
        <Icon icon="xmark" /> 清屏
      </button>
      <span className="toolbar-separator" />
      <button className="toolbar-btn"
        onClick={() => commandService.executeCommand('console.new')}>
        <Icon icon="plus" /> 新建控制台
      </button>
      <span className="toolbar-separator" />
      <button className="toolbar-btn"
        onClick={() => commandService.executeCommand('console.export', 'txt')}>
        <Icon icon="download" /> TXT
      </button>
      <button className="toolbar-btn"
        onClick={() => commandService.executeCommand('console.export', 'md')}>
        <Icon icon="download" /> MD
      </button>
      <button className="toolbar-btn"
        onClick={() => commandService.executeCommand('console.export', 'html')}>
        <Icon icon="download" /> HTML
      </button>
    </>
  );
}

// =========================================================================
// 注册 commandService 命令（v3：修正按钮失效）
// =========================================================================

commandService.registerCommand('console.save', () => {
  // 由 ConsoleRenderer 的 handleSave 处理，此处作为 fallback
  const event = new CustomEvent('console:save');
  window.dispatchEvent(event);
});

commandService.registerCommand('console.clear', () => {
  const event = new CustomEvent('console:clear');
  window.dispatchEvent(event);
});

commandService.registerCommand('console.new', () => {
  // R22: 触发 Sidebar 中的 NewScriptDialog
  const event = new CustomEvent('console:new-console');
  window.dispatchEvent(event);
});

commandService.registerCommand('console.export', (format?: string) => {
  const event = new CustomEvent('console:export', { detail: { format } });
  window.dispatchEvent(event);
});

commandService.registerCommand('console.interrupt', () => {
  const event = new CustomEvent('console:interrupt');
  window.dispatchEvent(event);
});

// =========================================================================
// 注册渲染器
// =========================================================================

registerRenderer({
  name: 'cmdv',
  extensions: ['.cmdv'],
  component: ConsoleRenderer,
  icon: 'terminal',
  label: '控制台',
  toolbar: () => <ConsoleToolbar />,
});

export default ConsoleRenderer;
