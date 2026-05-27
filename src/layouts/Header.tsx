// layouts/Header.tsx — 顶部导航栏 + 可交互地址栏

import { useState, useCallback, useRef } from 'react';
import { useNavigate, useLocation } from 'react-router-dom';
import { getRendererByExtension } from '../registry/registry';

function Header() {
  const navigate = useNavigate();
  const location = useLocation();
  const [editValue, setEditValue] = useState('');
  const [editing, setEditing] = useState(false);
  const editingRef = useRef(false);
  editingRef.current = editing;

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

  // 延迟 onBlur，避免点击前进/后退按钮时误提交
  const handleBlur = useCallback(() => {
    setTimeout(() => {
      if (!editingRef.current) return; // 已经在 timeout 期间被其他操作取消
      // 延迟后仍处于编辑状态 → 提交
    }, 100);
  }, []);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter') { e.preventDefault(); commitEdit(); }
    if (e.key === 'Escape') setEditing(false);
  }, [commitEdit]);

  return (
    <header className="app-header">
      <div style={{ display: 'flex', gap: 4 }}>
        <button onClick={() => navigate(-1)} title="后退">←</button>
        <button onClick={() => navigate(1)} title="前进">→</button>
      </div>
      <input
        className="header-address"
        style={{
          background: editing ? 'var(--gray-50)' : 'transparent',
        }}
        value={editing ? editValue : location.pathname + location.search}
        readOnly={!editing}
        onClick={startEdit}
        onBlur={handleBlur}
        onKeyDown={handleKeyDown}
        placeholder="输入 VFS 路径，如 script.py"
      />
    </header>
  );
}

export default Header;