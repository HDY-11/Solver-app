// layouts/Header.tsx — 顶部导航栏（拖拽区 + 可交互地址栏 + 窗口控制）

import { useState, useCallback, useRef } from 'react';
import { useNavigate, useLocation } from 'react-router-dom';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { getRendererByExtension } from '../registry/registry';
import { Icon } from '../utils/icons';

function Header() {
  const navigate = useNavigate();
  const location = useLocation();
  const [editValue, setEditValue] = useState('');
  const [editing, setEditing] = useState(false);
  const [maximized, setMaximized] = useState(false);
  const editingRef = useRef(false);
  editingRef.current = editing;
  const appWindow = getCurrentWindow();

  // 监听最大化状态
  appWindow.isMaximized().then(setMaximized);

  const startEdit = useCallback(() => {
    setEditValue(location.pathname + location.search);
    setEditing(true);
  }, [location]);

  const commitEdit = useCallback(() => {
    setEditing(false);
    const raw = editValue.trim();
    if (!raw) return;
    const ext = '.' + (raw.split('.').pop() ?? '');
    const renderer = getRendererByExtension(ext);
    if (renderer) {
      const vfsPath = raw.startsWith('(vfs)/') ? raw : `(vfs)/C/${raw}`;
      navigate(`/app/${renderer.name}/${encodeURIComponent(vfsPath)}`);
    } else {
      navigate(raw);
    }
  }, [editValue, navigate]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter') { e.preventDefault(); commitEdit(); }
    if (e.key === 'Escape') setEditing(false);
  }, [commitEdit]);

  const handleMaximize = async () => {
    await appWindow.toggleMaximize();
    setMaximized(await appWindow.isMaximized());
  };

  return (
    <header className="app-header" data-tauri-drag-region>
      <div style={{ display: 'flex', gap: 4, WebkitAppRegion: 'no-drag' } as React.CSSProperties}>
        <button onClick={() => navigate(-1)} title="后退"><Icon icon="chevron-left" /></button>
        <button onClick={() => navigate(1)} title="前进"><Icon icon="chevron-right" /></button>
      </div>
      <input
        className="header-address"
        style={{
          background: editing ? 'var(--gray-50)' : 'transparent',
          WebkitAppRegion: 'no-drag',
        } as React.CSSProperties}
        value={editing ? editValue : location.pathname + location.search}
        readOnly={!editing}
        onClick={startEdit}
        onKeyDown={handleKeyDown}
        placeholder="输入 VFS 路径，如 script.py"
      />
      <div className="titlebar-controls" style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}>
        <button className="titlebar-btn" onClick={() => appWindow.minimize()} title="最小化"><Icon icon="minus" /></button>
        <button className="titlebar-btn" onClick={handleMaximize} title={maximized ? '还原' : '最大化'}>
          <Icon icon={maximized ? 'maximize' : 'square'} />
        </button>
        <button className="titlebar-btn titlebar-close" onClick={() => appWindow.close()} title="关闭"><Icon icon="xmark" /></button>
      </div>
    </header>
  );
}

export default Header;