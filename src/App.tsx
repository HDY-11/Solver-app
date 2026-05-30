import { useEffect, useState } from 'react';
import { BrowserRouter, useNavigate } from 'react-router-dom';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Icon } from './utils/icons';
import { ToastProvider } from './hooks/useToast';
import { TabsProvider, useTabs } from './hooks/useTabs';
import { SettingsProvider } from './hooks/useSettings';
import ToastContainer from './components/Toast';
import ShortcutHelp from './components/ShortcutHelp';
import Header from './layouts/Header';
import Toolbar from './layouts/Toolbar';
import NavBar, { useNavMode } from './layouts/NavBar';
import Sidebar from './layouts/Sidebar';
import Nav from './layouts/Nav';
import Main from './layouts/Main';
import Footer from './layouts/Footer';
import { registerEditorCommands } from './commands/editorCommands';

// 导入即注册
import './renderers/PythonEditor';
import './renderers/HtmlViewer';
import './renderers/TextViewer';
import './renderers/RunResult';
import './panels/SettingPanel';

import './App.css';

// ── 分离窗口检测 ──

/** 通过 initialization_script 注入的全局变量检测分离窗口 */
function isDetachedWindow(): boolean {
  return !!(window as any).__DETACH_ROUTE__;
}

function getDetachedRoute(): string | null {
  return (window as any).__DETACH_ROUTE__ || null;
}

// ── 分离窗口路由恢复 ──

function DetachedRouteHandler() {
  const navigate = useNavigate();
  useEffect(() => {
    const route = getDetachedRoute();
    if (route) {
      delete (window as any).__DETACH_ROUTE__;
      navigate(route, { replace: true });
    }
  }, [navigate]);
  return null;
}

// ── 分离窗口标题栏 ──

function DetachedTitlebar() {
  const appWindow = getCurrentWindow();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    appWindow.isMaximized().then(setMaximized);
  }, [appWindow]);

  const handleMaximize = async () => {
    await appWindow.toggleMaximize();
    setMaximized(await appWindow.isMaximized());
  };

  return (
    <div
      style={{
        display: 'flex', alignItems: 'center', height: 32,
        background: 'var(--gray-200)', borderBottom: '1px solid var(--gray-300)',
        userSelect: 'none', flexShrink: 0,
      }}
      data-tauri-drag-region
    >
      <span style={{ padding: '0 12px', fontSize: '0.75rem', color: 'var(--gray-600)' }}>
        Solver
      </span>
      <div style={{ marginLeft: 'auto', display: 'flex', WebkitAppRegion: 'no-drag' } as React.CSSProperties}>
        <button className="titlebar-btn" onClick={() => appWindow.minimize()}><Icon icon="minus" /></button>
        <button className="titlebar-btn" onClick={handleMaximize}><Icon icon={maximized ? 'maximize' : 'square'} /></button>
        <button className="titlebar-btn titlebar-close" onClick={() => appWindow.close()}><Icon icon="xmark" /></button>
      </div>
    </div>
  );
}

// ── 分离窗口布局 ──

function DetachedApp() {
  useEffect(() => {
    const unreg = registerEditorCommands();
    return unreg;
  }, []);

  return (
    <BrowserRouter>
      <DetachedRouteHandler />
      <ToastProvider>
        <SettingsProvider>
        <TabsProvider>
          <div style={{ display: 'flex', flexDirection: 'column', height: '100vh' }}>
            <DetachedTitlebar />
            <Toolbar />
            <Nav detached />
            <Main />
          </div>
        </TabsProvider>
        </SettingsProvider>
      </ToastProvider>
    </BrowserRouter>
  );
}

// ── 合并监听器（主窗口） ──

interface MergePayload { path: string; label: string; icon: string; }

function MergeListenerWrapper() {
  const navigate = useNavigate();
  const { registerTab } = useTabs();

  useEffect(() => {
    const setup = async () => {
      const unlisten = await listen<MergePayload>('merge-request', (event) => {
        const { path, label, icon } = event.payload;
        registerTab({ path, label, icon });
        navigate(path);
      });
      return unlisten;
    };
    let unlistenFn: (() => void) | null = null;
    setup().then(fn => { unlistenFn = fn; });
    return () => { unlistenFn?.(); };
  }, [navigate, registerTab]);

  return null;
}

function App() {
  const [isDetached] = useState(() => isDetachedWindow());
  // hooks 必须在条件分支前调用（React 规则）
  const [navMode, setNavMode] = useNavMode();

  if (isDetached) return <DetachedApp />;

  // 启动时注册编辑器命令（全局一次，对标 VS Code contribution）
  useEffect(() => {
    const unreg = registerEditorCommands();
    return unreg;
  }, []);

  return (
    <BrowserRouter>
      <DetachedRouteHandler />
      <ToastProvider>
        <SettingsProvider>
        <TabsProvider>
          <MergeListenerWrapper />
          <div className="App">
            <NavBar mode={navMode} onChange={setNavMode} />
            <Header />
            <Toolbar />
            <Sidebar mode={navMode} />
            <Nav />
            <Main />
            <Footer />
          </div>
          <ToastContainer />
          <ShortcutHelp />
        </TabsProvider>
        </SettingsProvider>
      </ToastProvider>
    </BrowserRouter>
  );
}

export default App;