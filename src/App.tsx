import { useEffect, useState, Component } from 'react';
import { BrowserRouter, useNavigate } from 'react-router-dom';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
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
import { error as logError, info as logInfo } from '@tauri-apps/plugin-log';

// 导入即注册
import './renderers/PythonEditor';
import './renderers/HtmlViewer';
import './renderers/TextViewer';
import './renderers/RunResult';
import './panels/SettingPanel';

import './App.css';

// ── 错误边界（防止未捕获错误导致白屏）──

class ErrorBoundary extends Component<{ children: React.ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };
  static getDerivedStateFromError(e: Error) { return { error: e }; }
  render() {
    if (this.state.error) {
      return <div style={{ padding: 40, color: 'red', fontFamily: 'monospace' }}>
        <h2>应用出错</h2><pre>{this.state.error.message}</pre>
      </div>;
    }
    return this.props.children;
  }
}

// ── 分离窗口检测 ──

/** 检测分离窗口（不依赖 Tauri API，避免渲染期间 IPC 调用导致白屏） */
function isDetachedWindow(): boolean {
  try {
    // 优先检查 label（如果 getCurrentWindow 可用）
    if (getCurrentWindow().label.startsWith('detached-')) return true;
  } catch {
     /* API 不可用时回退 */ 
     logError('[isDetachedWindow]: getCurrentWindow 失败，回退');
  }
  // 回退：initialization_script 注入的全局变量
  return !!(window as any).__DETACH_ROUTE__;
}

function getDetachRoute(): string | null {
  return (window as any).__DETACH_ROUTE__ || null;
}

// ── 分离窗口路由恢复 ──

function DetachedRouteHandler() {
  const navigate = useNavigate();
  useEffect(() => {
    // 优先从后端状态表获取（比 initialization_script 可靠）
    invoke<string>('get_detach_route')
      .then(route => { navigate(route, { replace: true }); })
      .catch(() => {
        // 回退到全局变量
        const route = getDetachRoute();
        if (route) {
          delete (window as any).__DETACH_ROUTE__;
          navigate(route, { replace: true });
        }
      });
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

// ── 分离窗口拖拽合并处理器 ──

function DragMergeHandler() {
  const { tabs, closeTab } = useTabs();
  const [isDetached, setIsDetached] = useState(false);
  const [isMerging, setIsMerging] = useState(false);

  // 延迟检测（避免 React 首次渲染时 Tauri API 不可用）
  useEffect(() => {
    try {
      const label = getCurrentWindow().label;
      logInfo(`[merge] 窗口 label: ${label}`);
      setIsDetached(label.startsWith('detached-'));
    }
    catch (e) {
      logError('[merge] getCurrentWindow 失败:', e);
      setIsDetached(!!(window as any).__DETACH_ROUTE__);
    }
  }, []);

  // 监听 drag-release 事件 → 发起合并
  useEffect(() => {
    if (!isDetached) return;
    let unlisten: (() => void) | null = null;
    listen('drag-release', () => {
      logInfo('[merge] drag-release 收到');
      if (tabs.length === 0) { logInfo('[merge] 无标签，跳过'); return; }
      const tab = tabs[0];
      logInfo(`[merge] 合并标签: ${tab.path}`);
      invoke('emit_merge_request', { path: tab.path, label: tab.label, icon: tab.icon })
        .then(() => {
          logInfo('[merge] 合并成功，播放关闭动画');
          closeTab(tab.path);
          setIsMerging(true);
          // 动画持续 350ms 后关闭窗口
          setTimeout(() => getCurrentWindow().close(), 350);
        })
        .catch(err => logError('[merge] 拖拽合并失败:', err));
    }).then(fn => { unlisten = fn; logInfo('[merge] drag-release 监听已注册'); });
    return () => { unlisten?.(); };
  }, [isDetached, tabs, closeTab]);

  // 绑定标题栏拖拽事件（仅实际拖拽>30px才启动钩子，避免点击误触）
  useEffect(() => {
    if (!isDetached) return;
    const el = document.querySelector('[data-tauri-drag-region]');
    if (!el) { logError('[merge] 未找到 data-tauri-drag-region 元素'); return; }
    logInfo('[merge] 标题栏拖拽监听已绑定（阈值 30px）');

    let startX = 0, startY = 0;
    let hookStarted = false;
    const THRESHOLD = 30;

    const onDown = (e: MouseEvent) => {
      startX = e.screenX;
      startY = e.screenY;
      hookStarted = false;
      logInfo(`[merge] mousedown (${startX}, ${startY})`);
    };

    const onMove = (e: MouseEvent) => {
      if (hookStarted) return;
      const dx = e.screenX - startX;
      const dy = e.screenY - startY;
      if (dx * dx + dy * dy > THRESHOLD * THRESHOLD) {
        hookStarted = true;
        logInfo(`[merge] 拖拽超过阈值 → start_drag_track`);
        invoke('start_drag_track', { tabPath: '', tabLabel: '', devicePixelRatio: window.devicePixelRatio }).catch(e => logError('[merge] start_drag_track 失败:', e));
      }
    };

    const onUp = () => {
      if (hookStarted) {
        logInfo('[merge] mouseup → stop_drag_track');
        invoke('stop_drag_track').catch(e => logError('[merge] stop_drag_track 失败:', e));
      } else {
        logInfo('[merge] mouseup（未超过阈值，忽略）');
      }
      startX = startY = 0;
      hookStarted = false;
    };

    el.addEventListener('mousedown', onDown);
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
    return () => {
      el.removeEventListener('mousedown', onDown);
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
    };
  }, [isDetached]);

  if (!isDetached) return null;
  return (
    <>
      {/* 合并中：暗色遮罩 + 窗口缩小 */}
      {isMerging && (
        <div className="merge-close-overlay">
          <div className="merge-close-frame" />
        </div>
      )}
    </>
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
          <DragMergeHandler />
          <div style={{ display: 'flex', flexDirection: 'column', height: '100vh' }}>
            <DetachedTitlebar />
            <Toolbar />
            <Nav detached />
            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
              <Main />
            </div>
          </div>
        </TabsProvider>
        </SettingsProvider>
        <ToastContainer />
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
        logInfo(`[merge] 主窗口收到合并事件:, ${JSON.stringify(event.payload)}`);
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
  const [navMode, setNavMode] = useNavMode();

  // React 挂载完成 → 通知后端进度 100% + 关闭加载屏
  useEffect(() => {
    invoke('frontend_ready').catch(() => {});
    const close = (window as any).__closeSplash as (() => void) | undefined;
    if (close) close();
  }, []);

  // 注册编辑器命令和快捷键
  useEffect(() => {
    const unreg = registerEditorCommands();
    return unreg;
  }, []);

  if (isDetached) return <ErrorBoundary><DetachedApp /></ErrorBoundary>;

  return (
    <ErrorBoundary>
    <BrowserRouter>
      <DetachedRouteHandler />
      <ToastProvider>
        <SettingsProvider>
        <TabsProvider>
          <DragMergeHandler />
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
    </ErrorBoundary>
  );
}

export default App;