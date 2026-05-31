// renderers/HtmlViewer.tsx — HTML 文件查看器
//
// 从 VFS 读取 HTML 内容，用 iframe srcdoc 渲染预览。
// 模式切换改用 URL search params（?mode=source），避免兄弟组件间 window 事件通信。

import { useState, useEffect, useRef } from 'react';
import { useSearchParams } from 'react-router-dom';
import { error as logError } from '@tauri-apps/plugin-log';
import Editor from '@monaco-editor/react';
import { registerRenderer } from '../registry/registry';
import { readFile, writeFile } from '../api/vfs';
import { Loading } from '../components/Loading';
import { Icon } from '../utils/icons';
import { useToast } from '../hooks/useToast';
import type { RendererProps } from '../registry/types';
import styles from './HtmlViewer.module.css';

type ViewMode = 'preview' | 'source' | 'edit';

function HtmlViewer({ nodeId }: RendererProps) {
  const { addToast } = useToast();
  const vfsPath = nodeId ? decodeURIComponent(nodeId) : null;
  const [searchParams] = useSearchParams();
  const [html, setHtml] = useState('');
  const [loading, setLoading] = useState(false);
  const htmlRef = useRef(html);
  htmlRef.current = html;
  const vfsPathRef = useRef(vfsPath);
  vfsPathRef.current = vfsPath;
  const mode: ViewMode = (searchParams.get('mode') as ViewMode) || 'preview';

  useEffect(() => {
    if (!vfsPath) return;
    setLoading(true);
    readFile(vfsPath)
      .then(setHtml)
      .catch((err) => {
        logError(`HtmlViewer: 加载失败 (${vfsPath}): ${err}`);
        addToast('error', `加载 HTML 失败: ${err}`);
      })
      .finally(() => setLoading(false));
  }, [vfsPath, addToast]);

  // 监听刷新事件
  useEffect(() => {
    const handler = () => {
      if (!vfsPath) return;
      setLoading(true);
      readFile(vfsPath)
        .then(setHtml)
        .catch(() => {})
        .finally(() => setLoading(false));
    };
    window.addEventListener('html-viewer:refresh', handler);
    return () => window.removeEventListener('html-viewer:refresh', handler);
  }, [vfsPath]);

  // 监听保存事件（编辑模式下）
  useEffect(() => {
    const handler = async () => {
      if (!vfsPathRef.current) return;
      try {
        await writeFile(vfsPathRef.current, htmlRef.current);
        addToast('success', '已保存');
      } catch (err) {
        logError(`HtmlViewer: 保存失败: ${err}`);
        addToast('error', `保存失败: ${err}`);
      }
    };
    window.addEventListener('html-viewer:save', handler);
    return () => window.removeEventListener('html-viewer:save', handler);
  }, [addToast]);

  if (loading) return <Loading text="加载 HTML..." />;

  if (!nodeId) {
    return (
      <div className={styles.placeholder}>
        请从侧边栏选择 HTML 文件
      </div>
    );
  }

  return (
    <div className={styles.container}>
      {mode === 'source' ? (
        <pre className={styles.sourceView}>{html}</pre>
      ) : mode === 'edit' ? (
        <Editor
          height="100%"
          defaultLanguage="html"
          value={html}
          onChange={v => { setHtml(v ?? ''); }}
          theme="vs-dark"
          options={{
            minimap: { enabled: false },
            fontSize: 14,
            fontFamily: 'var(--font-mono)',
            padding: { top: 16 },
            scrollBeyondLastLine: false,
            automaticLayout: true,
          }}
        />
      ) : (
        <iframe
          className={styles.previewFrame}
          srcDoc={html}
          sandbox="allow-scripts allow-same-origin"
          title="HTML 预览"
        />
      )}
    </div>
  );
}

// ── 工具栏 ────────────────────────────────────

function HtmlToolbar() {
  const switchToPreview = () => {
    const url = new URL(window.location.href);
    url.searchParams.delete('mode');
    window.history.replaceState(null, '', url.toString());
    window.dispatchEvent(new Event('popstate'));
  };
  const switchToSource = () => {
    const url = new URL(window.location.href);
    url.searchParams.set('mode', 'source');
    window.history.replaceState(null, '', url.toString());
    window.dispatchEvent(new Event('popstate'));
  };
  const switchToEdit = () => {
    const url = new URL(window.location.href);
    url.searchParams.set('mode', 'edit');
    window.history.replaceState(null, '', url.toString());
    window.dispatchEvent(new Event('popstate'));
  };
  const handleSave = () => {
    window.dispatchEvent(new CustomEvent('html-viewer:save'));
  };
  const handleRefresh = () => {
    window.dispatchEvent(new CustomEvent('html-viewer:refresh'));
  };

  return (
    <>
      <button className="toolbar-btn toolbar-btn--primary" onClick={switchToPreview}>
        <Icon icon="eye" /> 预览
      </button>
      <button className="toolbar-btn" onClick={switchToSource}>
        <Icon icon="code" /> 源码
      </button>
      <button className="toolbar-btn" onClick={switchToEdit}>
        <Icon icon="edit" /> 编辑
      </button>
      <span style={{ width: 1, height: 20, background: 'var(--gray-300)', margin: '0 4px' }} />
      <button className="toolbar-btn" onClick={handleSave}>
        <Icon icon="save" /> 保存
      </button>
      <button className="toolbar-btn" onClick={handleRefresh}>
        <Icon icon="rotate" /> 刷新
      </button>
    </>
  );
}

// ── 注册 ──────────────────────────────────────

registerRenderer({
  name: 'html',
  extensions: ['.html', '.htm'],
  component: HtmlViewer,
  icon: 'globe',
  label: '浏览器',
  toolbar: () => <HtmlToolbar />,
});

export default HtmlViewer;
