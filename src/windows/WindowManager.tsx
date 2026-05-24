import { useWindow } from '../hooks/useWindow';

function WindowManager() {
  const { detachedWindows } = useWindow();
  // 后续用 Tauri API 创建独立 WebView 窗口
  return null;
}

export default WindowManager;