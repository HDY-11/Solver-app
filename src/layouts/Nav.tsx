// layouts/Nav.tsx — 标签页导航栏
//
// 同样在 <Routes> 外部，使用 useLocation() 手动解析路径。

import { useLocation } from 'react-router-dom';
import { getRenderer } from '../registry/registry';

function Nav() {
  const { pathname } = useLocation();
  const parts = pathname.split('/').filter(Boolean);
  const renderer = parts.length >= 2 && parts[0] === 'app' ? parts[1] : null;
  const content = parts.length >= 3 && parts[0] === 'app' ? parts[2] : null;
  const rendererDef = renderer ? getRenderer(renderer) : undefined;

  const hasTab = rendererDef && content;

  return (
    <nav className="app-nav">
      {hasTab ? (
        <span className="nav-tab nav-tab--active">
          {rendererDef!.icon} {(content ?? '').split('/').pop()}
        </span>
      ) : (
        <span style={{ color: 'var(--gray-500)', fontSize: '0.8125rem' }}>
          无打开的文件
        </span>
      )}
    </nav>
  );
}

export default Nav;