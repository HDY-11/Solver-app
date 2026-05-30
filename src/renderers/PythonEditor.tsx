// renderers/PythonEditor.tsx — Python 代码编辑器（基于 Monaco）

import { useState, useEffect, useCallback, useRef } from 'react';
import { useNavigate, useLocation } from 'react-router-dom';
import { error as logError } from '@tauri-apps/plugin-log';
import Editor from '@monaco-editor/react';
import { registerRenderer } from '../registry/registry';
import { readFile, writeFile } from '../api/vfs';
import { runScript } from '../api/script';
import { useToast } from '../hooks/useToast';
import { Loading } from '../components/Loading';
import { activeEditor } from '../services/activeEditor';
import { commandService, Commands } from '../services/commandService';
import { useSettings } from '../hooks/useSettings';
import { Icon } from '../utils/icons';
import type { RendererProps } from '../registry/types';
import styles from './PythonEditor.module.css';

/** 从 VFS 路径中提取并解码文件名（处理 URL 编码的中文） */
function getFileName(vfsPath: string | null): string {
  if (!vfsPath) return '';
  try {
    return decodeURIComponent(vfsPath.split('/').pop() ?? '');
  } catch {
    return vfsPath.split('/').pop() ?? '';
  }
}

function PythonEditor({ nodeId }: RendererProps) {
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const { addToast } = useToast();
  const { settings } = useSettings();
  const vfsPath = nodeId ? decodeURIComponent(nodeId) : null;
  const [code, setCode] = useState<string>('');
  const [loading, setLoading] = useState(false);
  const [saved, setSaved] = useState(true);
  const [running, setRunning] = useState(false);
  const codeRef = useRef(code);
  const pathRef = useRef(vfsPath);
  codeRef.current = code;
  pathRef.current = vfsPath;

  // 加载文件
  useEffect(() => {
    if (!vfsPath) return;
    setLoading(true);
    readFile(vfsPath)
      .then(content => { setCode(content); setSaved(true); })
      .catch(err => {
        logError(`PythonEditor: 加载文件失败 (${vfsPath}): ${err}`);
        setCode(`# 加载失败: ${err}`);
      })
      .finally(() => setLoading(false));
  }, [vfsPath]);

  // TimelinePanel 恢复旧版本时自动重载
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

  // 运行 → 保存 → 跳转到运行结果页面
  const handleRun = useCallback(async () => {
    const p = pathRef.current;
    if (!p) return;
    setRunning(true);
    try {
      await handleSave();
      const { run_path } = await runScript(p);
      navigate(`/app/run/${encodeURIComponent(run_path)}`);
    } catch (err) {
      logError(`PythonEditor: 运行失败 (${p}): ${err}`);
      addToast('error', `运行失败: ${err}`);
    } finally {
      setRunning(false);
    }
  }, [handleSave, addToast, navigate]);

  // 注册为活跃编辑器（命令系统通过 activeEditor 找到当前应操作的实例）
  // 仅当 URL pathname 对应当前实例时才设为活跃
  useEffect(() => {
    const expectedPath = `/app/py/${encodeURIComponent(vfsPath ?? '')}`;
    if (pathname !== expectedPath) return;

    const unreg = activeEditor.setActive({
      vfsPath,
      save: handleSave,
      run: handleRun,
    });
    return unreg;
  }, [pathname, vfsPath, handleSave, handleRun]);

  // 设置 codeRef / pathRef 为最新值
  codeRef.current = code;
  pathRef.current = vfsPath;

  return (
    <div className={styles.container}>
      {loading ? (
        <Loading text="加载文件中..." />
      ) : (
      <>
      <div className={styles.editorArea}>
        {!saved && (
          <div className={styles.unsavedBadge}><Icon icon="circle" /> 未保存</div>
        )}
        {running && (
          <div className={styles.unsavedBadge} style={{ background: '#f0ad4e', color: '#fff' }}>
            <Icon icon="spinner" /> 运行中...
          </div>
        )}
        <Editor
          height="100%"
          defaultLanguage="python"
          value={code}
          onChange={v => { setCode(v ?? ''); setSaved(false); }}
          theme={settings.theme === 'light' ? 'vs' : 'vs-dark'}
          options={{
            minimap: { enabled: false },
            fontSize: settings.font_size,
            fontFamily: 'var(--font-mono)',
            padding: { top: 16, bottom: 16 },
            scrollBeyondLastLine: false,
            automaticLayout: true,
            tabSize: settings.tab_size,
            insertSpaces: true,
          }}
        />
        <div className={styles.fileBadge}>
          <Icon icon={saved ? 'check' : 'circle'} /> {getFileName(vfsPath)}
        </div>
      </div>
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
  icon: 'python',
  label: 'Python',
  toolbar: () => <PythonToolbar />,
});

// ── 工具栏 ────────────────────────────────────

function PythonToolbar() {
  const handleSave = () => commandService.executeCommand(Commands.EDITOR_SAVE);
  const handleRun = () => commandService.executeCommand(Commands.EDITOR_RUN);

  return (
    <>
      <button className="btn btn-primary btn-sm" onClick={handleRun}>
        <Icon icon="play" /> 运行
      </button>
      <button className="btn btn-sm" onClick={handleSave}>
        <Icon icon="save" /> 保存
      </button>
    </>
  );
}

export default PythonEditor;