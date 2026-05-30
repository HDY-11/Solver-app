// panels/SettingPanel.tsx — 应用设置面板
//
// 使用 SettingsContext 读写配置，修改先存入草稿，确认后持久化到磁盘。
// 影响全局行为（主题、编辑器字体/Tab 等）。

import { useState, useEffect, useCallback } from 'react';
import { error as logError } from '@tauri-apps/plugin-log';
import { registerPanel } from '../registry/registry';
import { Icon } from '../utils/icons';
import { getInfo } from '../api/vfs';
import { useSettings } from '../hooks/useSettings';
import type { AppSettings } from '../api/config';
import { DEFAULT_SETTINGS } from '../api/config';
import { useToast } from '../hooks/useToast';
import type { VfsInfo } from '../types';
import { fmtSize } from '../types';
import styles from './SettingPanel.module.css';

// =========================================================================
// 组件
// =========================================================================

function SettingPanel() {
  const { addToast } = useToast();
  const { settings: saved, update } = useSettings();
  const [vfsInfo, setVfsInfo] = useState<VfsInfo | null>(null);

  // 草稿：当前编辑中的值（未确认）
  const [draft, setDraft] = useState<AppSettings>(saved);
  const [dirty, setDirty] = useState(false);

  // 外部配置变更时同步到草稿（例如 Footer 切换主题）
  useEffect(() => {
    if (!dirty) setDraft(saved);
  }, [saved, dirty]);

  // 加载 VFS 信息
  useEffect(() => {
    getInfo()
      .then(setVfsInfo)
      .catch((err) => logError(`SettingPanel: 获取 VFS 信息失败: ${err}`));
  }, []);

  // 修改草稿
  const patch = useCallback((partial: Partial<AppSettings>) => {
    setDraft(prev => ({ ...prev, ...partial }));
    setDirty(true);
  }, []);

  // 确认 → 写入后端 + 通知全局
  const handleConfirm = useCallback(async () => {
    try {
      await update(draft);
      setDirty(false);
      addToast('success', '设置已保存');
    } catch (err) {
      addToast('error', `保存失败: ${err}`);
    }
  }, [draft, update, addToast]);

  // 取消 → 回退到已保存的值
  const handleCancel = useCallback(() => {
    setDraft(saved);
    setDirty(false);
  }, [saved]);

  // 重置 → 恢复默认配置（需确认）
  const handleReset = useCallback(async () => {
    try {
      const def = { ...DEFAULT_SETTINGS };
      await update(def);
      setDraft(def);
      setDirty(false);
      addToast('info', '已恢复默认设置');
    } catch (err) {
      addToast('error', `重置失败: ${err}`);
    }
  }, [update, addToast]);

  return (
    <div className={styles.container}>
      <h2 className={styles.title}><Icon icon="gear" /> 设置</h2>

      {/* ── 确认/取消操作栏 ── */}
      {dirty && (
        <div style={{ display: 'flex', gap: 8, marginBottom: 16 }}>
          <button className="btn btn-primary btn-sm" onClick={handleConfirm}>
            <Icon icon="check" /> 确认修改
          </button>
          <button className="btn btn-sm" onClick={handleCancel}>
            <Icon icon="xmark" /> 取消
          </button>
        </div>
      )}

      <section className={styles.section}>
        <h3 className={styles.sectionTitle}>编辑器</h3>
        <div className={styles.row}>
          <label className={styles.label}>字体大小</label>
          <input type="number" min={10} max={24} value={draft.font_size}
            onChange={(e) => patch({ font_size: Number(e.target.value) })}
            className={styles.input} />
        </div>
        <div className={styles.row}>
          <label className={styles.label}>Tab 大小</label>
          <select value={draft.tab_size}
            onChange={(e) => patch({ tab_size: Number(e.target.value) })}
            className={styles.input}>
            {[2, 4, 8].map(n => <option key={n} value={n}>{n}</option>)}
          </select>
        </div>
        <div className={styles.row}>
          <label className={styles.label}>主题</label>
          <select value={draft.theme}
            onChange={(e) => patch({ theme: e.target.value as AppSettings['theme'] })}
            className={styles.input}>
            <option value="dark">深色</option>
            <option value="light">亮色</option>
          </select>
        </div>
        <div className={styles.row}>
          <label className={styles.label}>自动保存</label>
          <input type="checkbox" checked={draft.auto_save}
            onChange={(e) => patch({ auto_save: e.target.checked })} />
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
            <span className={styles.value}>{vfsInfo.c_exists ? <><Icon icon="success" /> 正常</> : <><Icon icon="error" /> 不存在</>}</span>
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
        <Icon icon="rotate" /> 恢复默认设置
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
