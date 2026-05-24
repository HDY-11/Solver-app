// layouts/Footer.tsx

import { useLocation, useParams, useNavigate } from 'react-router-dom';
import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getRenderer } from '../registry/registry';

interface VfsInfo {
  c_exists: boolean;
  c_used: number;
  c_total: number;
  c_node_count: number;
}

function fmtSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function Footer() {
  const location = useLocation();
  const { renderer } = useParams();
  const navigate = useNavigate();
  const [vfsInfo, setVfsInfo] = useState<VfsInfo | null>(null);
  const [expanded, setExpanded] = useState(false);

  const rendererDef = renderer ? getRenderer(renderer) : undefined;

  // 监听 /app/state 展开状态
  useEffect(() => {
    setExpanded(location.pathname === '/app/state');
  }, [location.pathname]);

  // 展开时加载 VFS 信息
  useEffect(() => {
    if (expanded) {
      invoke<VfsInfo>('vfs_info')
        .then(setVfsInfo)
        .catch(() => setVfsInfo(null));
    }
  }, [expanded]);

  // 路径变化时刷新 VFS 信息（如果已展开）
  useEffect(() => {
    if (expanded) {
      invoke<VfsInfo>('vfs_info')
        .then(setVfsInfo)
        .catch(() => setVfsInfo(null));
    }
  }, [location.pathname, expanded]);

  return (
    <footer className="app-footer">
      {/* 左侧：当前状态 */}
      <span>
        {rendererDef ? `${rendererDef.icon} ${rendererDef.label}` : '就绪'}
      </span>

      {/* VFS 使用量 */}
      {vfsInfo ? (
        <span>
          VFS: {fmtSize(vfsInfo.c_used)} / {fmtSize(vfsInfo.c_total)}
        </span>
      ) : (
        <span>VFS: --/-- MB</span>
      )}

      {/* 展开状态时显示详细信息 */}
      {expanded && vfsInfo && (
        <>
          <span className="footer-sep">|</span>
          <span>节点: {vfsInfo.c_node_count}</span>
          <span className="footer-sep">|</span>
          <span>Python 3.12</span>
          <span className="footer-sep">|</span>
          <span>连接池: 8</span>
        </>
      )}

      <span className="footer-spacer" />

      {/* 右侧按钮 */}
      <button
        className="icon-btn"
        title={expanded ? '收起状态' : '展开状态'}
        onClick={() => navigate(expanded ? location.pathname : '/app/state')}
      >
        📊
      </button>
      <button
        className="icon-btn"
        title="设置"
        onClick={() => navigate('/app/window/setting')}
      >
        ⚙
      </button>
    </footer>
  );
}

export default Footer;