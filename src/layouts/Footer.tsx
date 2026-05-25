// layouts/Footer.tsx — 底部状态栏：渲染器信息 + VFS 用量 + 快捷入口 + 主题切换

import { useLocation, useParams, useNavigate } from 'react-router-dom';
import { useState, useEffect } from 'react';
import { error as logError } from '@tauri-apps/plugin-log';
import { getRenderer } from '../registry/registry';
import { getInfo } from '../api/vfs';
import type { VfsInfo, Theme } from '../types';

function fmtSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** 从 localStorage 读取主题设置，默认 dark */
function loadTheme(): Theme {
  try {
    const raw = localStorage.getItem('solver-settings');
    if (raw) return JSON.parse(raw).theme === 'light' ? 'light' : 'dark';
  } catch { /* ignore */ }
  return 'dark';
}

function Footer() {
  const location = useLocation();
  const { renderer } = useParams();
  const navigate = useNavigate();
  const [vfsInfo, setVfsInfo] = useState<VfsInfo | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [theme, setTheme] = useState<Theme>(loadTheme);

  const rendererDef = renderer ? getRenderer(renderer) : undefined;

  // 初始化 + 切换主题：修改 document 的 data-theme 属性
  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
  }, [theme]);

  const toggleTheme = () => {
    setTheme((prev) => {
      const next: Theme = prev === 'dark' ? 'light' : 'dark';
      // 同步写回 localStorage
      try {
        const raw = localStorage.getItem('solver-settings');
        const settings = raw ? JSON.parse(raw) : {};
        settings.theme = next;
        localStorage.setItem('solver-settings', JSON.stringify(settings));
      } catch { /* ignore */ }
      return next;
    });
  };

  // 监听 /app/state 展开状态
  useEffect(() => {
    setExpanded(location.pathname === '/app/state');
  }, [location.pathname]);

  // 展开时加载 VFS 信息
  useEffect(() => {
    if (expanded) {
      getInfo()
        .then(setVfsInfo)
        .catch((err) => {
          logError(`Footer: 获取 VFS 信息失败: ${err}`);
          setVfsInfo(null);
        });
    }
  }, [expanded]);

  // 路径变化时刷新 VFS 信息（如果已展开）
  useEffect(() => {
    if (expanded) {
      getInfo()
        .then(setVfsInfo)
        .catch((err) => {
          logError(`Footer: 获取 VFS 信息失败: ${err}`);
          setVfsInfo(null);
        });
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