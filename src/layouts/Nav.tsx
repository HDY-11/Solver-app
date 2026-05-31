// layouts/Nav.tsx — 标签页导航栏（支持拖拽分离/合并窗口）

import { useCallback, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useTabs } from '../hooks/useTabs';
import { Icon } from '../utils/icons';

function Nav({ detached }: { detached?: boolean }) {
  const { pathname } = useLocation();
  const navigate = useNavigate();
  const { tabs, closeTab, registerTab } = useTabs();
  const [contextMenu, setContextMenu] = useState<{ path: string; x: number; y: number } | null>(null);

  // 拖拽开始：记录标签信息
  const handleDragStart = useCallback((e: React.DragEvent, tabPath: string) => {
    e.dataTransfer.setData('text/plain', tabPath);
    e.dataTransfer.effectAllowed = 'move';
  }, []);

  // 拖拽结束：若拖出 Nav 区域则分离窗口
  const handleDragEnd = useCallback(async (e: React.DragEvent, tabPath: string) => {
    if (e.dataTransfer.dropEffect === 'none') {
      // 拖到了 Nav 外面 → 创建分离窗口
      const tab = tabs.find(t => t.path === tabPath);
      if (!tab) return;
      try {
        await invoke('detach_window', { urlPath: tab.path, title: tab.label });
        closeTab(tabPath);
        if (pathname === tabPath) {
          const idx = tabs.findIndex(t => t.path === tabPath);
          const next = tabs[idx + 1] ?? tabs[idx - 1];
          navigate(next ? next.path : '/');
        }
      } catch (err) {
        console.error('分离窗口失败:', err);
      }
    }
  }, [tabs, closeTab, navigate, pathname]);

  // Nav 作为 drop zone：接收从分离窗口拖回的标签
  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
  }, []);

  const handleDrop = useCallback(async (e: React.DragEvent) => {
    e.preventDefault();
    const tabPath = e.dataTransfer.getData('text/plain');
    if (!tabPath) return;
    // 从分离窗口拖回 → 重新打开标签
    const parts = tabPath.split('/').filter(Boolean);
    if (parts.length >= 2) {
      const content = parts.slice(2).join('/');
      const label = decodeURIComponent(content?.split('/').pop() ?? tabPath);
      registerTab({ path: tabPath, label, icon: 'file' });
      navigate(tabPath);
    }
  }, [navigate, registerTab]);

  // ── 分离窗口：右键合并回主窗口 ──
  const handleContextMenu = useCallback((e: React.MouseEvent, tabPath: string) => {
    if (!detached) return;
    e.preventDefault();
    setContextMenu({ path: tabPath, x: e.clientX, y: e.clientY });
  }, [detached]);

  const handleMergeBack = useCallback(async () => {
    if (!contextMenu) return;
    const tab = tabs.find(t => t.path === contextMenu.path);
    if (!tab) return;
    try {
      // 发送合并事件到主窗口
      console.log('[merge] 分离窗口请求合并:', tab.path);
      await invoke('emit_merge_request', { path: tab.path, label: tab.label, icon: tab.icon });
      console.log('[merge] 合并请求已发送');
      closeTab(contextMenu.path);
      // 如果是最后一个标签，关闭整个窗口
      if (tabs.length <= 1) {
        getCurrentWindow().close();
      }
    } catch (err) {
      console.error('合并失败:', err);
    }
    setContextMenu(null);
  }, [contextMenu, tabs, closeTab]);

  return (
    <>
    <nav
      className="app-nav"
      onDragOver={handleDragOver}
      onDrop={handleDrop}
      onClick={() => setContextMenu(null)}
    >
      {tabs.length === 0 ? (
        <span style={{ color: 'var(--gray-500)', fontSize: '0.8125rem' }}>
          无打开的文件
        </span>
      ) : (
        tabs.map((tab) => {
          const isActive = tab.path === pathname;
          return (
            <span
              key={tab.path}
              className={`nav-tab ${isActive ? 'nav-tab--active' : ''}`}
              draggable={!detached}
              onDragStart={(e) => !detached && handleDragStart(e, tab.path)}
              onDragEnd={(e) => !detached && handleDragEnd(e, tab.path)}
              onClick={() => isActive || navigate(tab.path)}
              onContextMenu={(e) => handleContextMenu(e, tab.path)}
              title={detached ? '右键可合并回主窗口' : '拖拽可分离窗口'}
            >
              <span className="nav-tab__icon"><Icon icon={tab.icon} /></span>
              <span className="nav-tab__label">{tab.label}</span>
              <button
                className="nav-tab__close"
                onClick={(e) => {
                  e.stopPropagation();
                  closeTab(tab.path);
                  if (isActive) {
                    const idx = tabs.findIndex((t) => t.path === tab.path);
                    const next = tabs[idx + 1] ?? tabs[idx - 1];
                    navigate(next ? next.path : '/');
                  }
                }}
                title="关闭"
              ><Icon icon="xmark" /></button>
            </span>
          );
        })
      )}
    </nav>
    {/* 分离窗口右键菜单 */}
    {contextMenu && (
      <div className="context-menu" style={{ left: contextMenu.x, top: contextMenu.y }}
        onClick={() => setContextMenu(null)}>
        <div className="context-menu__item" onClick={handleMergeBack}><Icon icon="download" /> 合并回主窗口</div>
      </div>
    )}
    </>
  );
}

export default Nav;