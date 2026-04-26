import { useState } from 'react';
import EditorPage from './pages/EditorPage';
import ViewsPage from './pages/ViewsPage';
import './App.css';
import { PageId } from './types';
import { BarProvider, useBar } from './components/BarContext';

// 页面渲染函数（抽到组件外）
const renderEditorPage = (visible: boolean) => (
  <EditorPage defaultLanguage="python" display={visible}/>
)

const renderSettingsPage = (visible: boolean) => (
  <div style={{ display: visible ? 'contents' : 'none' }}>
    <div className="page-container">
      <h2>设置</h2>
      <p>偏好设置开发中...</p>
    </div>
  </div>
);

const renderViewsPage = (visible: boolean) => (
  <ViewsPage display={visible}/>
)

const AppContent: React.FC = () => {
  const [currentPage, setCurrentPage] = useState<PageId>('EditorPage');
  const { barContent } = useBar();

  // 导航配置
  const navItems = [
    { id: 'EditorPage' as PageId, name: '编辑器', details: '代码编辑器' },
    { id: 'ViewsPage' as PageId, name: '视图', details: '查看运行结果'},
    { id: 'settings' as PageId, name: '设置', details: '偏好设置' },
  ];

  return (
    <div className="App">
      {/* 顶部导航栏 */}
      <header className="app-header">
        <div className="header-content">
          <nav className="header-nav">
            {navItems.map(item => (
              <button
                key={item.id}
                className={`nav-link ${currentPage === item.id ? 'active' : ''}`}
                onClick={() => setCurrentPage(item.id)}
                title={item.details}
              >
                <span className="nav-name">{item.name}</span>
              </button>
            ))}
          </nav>
        </div>
      </header>
      
      {/* 工具栏区域 */}
      <div className="bar">
        <div className="bar-content">
          {barContent}
        </div>
      </div>
      
      {/* 侧边栏 */}
      <aside className="app-sidebar">
        <div className="sidebar-section">
          <div className="sidebar-title">导航</div>
        </div>
      </aside>
      
      {/* 主内容区 */}
      <main className="main-content">
        <div className="content-container">
          {renderEditorPage(currentPage === 'EditorPage')}
          {renderViewsPage(currentPage === 'ViewsPage')}
          {renderSettingsPage(currentPage === 'settings')}
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