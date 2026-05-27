import { BrowserRouter } from 'react-router-dom';
import { WindowProvider } from './hooks/useWindow';
import { ToastProvider } from './hooks/useToast';
import { TabsProvider } from './hooks/useTabs';
import ToastContainer from './components/Toast';
import ShortcutHelp from './components/ShortcutHelp';
import Header from './layouts/Header';
import Toolbar from './layouts/Toolbar';
import NavBar, { useNavMode } from './layouts/NavBar';
import Sidebar from './layouts/Sidebar';
import Nav from './layouts/Nav';
import Main from './layouts/Main';
import Footer from './layouts/Footer';
import WindowManager from './windows/WindowManager';

// 导入即注册
import './renderers/PythonEditor';
import './renderers/HtmlViewer';
import './renderers/TextViewer';
import './renderers/RunResult';
import './panels/SettingPanel';

import './App.css';

function App() {
  const [navMode, setNavMode] = useNavMode();

  return (
    <BrowserRouter>
      <ToastProvider>
        <TabsProvider>
        <WindowProvider>
          <div className="App">
            <NavBar mode={navMode} onChange={setNavMode} />
            <Header />
            <Toolbar />
            <Sidebar mode={navMode} />
            <Nav />
            <Main />
            <Footer />
          </div>
          <WindowManager />
          <ToastContainer />
          <ShortcutHelp />
        </WindowProvider>
        </TabsProvider>
      </ToastProvider>
    </BrowserRouter>
  );
}

export default App;