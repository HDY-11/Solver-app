// components/ShortcutHelp.tsx — 快捷键帮助面板
//
// 按 ? 键弹出，显示所有可用快捷键。

import { useEffect, useState } from 'react';

interface Shortcut {
  keys: string;
  desc: string;
}

const SHORTCUTS: Shortcut[] = [
  { keys: 'Ctrl + S', desc: '保存当前文件' },
  { keys: 'Ctrl + Enter', desc: '运行当前脚本' },
  { keys: 'Ctrl + N', desc: '新建文件（侧边栏）' },
  { keys: '?', desc: '显示/隐藏此帮助' },
  { keys: 'Esc', desc: '关闭面板' },
];

function ShortcutHelp() {
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // 在输入框内不触发快捷键
      const tag = (e.target as HTMLElement).tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA') return;

      if (e.key === '?') {
        e.preventDefault();
        setOpen((prev) => !prev);
      }
      if (e.key === 'Escape' && open) {
        setOpen(false);
      }
    };

    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [open]);

  if (!open) return null;

  return (
    <div className="confirm-overlay" onClick={() => setOpen(false)}>
      <div className="shortcut-panel" onClick={(e) => e.stopPropagation()}>
        <div className="shortcut-panel__header">
          <h3>⌨ 快捷键</h3>
          <button
            className="icon-btn"
            onClick={() => setOpen(false)}
            aria-label="关闭"
          >
            ✕
          </button>
        </div>
        <div className="shortcut-panel__list">
          {SHORTCUTS.map((s) => (
            <div key={s.keys} className="shortcut-item">
              <kbd className="shortcut-item__keys">{s.keys}</kbd>
              <span className="shortcut-item__desc">{s.desc}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

export default ShortcutHelp;
