import { useState } from 'react';
import TSPSolver from './pages/TSPSolver';
import EditorPage from './pages/EditorPage';
import './App.css';
import { Page, PageId } from './types';
import Popover from './components/Popover';

function App() {
  const [currentPage, setCurrentPage] = useState<PageId>('solver');

  const pages: Page[] = [
    {
      id: 'solver',
      name: '模板',
      component: <TSPSolver />,
    },
    {
      id: 'about',
      name: '关于',
      component: (
        <div className="page-container">
          <h2>关于本应用</h2>
          <p>TSP 问题可视化求解器</p>
          <p>使用 Tauri + React + Rust 构建</p>
          <p>算法包括：贪心算法、MST+DFS序等</p>
        </div>
      ),
    },
    {
      id: 'settings',
      name: '设置',
      component: (
        <div className="page-container">
          <h2>设置</h2>
          <p>更多功能开发中...</p>
        </div>
      ),
      details: '偏好设置',
    },
    {
      id: 'EditorPage',
      name: '编辑器',
      component: <EditorPage defaultLanguage="python" />,
      details: '代码编辑器',
    }
  ];

  const currentPageComponent = pages.find(p => p.id === currentPage)?.component;

  return (
  <div className="App">
    {/* 顶部导航栏 */}
    <header className="app-header">
      <div className="header-content">
        <nav className="header-nav">
          {pages.map(page => (
            <button
              key={page.id}
              className={`nav-link ${currentPage === page.id ? 'active' : ''}`}
              onClick={() => setCurrentPage(page.id)}
              title={page.details}
            >
              <span className="nav-icon">{page.icon}</span>
              <span className="nav-name">{page.name}</span>
            </button>
          ))}
        </nav>
      </div>
    </header>
    
    {/* 工具栏区域（预留） */}
    <div className="bar">
      {/* 未来可以放工具栏、面包屑等 */}
      <span>hdfgdgf</span>
    </div>
    
    {/* 侧边栏 */}
    <aside className="app-sidebar">114514</aside>
    
    {/* 主内容区 */}
    <main className="main-content">
      <div className="content-container">
        <div>{currentPageComponent}</div>
      </div>
    </main>
  </div>
  );
}

export default App;