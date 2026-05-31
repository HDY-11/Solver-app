// layouts/Footer.tsx — 底部状态栏

import { useLocation, useNavigate } from 'react-router-dom';
import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { error as logError } from '@tauri-apps/plugin-log';
import { Icon } from '../utils/icons';
import { getInfo } from '../api/vfs';
import { useSettings } from '../hooks/useSettings';
import type { VfsInfo } from '../types';
import { fmtSize } from '../types';

interface VolumeInfo {
  volume: string;
  node_count: number;
  total_size: number;
  is_real: boolean;
}

function Footer() {
  const { pathname } = useLocation();
  const navigate = useNavigate();
  const { settings, update } = useSettings();
  const [vfsInfo, setVfsInfo] = useState<VfsInfo | null>(null);
  const [volInfo, setVolInfo] = useState<VolumeInfo | null>(null);
  const [expanded, setExpanded] = useState(false);

  const theme = settings.theme;

  // 从当前路径提取卷名
  const currentVolume = useCallback((): string => {
    const parts = pathname.split('/').filter(Boolean);
    if (parts.length >= 3 && parts[0] === 'app') {
      const content = parts.slice(2).join('/');
      // content 是编码后的 VFS 路径，如 (vfs)/C/hello.py
      const decoded = decodeURIComponent(content);
      const m = decoded.match(/^\(vfs\)\/([A-Z])/);
      if (m) return m[1];
    }
    return 'C';
  }, [pathname]);

  const vol = currentVolume();

  // 主题生效
  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
  }, [theme]);

  const toggleTheme = () => {
    update({ theme: theme === 'dark' ? 'light' : 'dark' });
  };

  // 加载卷信息
  const loadVolInfo = useCallback(async () => {
    try {
      const info = await invoke<VolumeInfo>('get_volume_info', { volume: vol });
      setVolInfo(info);
    } catch { setVolInfo(null); }
  }, [vol]);

  useEffect(() => { loadVolInfo(); }, [loadVolInfo]);

  // VFS C盘信息（展开时）
  useEffect(() => {
    if (!expanded) return;
    getInfo()
      .then(setVfsInfo)
      .catch((err) => { logError(`Footer: 获取 VFS 信息失败: ${err}`); setVfsInfo(null); });
  }, [expanded]);

  // 监听 app-ready 刷新
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    import('@tauri-apps/api/event').then(({ listen }) => {
      listen('app-ready', () => {
        loadVolInfo();
      }).then(fn => { unlisten = fn; });
    });
    return () => { unlisten?.(); };
  }, [loadVolInfo]);

  return (
    <footer className="app-footer">
      <span>
        {vol !== 'C' ? `${vol}盘: ` : 'VFS: '}
        {volInfo ? `${volInfo.node_count} 项 · ${fmtSize(volInfo.total_size)}` : '--'}
      </span>

      {expanded && vfsInfo && (
        <>
          <span className="footer-sep">|</span>
          <span>C盘节点: {vfsInfo.c_node_count}</span>
          <span className="footer-sep">|</span>
          <span>C盘用量: {fmtSize(vfsInfo.c_used)}/{fmtSize(vfsInfo.c_total)}</span>
        </>
      )}

      <span className="footer-spacer" />

      <button className="icon-btn" title={expanded ? '收起状态' : '展开状态'}
        onClick={() => setExpanded(!expanded)}>
        <Icon icon="chart" />
      </button>
      <button className="icon-btn"
        title={theme === 'dark' ? '切换亮色主题' : '切换深色主题'}
        onClick={toggleTheme}>
        <Icon icon={theme === 'dark' ? 'sun' : 'moon'} />
      </button>
      <button className="icon-btn" title="设置"
        onClick={() => navigate('/app/window/setting')}>
        <Icon icon="gear" />
      </button>
    </footer>
  );
}

export default Footer;