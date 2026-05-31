// hooks/useToast.tsx — 全局 Toast 通知管理
//
// 提供 toast 通知的全局状态，组件通过 useToast() 获取 addToast 方法。
// Toast 组件渲染在 App 顶层，通过 ToastProvider 注入。

import { createContext, useContext, useState, useCallback, useRef, type ReactNode } from 'react';

// =========================================================================
// 类型
// =========================================================================

export type ToastType = 'success' | 'error' | 'warning' | 'info';

export interface Toast {
  id: number;
  type: ToastType;
  message: string;
  /** 自动消失时间（ms），0 表示不自动消失 */
  duration: number;
}

interface ToastContextValue {
  toasts: Toast[];
  addToast: (type: ToastType, message: string, duration?: number) => void;
  removeToast: (id: number) => void;
}

// =========================================================================
// Context
// =========================================================================

const ToastContext = createContext<ToastContextValue | null>(null);

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(0);  // useRef 避免 HMR 时 ID 碰撞

  const removeToast = useCallback((id: number) => {
    setToasts(prev => prev.filter(t => t.id !== id));
  }, []);

  const addToast = useCallback((type: ToastType, message: string, duration = 3000) => {
    const id = ++nextId.current;
    setToasts(prev => [...prev, { id, type, message, duration }]);
    // 自动消失
    if (duration > 0) {
      setTimeout(() => removeToast(id), duration);
    }
  }, [removeToast]);

  return (
    <ToastContext.Provider value={{ toasts, addToast, removeToast }}>
      {children}
    </ToastContext.Provider>
  );
}

/**
 * 获取 toast 操作方法。
 * 必须在 ToastProvider 内使用。
 */
export function useToast(): Omit<ToastContextValue, 'toasts'> {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error('useToast 必须在 ToastProvider 内使用');
  return ctx;
}

/**
 * 获取当前 toast 列表（供 Toast 渲染组件使用）。
 */
export function useToastList(): Toast[] {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error('useToastList 必须在 ToastProvider 内使用');
  return ctx.toasts;
}
