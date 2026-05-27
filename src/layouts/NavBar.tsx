// layouts/NavBar.tsx — 左侧导航栏
//
// 竖向图标导航：资源管理器 / 运行结果管理器

import { useState, useCallback } from 'react';

export type NavMode = 'files' | 'runs';

interface NavBarProps {
  mode: NavMode;
  onChange: (mode: NavMode) => void;
}

const ITEMS: { id: NavMode; icon: string; label: string }[] = [
  { id: 'files', icon: '📁', label: '资源管理器' },
  { id: 'runs', icon: '📊', label: '运行结果' },
];

function NavBar({ mode, onChange }: NavBarProps) {
  return (
    <nav className="app-navbar">
      {ITEMS.map((item) => (
        <button
          key={item.id}
          className={`navbar-btn ${mode === item.id ? 'navbar-btn--active' : ''}`}
          title={item.label}
          onClick={() => onChange(item.id)}
        >
          <span className="navbar-btn__icon">{item.icon}</span>
        </button>
      ))}
    </nav>
  );
}

/** Hook：共享导航模式状态 */
export function useNavMode() {
  return useState<NavMode>('files');
}

export default NavBar;
