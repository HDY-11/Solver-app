// panels/SettingPanel.tsx — 应用设置面板
//
// 持久化到 localStorage，管理：编辑器配置、Python 环境信息、VFS 状态。

import { useState, useEffect } from 'react';
import { error as logError } from '@tauri-apps/plugin-log';
import { registerPanel } from '../registry/registry';
import { getInfo } from '../api/vfs';
import { useToast } from '../hooks/useToast';
import type { VfsInfo, Theme } from '../types';
import { fmtSize } from '../types';
import styles from './SettingPanel.module.css';

// =========================================================================
// localStorage 读写工具
// =========================================================================

const STORAGE_KEY = 'solver-settings';

interface AppSettings {
  fontSize: number;
  theme: Theme;
  tabSize: number;
  autoSave: boolean;
}

const DEFAULTS: AppSettings = {
  fontSize: 14,
  theme: 'dark',
  tabSize: 4,
  autoSave: true,
};

function loadSettings(): AppSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? { ...DEFAULTS, ...JSON.parse(raw) } : { ...DEFAULTS };
  } catch {
    return { ...DEFAULTS };
  }
}

function saveSettings(s: AppSettings): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(s));
}

// =========================================================================
// 组件
// =========================================================================

function SettingPanel() {
  const { addToast } = useToast();
  const [settings, setSettings] = useState<AppSettings>(loadSettings);
  const [vfsInfo, setVfsInfo] = useState<VfsInfo | null>(null);

  // 加载 VFS 信息
  useEffect(() => {
    getInfo()
      .then(setVfsInfo)
      .catch((err) => logError(`SettingPanel: 获取 VFS 信息失败: ${err}`));
  }, []);

  const update = (partial: Partial<AppSettings>) => {
    const next = { ...settings, ...partial };
    setSettings(next);
    saveSettings(next);
  };

  const handleReset = () => {
    setSettings({ ...DEFAULTS });
    saveSettings({ ...DEFAULTS });
    addToast('info', '已恢复默认设置');
  };

  return (
    <div className={styles.container}>
      <h2 className={styles.title}>⚙ 设置</h2>

      <section className={styles.section}>
        <h3 className={styles.sectionTitle}>编辑器</h3>
        <div className={styles.row}>
          <label className={styles.label}>字体大小</label>
          <input type="number" min={10} max={24} value={settings.fontSize}
            onChange={(e) => update({ fontSize: Number(e.target.value) })}
            className={styles.input} />
        </div>
        <div className={styles.row}>
          <label className={styles.label}>Tab 大小</label>
          <select value={settings.tabSize}
            onChange={(e) => update({ tabSize: Number(e.target.value) })}
            className={styles.input}>
            {[2, 4, 8].map(n => <option key={n} value={n}>{n}</option>)}
          </select>
        </div>
        <div className={styles.row}>
          <label className={styles.label}>主题</label>
          <select value={settings.theme}
            onChange={(e) => update({ theme: e.target.value as Theme })}
            className={styles.input}>
            <option value="dark">深色</option>
            <option value="light">亮色</option>
          </select>
        </div>
        <div className={styles.row}>
          <label className={styles.label}>自动保存</label>
          <input type="checkbox" checked={settings.autoSave}
            onChange={(e) => update({ autoSave: e.target.checked })} />
        </div>
      </section>

      <section className={styles.section}>
        <h3 className={styles.sectionTitle}>Python 环境</h3>
        <div className={styles.row}>
          <label className={styles.label}>版本</label>
          <span className={styles.valueMono}>Python 3.12 (嵌入)</span>
        </div>
        <div className={styles.row}>
          <label className={styles.label}>脚本路径</label>
          <span className={styles.valueMonoSm}>scripts/</span>
        </div>
      </section>

      <section className={styles.section}>
        <h3 className={styles.sectionTitle}>VFS 存储</h3>
        {vfsInfo ? (<>
          <div className={styles.row}>
            <label className={styles.label}>C 盘状态</label>
            <span className={styles.value}>{vfsInfo.c_exists ? '✅ 正常' : '❌ 不存在'}</span>
          </div>
          <div className={styles.row}>
            <label className={styles.label}>节点数</label>
            <span className={styles.valueMono}>{vfsInfo.c_node_count}</span>
          </div>
          <div className={styles.row}>
            <label className={styles.label}>已用空间</label>
            <span className={styles.valueMono}>
              {fmtSize(vfsInfo.c_used)} / {fmtSize(vfsInfo.c_total)}
            </span>
          </div>
          <div className={styles.progressBar}>
            <div className={`${styles.progressFill} ${vfsInfo.c_used / vfsInfo.c_total > 0.9 ? styles.progressFillDanger : styles.progressFillSafe}`}
              style={{ width: `${((vfsInfo.c_used / vfsInfo.c_total) * 100).toFixed(1)}%` }} />
          </div>
        </>) : (
          <p className={styles.errorHint}>无法获取 VFS 信息</p>
        )}
      </section>

      <button className="btn btn-sm" onClick={handleReset}>
        🔄 恢复默认设置
      </button>
    </div>
  );
}

// ── 注册 ──────────────────────────────────────

registerPanel({
  name: 'setting',
  component: SettingPanel,
  label: '设置',
});

export default SettingPanel;
