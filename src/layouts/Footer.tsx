// layouts/Footer.tsx — 底部状态栏：渲染器信息 + VFS 用量 + 快捷入口 + 主题切换
//
// 使用 useLocation().pathname 手动解析当前 renderer（Footer 在 <Routes> 外部）。

import { useLocation, useNavigate } from 'react-router-dom';
import { useState, useEffect } from 'react';
import { error as logError } from '@tauri-apps/plugin-log';
import { getRenderer } from '../registry/registry';
import { getInfo } from '../api/vfs';
import type { VfsInfo, Theme } from '../types';
import { fmtSize } from '../types';

function loadTheme(): Theme {
  try {
    const raw = localStorage.getItem('solver-settings');
    if (raw) return JSON.parse(raw).theme === 'light' ? 'light' : 'dark';
  } catch { /* ignore */ }
  return 'dark';
}

function Footer() {
  const { pathname } = useLocation();
  const navigate = useNavigate();
  const [vfsInfo, setVfsInfo] = useState<VfsInfo | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [theme, setTheme] = useState<Theme>(loadTheme);

  // 手动解析 renderer（Footer 在 Routes 外，useParams 不可用）
  const parts = pathname.split('/').filter(Boolean);
  const renderer = parts.length >= 2 && parts[0] === 'app' ? parts[1] : null;
  const rendererDef = renderer ? getRenderer(renderer) : undefined;

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
  }, [theme]);

  const toggleTheme = () => {
    setTheme((prev) => {
      const next: Theme = prev === 'dark' ? 'light' : 'dark';
      try {
        const raw = localStorage.getItem('solver-settings');
        const settings = raw ? JSON.parse(raw) : {};
        settings.theme = next;
        localStorage.setItem('solver-settings', JSON.stringify(settings));
      } catch { /* ignore */ }
      return next;
    });
  };

  // 合并为一个 effect：展开或路径变化时刷新 VFS 信息
  useEffect(() => {
    if (!expanded) return;
    getInfo()
      .then(setVfsInfo)
      .catch((err) => {
        logError(`Footer: 获取 VFS 信息失败: ${err}`);
        setVfsInfo(null);
      });
  }, [expanded, pathname]);

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
      {/* 主题切换 */}
      <button
        className="icon-btn"
        title={theme === 'dark' ? '切换亮色主题' : '切换深色主题'}
        onClick={toggleTheme}
      >
        {theme === 'dark' ? '☀️' : '🌙'}
      </button>
      <button
        className="icon-btn"
        title={expanded ? '收起状态' : '展开状态'}
        onClick={() => setExpanded(!expanded)}
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