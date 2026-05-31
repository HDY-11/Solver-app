// components/Loading.tsx — 加载与空状态组件
//
// 提供两种形式：
// - Loading: 全屏或内联 spinner，带可选提示文字
// - EmptyState: 空数据占位，显示图标和提示

import type { ReactNode } from 'react';

// =========================================================================
// Loading — 加载中旋转指示器
// =========================================================================

interface LoadingProps {
  /** 内联模式：紧凑行内显示；否则为全屏居中 */
  inline?: boolean;
  /** 提示文字 */
  text?: string;
  /** 尺寸 */
  size?: number;
}

function Loading({ inline = false, text, size = 24 }: LoadingProps) {
  const spinner = (
    <span
      className="loading-spinner"
      style={{ width: size, height: size }}
      aria-label="加载中"
    />
  );

  if (inline) {
    return (
      <span className="loading-inline">
        {spinner}
        {text && <span className="loading-inline__text">{text}</span>}
      </span>
    );
  }

  return (
    <div className="loading-full">
      {spinner}
      {text && <p className="loading-full__text">{text}</p>}
    </div>
  );
}

// =========================================================================
// EmptyState — 空数据占位
// =========================================================================

interface EmptyStateProps {
  icon?: string;
  title: string;
  description?: string;
  action?: ReactNode;
}

function EmptyState({ icon = '📭', title, description, action }: EmptyStateProps) {
  return (
    <div className="empty-state">
      <span className="empty-state__icon">{icon}</span>
      <h3 className="empty-state__title">{title}</h3>
      {description && <p className="empty-state__desc">{description}</p>}
      {action && <div className="empty-state__action">{action}</div>}
    </div>
  );
}

// =========================================================================
// CSS（通过 App.css 中的类名驱动）
// =========================================================================

export { Loading, EmptyState };
