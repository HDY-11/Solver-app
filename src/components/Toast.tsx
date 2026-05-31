// components/Toast.tsx — Toast 通知渲染组件
//
// 渲染在页面右下角的 toast 通知栈。每个 toast 有类型图标和关闭按钮。
// 自动从 ToastProvider 读取 toast 列表。

import { useToastList, useToast } from '../hooks/useToast';
import { Icon } from '../utils/icons';
import type { ToastType } from '../hooks/useToast';

// =========================================================================
// 图标映射
// =========================================================================

const TYPE_ICON: Record<ToastType, string> = {
  success: 'success',
  error: 'error',
  warning: 'warning',
  info: 'info',
};

const TYPE_CLASS: Record<ToastType, string> = {
  success: 'toast--success',
  error: 'toast--error',
  warning: 'toast--warning',
  info: 'toast--info',
};

// =========================================================================
// 组件
// =========================================================================

function ToastContainer() {
  const toasts = useToastList();
  const { removeToast } = useToast();

  if (toasts.length === 0) return null;

  return (
    <div className="toast-container" role="alert" aria-live="polite">
      {toasts.map(toast => (
        <div
          key={toast.id}
          className={`toast ${TYPE_CLASS[toast.type]}`}
          onClick={() => removeToast(toast.id)}
        >
          <span className="toast__icon"><Icon icon={TYPE_ICON[toast.type]} /></span>
          <span className="toast__message">{toast.message}</span>
          <button
            className="toast__close"
            onClick={(e) => { e.stopPropagation(); removeToast(toast.id); }}
            aria-label="关闭通知"
          >
            <Icon icon="xmark" />
          </button>
        </div>
      ))}
    </div>
  );
}

export default ToastContainer;
