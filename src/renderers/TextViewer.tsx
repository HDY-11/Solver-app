// renderers/TextViewer.tsx — 文本文件编辑器
//
// 支持 .txt .log .csv .json .md .yaml 等文本格式，可编辑保存。

import { useState, useEffect, useCallback, useRef } from 'react';
import { error as logError } from '@tauri-apps/plugin-log';
import Editor from '@monaco-editor/react';
import { registerRenderer } from '../registry/registry';
import { readFile, writeFile } from '../api/vfs';
import { Loading } from '../components/Loading';
import { useToast } from '../hooks/useToast';
import type { RendererProps } from '../registry/types';
import styles from './TextViewer.module.css';

/** 根据文件扩展名映射 Monaco 语言标识 */
function langFromExt(name: string): string {
  const ext = name.split('.').pop()?.toLowerCase() ?? '';
  const map: Record<string, string> = {
    json: 'json', md: 'markdown', yaml: 'yaml', yml: 'yaml',
    toml: 'ini', cfg: 'ini', ini: 'ini', csv: 'plaintext',
    log: 'plaintext', txt: 'plaintext',
  };
  return map[ext] ?? 'plaintext';
}

function TextViewer({ nodeId }: RendererProps) {
  const { addToast } = useToast();
  const vfsPath = nodeId ? decodeURIComponent(nodeId) : null;
  const [code, setCode] = useState('');
  const [loading, setLoading] = useState(false);
  const codeRef = useRef(code);
  const pathRef = useRef(vfsPath);
  codeRef.current = code;
  pathRef.current = vfsPath;

  useEffect(() => {
    if (!vfsPath) return;
    setLoading(true);
    readFile(vfsPath)
      .then(setCode)
      .catch((err) => {
        logError(`TextViewer: 加载失败 (${vfsPath}): ${err}`);
        addToast('error', `加载失败: ${err}`);
      })
      .finally(() => setLoading(false));
  }, [vfsPath, addToast]);

  // 版本恢复时自动重载
  const vfsPathRef = useRef(vfsPath);
  vfsPathRef.current = vfsPath;
  useEffect(() => {
    const handler = (e: Event) => {
      const { path } = (e as CustomEvent).detail as { path: string };
      if (path !== vfsPathRef.current) return;
      readFile(path).then(setCode).catch(() => {});
    };
    window.addEventListener('vfs:file-changed', handler);
    return () => window.removeEventListener('vfs:file-changed', handler);
  }, []);

  const handleSave = useCallback(async () => {
    const p = pathRef.current;
    if (!p) return;
    try {
      await writeFile(p, codeRef.current);
      addToast('success', '已保存');
    } catch (err) {
      logError(`TextViewer: 保存失败 (${p}): ${err}`);
      addToast('error', `保存失败: ${err}`);
    }
  }, [addToast]);

  const saveRef = useRef(handleSave);
  saveRef.current = handleSave;

  // 快捷键 Ctrl+S + toolbar 自定义事件
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 's') { e.preventDefault(); saveRef.current(); }
    };
    const onSave = () => saveRef.current();
    window.addEventListener('keydown', onKey);
    window.addEventListener('text-editor:save', onSave);
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('text-editor:save', onSave);
    };
  }, []);

  if (loading) return <Loading text="加载中..." />;

  if (!nodeId) {
    return (
      <div className={styles.placeholder}>
        请从侧边栏选择文本文件
      </div>
    );
  }

  const fileName = vfsPath?.split('/').pop() ?? '';

  return (
    <div className={styles.container}>
      <div className={styles.editorArea}>
        <Editor
          height="100%"
          defaultLanguage={langFromExt(fileName)}
          value={code}
          onChange={(v) => setCode(v ?? '')}
          theme="vs-dark"
          options={{
            minimap: { enabled: false },
            fontSize: 14,
            fontFamily: 'var(--font-mono)',
            padding: { top: 16, bottom: 16 },
            scrollBeyondLastLine: false,
            automaticLayout: true,
            tabSize: 2,
            insertSpaces: true,
            wordWrap: 'on',
          }}
        />
      </div>
    </div>
  );
}

// ── 工具栏 ────────────────────────────────────

function TextToolbar() {
  const handleSave = () => window.dispatchEvent(new Event('text-editor:save'));

  return (
    <button className="btn btn-primary btn-sm" onClick={handleSave}>
      💾 保存
    </button>
  );
}

// ── 注册 ──────────────────────────────────────

registerRenderer({
  name: 'text',
  extensions: ['.txt', '.log', '.csv', '.json', '.md', '.yaml', '.yml', '.toml', '.cfg', '.ini'],
  component: TextViewer,
  icon: '📄',
  label: '文本',
  toolbar: () => <TextToolbar />,
});

export default TextViewer;

