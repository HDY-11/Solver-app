// renderers/HtmlViewer.tsx — HTML 文件查看器
//
// 从 VFS 读取 HTML 内容，用 iframe srcdoc 渲染预览。
// 模式切换改用 URL search params（?mode=source），避免兄弟组件间 window 事件通信。

import { useState, useEffect } from 'react';
import { useSearchParams } from 'react-router-dom';
import { error as logError } from '@tauri-apps/plugin-log';
import { registerRenderer } from '../registry/registry';
import { readFile } from '../api/vfs';
import { Loading } from '../components/Loading';
import { useToast } from '../hooks/useToast';
import type { RendererProps } from '../registry/types';
import styles from './HtmlViewer.module.css';

type ViewMode = 'preview' | 'source';

function HtmlViewer({ nodeId }: RendererProps) {
  const { addToast } = useToast();
  const vfsPath = nodeId ? decodeURIComponent(nodeId) : null;
  const [searchParams] = useSearchParams();
  const [html, setHtml] = useState('');
  const [loading, setLoading] = useState(false);
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

  return (
    <>
      <button className="btn btn-sm" onClick={switchToPreview}>
        🌐 预览
      </button>
      <button className="btn btn-sm" onClick={switchToSource}>
        {'<>'} 源码
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
