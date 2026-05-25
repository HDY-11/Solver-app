// windows/WindowManager.tsx — 多窗口管理
//
// 管理"分离窗口"功能：将当前编辑器内容在新的独立窗口中打开。
// 使用 Tauri v2 WebviewWindow API 创建独立窗口。

import { useEffect } from 'react';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { error as logError, info as logInfo } from '@tauri-apps/plugin-log';
import { useWindow } from '../hooks/useWindow';

/**
 * 创建独立窗口显示指定 VFS 文件。
 * 新窗口加载相同前端，通过 URL 参数 `/app/window/py/{nodeId}` 进入编辑器。
 */
async function openDetachedWindow(nodeId: string, label: string): Promise<void> {
  // 窗口标签必须唯一，用 nodeId 和时间戳组合
  const windowLabel = `editor-${nodeId}-${Date.now()}`;

  try {
    const webview = new WebviewWindow(windowLabel, {
      url: `/app/window/py/${nodeId}`,
      title: `Solver — ${label}`,
      width: 900,
      height: 650,
      minWidth: 500,
      minHeight: 350,
      // 复用已安装的插件权限
    });

    // 等待窗口创建完成
    await webview.once('tauri://created', () => {
      logInfo(`WindowManager: 分离窗口已创建: ${windowLabel}`);
    });

    webview.once('tauri://error', (e) => {
      logError(`WindowManager: 窗口创建失败: ${e}`);
    });
  } catch (err) {
    logError(`WindowManager: 创建窗口失败: ${err}`);
  }
}

function WindowManager() {
  const { detachWindow } = useWindow();

  // 监听自定义事件，由 PythonEditor toolbar 触发
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

  return null; // 纯逻辑组件，无 UI
}

export default WindowManager;
