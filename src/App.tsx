import { BrowserRouter } from 'react-router-dom';
import { WindowProvider } from './hooks/useWindow';
import { ToastProvider } from './hooks/useToast';
import ToastContainer from './components/Toast';
import ShortcutHelp from './components/ShortcutHelp';
import Header from './layouts/Header';
import Toolbar from './layouts/Toolbar';
import Sidebar from './layouts/Sidebar';
import Nav from './layouts/Nav';
import Main from './layouts/Main';
import Footer from './layouts/Footer';
import WindowManager from './windows/WindowManager';

// 导入即注册
import './renderers/PythonEditor';
import './renderers/HtmlViewer';
import './renderers/TextViewer';
import './panels/SettingPanel';
import './pages/ViewsPage';

import './App.css';

import { getRenderer } from './registry/registry.ts';
console.log('已注册渲染器:', getRenderer('py'));

function App() {
  return (
    <BrowserRouter>
      <ToastProvider>
        <WindowProvider>
          <div className="App">
            <Header />
            <Toolbar />
            <Sidebar />
            <Nav />
            <Main />
            <Footer />
          </div>
          <WindowManager />
          <ToastContainer />
          <ShortcutHelp />
        </WindowProvider>
      </ToastProvider>
    </BrowserRouter>
  );
}

export default App;