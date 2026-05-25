// renderers/PythonEditor.tsx — Python 代码编辑器（基于 Monaco）

import { useState, useEffect, useCallback, useRef } from 'react';
import { error as logError } from '@tauri-apps/plugin-log';
import Editor from '@monaco-editor/react';
import { registerRenderer } from '../registry/registry';
import { readFile, writeFile } from '../api/vfs';
import { runScript } from '../api/script';
import { useToast } from '../hooks/useToast';
import { Loading } from '../components/Loading';
import type { RendererProps } from '../registry/types';
import styles from './PythonEditor.module.css';

function PythonEditor({ nodeId }: RendererProps) {
  const { addToast } = useToast();
  // 解码 VFS 路径（Sidebar 传入时用了 encodeURIComponent）
  const vfsPath = nodeId ? decodeURIComponent(nodeId) : null;
  const [code, setCode] = useState<string>('');
  const [loading, setLoading] = useState(false);
  const [saved, setSaved] = useState(true);
  const [running, setRunning] = useState(false);
  const [output, setOutput] = useState<string | null>(null);
  const [elapsedMs, setElapsedMs] = useState<number | null>(null);
  const codeRef = useRef(code);
  const pathRef = useRef(vfsPath);
  const outputRef = useRef<HTMLDivElement>(null);
  codeRef.current = code;
  pathRef.current = vfsPath;

  // 加载文件
  useEffect(() => {
    if (!vfsPath) return;
    setLoading(true);
    readFile(vfsPath)
      .then(content => {
        setCode(content);
        setSaved(true);
      })
      .catch(err => {
        logError(`PythonEditor: 加载文件失败 (${vfsPath}): ${err}`);
        setCode(`# 加载失败: ${err}`);
      })
      .finally(() => setLoading(false));
  }, [vfsPath]);

  // 保存
  const handleSave = useCallback(async () => {
    const p = pathRef.current;
    if (!p) return;
    try {
      await writeFile(p, codeRef.current);
      setSaved(true);
      addToast('success', '已保存');
    } catch (err) {
      logError(`PythonEditor: 保存失败 (${p}): ${err}`);
      addToast('error', `保存失败: ${err}`);
    }
  }, [addToast]);

  // 运行
  const handleRun = useCallback(async () => {
    const p = pathRef.current;
    if (!p) return;
    setRunning(true);
    setOutput(null);
    setElapsedMs(null);
    const start = performance.now();
    try {
      await handleSave();
      const result = await runScript(p);
      setOutput(result);
      setElapsedMs(Math.round(performance.now() - start));
      addToast('success', '运行完成');
    } catch (err) {
      logError(`PythonEditor: 运行失败 (${p}): ${err}`);
      setOutput(`运行失败: ${err}`);
      setElapsedMs(Math.round(performance.now() - start));
      addToast('error', `运行失败: ${err}`);
    } finally {
      setRunning(false);
    }
  }, [handleSave, addToast]);

  // 快捷键 Ctrl+S / Ctrl+Enter
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 's') {
        e.preventDefault();
        handleSave();
      }
      if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
        e.preventDefault();
        handleRun();
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [handleSave, handleRun]);

  return (
    <div className={styles.container}>
      {loading ? (
        <Loading text="加载文件中..." />
      ) : (
      <>
      <div className={styles.editorArea}>
        {!saved && (
          <div className={styles.unsavedBadge}>● 未保存</div>
        )}
        <Editor
          height="100%"
          defaultLanguage="python"
          value={code}
          onChange={v => {
            setCode(v ?? '');
            setSaved(false);
          }}
          theme="vs-dark"
          options={{
            minimap: { enabled: false },
            fontSize: 14,
            fontFamily: 'var(--font-mono)',
            padding: { top: 16, bottom: 16 },
            scrollBeyondLastLine: false,
            automaticLayout: true,
            tabSize: 4,
            insertSpaces: true,
          }}
        />
        <div className={styles.fileBadge}>
          {saved ? '✓' : '●'} {(nodeId ?? '').split('/').pop()}
        </div>
      </div>

      {output !== null && (
        <div ref={outputRef} className={styles.outputPanel}>
          <div className={styles.outputHeader}>
            <span className={styles.outputStatus}>
              {running ? '⏳ 运行中...' : '📋 输出'}
              {elapsedMs !== null && !running && (
                <span className={styles.elapsed}>({elapsedMs}ms)</span>
              )}
            </span>
            <div className={styles.outputActions}>
              <button
                className={styles.outputBtn}
                onClick={() => {
                  navigator.clipboard.writeText(output).then(() => {
                    addToast('info', '已复制到剪贴板');
                  });
                }}
                title="复制输出"
              >📋</button>
              <button
                className={styles.outputBtn}
                onClick={() => setOutput(null)}
                title="关闭面板"
              >✕</button>
            </div>
          </div>
          {output.includes('运行失败') ? (
            <span className={styles.outputStderr}>{output}</span>
          ) : (
            <span>{output}</span>
          )}
        </div>
      )}
      </>
      )}
    </div>
  );
}

// ── 注册 ──────────────────────────────────────

registerRenderer({
  name: 'py',
  extensions: ['.py'],
  component: PythonEditor,
  icon: '🐍',
  label: 'Python',
  toolbar: () => <PythonToolbar />,
});

// ── 工具栏 ────────────────────────────────────

function PythonToolbar() {
  const handleSave = () => {
    window.dispatchEvent(new KeyboardEvent('keydown', {
      ctrlKey: true, key: 's', bubbles: true,
    }));
  };

  const handleRun = () => {
    window.dispatchEvent(new KeyboardEvent('keydown', {
      ctrlKey: true, key: 'Enter', bubbles: true,
    }));
  };

  // 分离窗口：通过自定义事件通知 WindowManager
  const handleDetach = () => {
    const pathParts = window.location.pathname.split('/');
    const nodeId = pathParts[pathParts.length - 1];
    window.dispatchEvent(new CustomEvent('detach-window', {
      detail: { nodeId, label: `脚本 ${nodeId}` },
    }));
  };

  return (
    <>
      <button className="btn btn-primary btn-sm" onClick={handleRun}>
        ▶ 运行
      </button>
      <button className="btn btn-sm" onClick={handleSave}>
        💾 保存
      </button>
      <button className="btn btn-sm" onClick={handleDetach} title="在新窗口打开">
        🪟 分离
      </button>
    </>
  );
}

export default PythonEditor;