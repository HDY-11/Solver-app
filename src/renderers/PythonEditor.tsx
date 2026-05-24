// renderers/PythonEditor.tsx

import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import Editor from '@monaco-editor/react';
import { registerRenderer } from '../registry/registry';
import type { RendererProps } from '../registry/types';

function PythonEditor({ nodeId }: RendererProps) {
  const [code, setCode] = useState<string>('');
  const [saved, setSaved] = useState(true);
  const [running, setRunning] = useState(false);
  const [output, setOutput] = useState<string | null>(null);
  const codeRef = useRef(code);
  const nodeIdRef = useRef(nodeId);
  codeRef.current = code;
  nodeIdRef.current = nodeId;

  // 加载文件
  useEffect(() => {
    if (!nodeId) return;
    invoke<string>('vfs_read', { path: `(vfs)/C/${nodeId}` })
      .then(content => {
        setCode(content);
        setSaved(true);
      })
      .catch(err => setCode(`# 加载失败: ${err}`));
  }, [nodeId]);

  // 保存
  const handleSave = useCallback(async () => {
    const id = nodeIdRef.current;
    if (!id) return;
    try {
      await invoke('vfs_write', {
        path: `(vfs)/C/${id}`,
        content: codeRef.current,
      });
      setSaved(true);
    } catch (err) {
      console.error('保存失败:', err);
    }
  }, []);

  // 运行
  const handleRun = useCallback(async () => {
    const id = nodeIdRef.current;
    if (!id) return;
    setRunning(true);
    setOutput(null);
    try {
      await handleSave();
      const result = await invoke<string>('run_script', {
        path: `(vfs)/C/${id}`,
      });
      setOutput(result);
    } catch (err) {
      setOutput(`运行失败: ${err}`);
    } finally {
      setRunning(false);
    }
  }, [handleSave]);

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
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      {/* 编辑器 */}
      <div style={{ flex: 1 }}>
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
      </div>

      {/* 输出面板 */}
      {output !== null && (
        <div style={{
          height: 200,
          background: '#1e1e1e',
          color: '#d4d4d4',
          borderTop: '1px solid var(--gray-300)',
          padding: 12,
          fontFamily: 'var(--font-mono)',
          fontSize: 13,
          overflow: 'auto',
          whiteSpace: 'pre-wrap',
        }}>
          <div style={{
            display: 'flex',
            justifyContent: 'space-between',
            marginBottom: 8,
            fontSize: 12,
            color: '#888',
          }}>
            <span>{running ? '⏳ 运行中...' : '📋 输出'}</span>
            <button
              onClick={() => setOutput(null)}
              style={{
                background: 'none',
                border: 'none',
                color: '#888',
                cursor: 'pointer',
                fontSize: 14,
              }}
            >✕</button>
          </div>
          {output}
        </div>
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
  toolbar: ({ nodeId }) => <PythonToolbar nodeId={nodeId} />,
});

// ── 工具栏 ────────────────────────────────────

function PythonToolbar({ nodeId }: RendererProps) {
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

  return (
    <>
      <button className="btn btn-primary btn-sm" onClick={handleRun}>
        ▶ 运行
      </button>
      <button className="btn btn-sm" onClick={handleSave}>
        💾 保存
      </button>
    </>
  );
}

export default PythonEditor;