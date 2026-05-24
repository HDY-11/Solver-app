import { BrowserRouter } from 'react-router-dom';
import { WindowProvider } from './hooks/useWindow';
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
import './panels/SettingPanel';

import './App.css';

import { getRenderer } from './registry/registry.ts';
console.log('已注册渲染器:', getRenderer('py'));

function App() {
  return (
    <BrowserRouter>
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
      </WindowProvider>
    </BrowserRouter>
  );
}

export default App;