// layouts/Main.tsx — 主内容区（缓存渲染，URL pathname 是唯一显示驱动）
//
// tabs 只管"有哪些标签"，不存 activePath。显示完全由当前 URL pathname 决定。

import { useRef, useEffect } from 'react';
import { useLocation } from 'react-router-dom';
import { getRenderer, getPanel } from '../registry/registry';
import { useTabs } from '../hooks/useTabs';
import WelcomeView from './WelcomeView.tsx';

interface CacheEntry {
  key: string;
  element: React.ReactNode;
}

function isWelcome(pathname: string): boolean {
  const parts = pathname.split('/').filter(Boolean);
  return parts.length === 0 || (parts.length === 1 && !parts[0].startsWith('app'));
}

function getTabLabel(pathname: string): { label: string; icon: string } {
  const parts = pathname.split('/').filter(Boolean);
  const renderer = parts.length >= 2 ? parts[1] : null;
  const content = parts.length >= 3 ? parts[2] : null;
  if (renderer === 'window' && content) {
    const panel = getPanel(content);
    if (panel) return { label: panel.label, icon: '📌' };
  }
  const r = renderer ? getRenderer(renderer) : undefined;
  if (r) {
    const name = decodeURIComponent((content ?? '').split('/').pop() ?? '');
    return { label: name, icon: r.icon };
  }
  return { label: pathname, icon: '📄' };
}

function Main() {
  const { pathname } = useLocation();
  const { registerTab } = useTabs();
  const cacheRef = useRef<Map<string, CacheEntry>>(new Map());

  // 注册标签（仅在 pathname 变化时触发一次，无 activePath 竞态）
  useEffect(() => {
    if (!isWelcome(pathname)) {
      registerTab({ path: pathname, ...getTabLabel(pathname) });
    }
  }, [pathname, registerTab]);

  // 确保当前 pathname 的缓存条目存在（在 effect 中创建，不在 render 中改 ref）
  useEffect(() => {
    const map = cacheRef.current;
    if (isWelcome(pathname)) return;
    if (map.has(pathname)) return;

    const parts = pathname.split('/').filter(Boolean);
    const renderer = parts.length >= 2 ? parts[1] : null;
    const content = parts.length >= 3 ? parts[2] : null;

    if (renderer === 'window' && content) {
      const panelDef = getPanel(content);
      if (panelDef) map.set(pathname, { key: pathname, element: <panelDef.component /> });
    } else if (renderer) {
      const rendererDef = getRenderer(renderer);
      if (rendererDef) map.set(pathname, { key: pathname, element: <rendererDef.component nodeId={content ?? null} /> });
    }
  }, [pathname]);

  const map = cacheRef.current;
  if (!map.has('/')) {
    map.set('/', { key: '/', element: <WelcomeView /> });
  }
  if (map.size > 20) {
    const keys = Array.from(map.keys());
    for (const k of keys.slice(0, keys.length - 20)) {
      if (k !== '/' && k !== pathname) map.delete(k);
    }
  }

  const showWelcome = isWelcome(pathname);
  const entries = Array.from(map.values());

  return (
    <div className="app-main" style={{ position: 'relative' }}>
      {entries.map((entry) => (
        <div
          key={entry.key}
          style={{
            position: 'absolute', inset: 0,
            display: (entry.key === '/' && showWelcome) || entry.key === pathname
              ? undefined : 'none',
          }}
        >
          {entry.element}
        </div>
      ))}
    </div>
  );
}

export default Main;