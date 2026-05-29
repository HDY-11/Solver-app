// windows/WindowManager.tsx — 多窗口管理
//
// 监听 PythonEditor 的"分离窗口"事件，尝试创建独立 Tauri WebviewWindow。

import { useEffect } from 'react';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { error as logError, info as logInfo } from '@tauri-apps/plugin-log';
import { useWindow } from '../hooks/useWindow';

async function openDetachedWindow(nodeId: string, label: string): Promise<void> {
  // Tauri 窗口标签仅允许 [a-zA-Z0-9\-/:_]，sanitize 非法字符
  const safeId = nodeId.replace(/[^a-zA-Z0-9\-/:_]/g, '_').slice(0, 40);
  const windowLabel = `editor-${safeId}-${Date.now()}`;

  try {
    const webview = new WebviewWindow(windowLabel, {
      url: `/app/py/${nodeId}`,
      title: `Solver — ${label}`,
      width: 900,
      height: 650,
      minWidth: 500,
      minHeight: 350,
    });

    // WebviewWindow 构造函数不抛异常，通过事件通知结果
    webview.once('tauri://created', () => {
      logInfo(`WindowManager: 分离窗口已创建: ${windowLabel}`);
    });
    webview.once('tauri://error', (e: unknown) => {
      const msg = typeof e === 'string' ? e : JSON.stringify(e);
      logError(`WindowManager: 窗口创建失败: ${msg}`);
      // 通知前端显示 toast
      window.dispatchEvent(new CustomEvent('toast', {
        detail: { type: 'error', message: '分离窗口失败（可能缺少多窗口支持）' },
      }));
    });
  } catch (err) {
    logError(`WindowManager: 创建窗口异常: ${err}`);
    window.dispatchEvent(new CustomEvent('toast', {
      detail: { type: 'error', message: `分离窗口失败: ${err}` },
    }));
  }
}

function WindowManager() {
  const { detachWindow } = useWindow();

  useEffect(() => {
    const handler = (e: Event) => {
      const { nodeId, label } = (e as CustomEvent).detail as {
        nodeId: string; label: string;
      };
      openDetachedWindow(nodeId, label);
      detachWindow(nodeId);
    };

    window.addEventListener('detach-window', handler);
    return () => window.removeEventListener('detach-window', handler);
  }, [detachWindow]);

  return null;
}

export default WindowManager;
