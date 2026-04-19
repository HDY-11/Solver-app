import { useState } from 'react';
import EditorPage from './pages/EditorPage';
import './App.css';
import { Page, PageId } from './types';
import { BarProvider, useBar } from './components/BarContext';

const AppContent: React.FC = () => {
  const [currentPage, setCurrentPage] = useState<PageId>('EditorPage');
  const { barContent } = useBar();

  const pages: Page[] = [
    {
      id: 'EditorPage',
      name: '编辑器',
      component: <EditorPage defaultLanguage="python" />,
      details: '代码编辑器',
    },
    {
      id: 'settings',
      name: '设置',
      component: (
        <div className="page-container">
          <h2>设置</h2>
          <p>偏好设置开发中...</p>
        </div>
      ),
      details: '偏好设置',
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
                <span className="nav-name">{page.name}</span>
              </button>
            ))}
          </nav>
        </div>
      </header>
      
      {/* 工具栏区域 - 由页面动态注入内容 */}
      <div className="bar">
        <div className="bar-content">
          {barContent}
        </div>
      </div>
      
      {/* 侧边栏 - 目前为空，可后续添加菜单 */}
      <aside className="app-sidebar">
        <div className="sidebar-section">
          <div className="sidebar-title">导航</div>
          {/* 示例菜单项，可根据需要取消注释 */}
          {/* <div className="sidebar-item">
            <span className="sidebar-icon">📁</span>
            <span>文件</span>
          </div> */}
        </div>
      </aside>
      
      {/* 主内容区 */}
      <main className="main-content">
        <div className="content-container">
          {currentPageComponent}
        </div>
      </main>
    </div>
  );
};

function App() {
  return (
    <BarProvider>
      <AppContent />
    </BarProvider>
  );
}

export default App;