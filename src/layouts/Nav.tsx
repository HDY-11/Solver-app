// layouts/Nav.tsx — 标签页导航栏
//
// 显示所有打开的标签，用 pathname 高亮当前，点击 navigate 切换，✕ 关闭。

import { useLocation, useNavigate } from 'react-router-dom';
import { useTabs } from '../hooks/useTabs';

function Nav() {
  const { pathname } = useLocation();
  const navigate = useNavigate();
  const { tabs, closeTab } = useTabs();

  return (
    <nav className="app-nav">
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
              onClick={() => isActive || navigate(tab.path)}
              title={tab.path}
            >
              <span className="nav-tab__icon">{tab.icon}</span>
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
              >✕</button>
            </span>
          );
        })
      )}
    </nav>
  );
}

export default Nav;