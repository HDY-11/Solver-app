// components/TimelinePanel.tsx — 版本时间线面板（R7）
//
// 显示在 Sidebar 下半部分。自动随当前打开文件切换版本列表。
// 点击版本条目可恢复历史版本内容。
//
// R7: 适配 .cmdv 文件版本历史。
// - getCurrentPath 已正确处理 /app/cmdv/(vfs)/C/test.cmdv 格式 URL
// - VFS 版本记录对所有 node_type 一视同仁（包括 node_type='run' 的 .cmdv）
// - 恢复版本后发射 'vfs:file-changed' 事件通知编辑器刷新

import { useState, useEffect, useCallback } from 'react';
import { useLocation } from 'react-router-dom';
import { error as logError } from '@tauri-apps/plugin-log';
import { listVersions, readVersion, writeFile } from '../api/vfs';
import { Icon } from '../utils/icons';
import { useToast } from '../hooks/useToast';
import { fmtSize } from '../types';
import type { VfsVersion } from '../types';

/** 从 URL 解析当前文件的 VFS 路径。
 *
 *  URL 格式: /app/{renderer}/{encodedVfsPath}
 *  示例: /app/cmdv/(vfs)/C/test.cmdv
 *      → parts = ['app', 'cmdv', '(vfs)', 'C', 'test.cmdv']
 *      → encoded = parts.slice(2).join('/') = '(vfs)/C/test.cmdv'
 *      → decodeURIComponent → '(vfs)/C/test.cmdv'
 */
function getCurrentPath(pathname: string): string | null {
  const parts = pathname.split('/').filter(Boolean);
  if (parts.length >= 3 && parts[0] === 'app') {
    const encoded = parts.slice(2).join('/');
    try { return decodeURIComponent(encoded); } catch { return null; }
  }
  return null;
}

/** 截短哈希显示（前 8 位） */
function shortHash(hash: string): string {
  return hash.slice(0, 8);
}

function TimelinePanel() {
  const { pathname } = useLocation();
  const { addToast } = useToast();
  const [versions, setVersions] = useState<VfsVersion[]>([]);
  const [loading, setLoading] = useState(false);
  const [restoring, setRestoring] = useState<string | null>(null);
  const [activeHash, setActiveHash] = useState<string | null>(null);

  const currentPath = getCurrentPath(pathname);

  // 路径变化时加载版本列表
  useEffect(() => {
    if (!currentPath) {
      setVersions([]);
      setActiveHash(null);
      return;
    }
    setLoading(true);
    listVersions(currentPath)
      .then((v) => {
        setVersions(v);
        if (v.length > 0) setActiveHash(v[0].content_hash);
      })
      .catch((err) => {
        logError(`TimelinePanel: 加载版本失败: ${err}`);
        setVersions([]);
      })
      .finally(() => setLoading(false));
  }, [currentPath]);

  // 恢复到指定版本
  const handleRestore = useCallback(async (version: VfsVersion) => {
    if (!currentPath) return;
    setRestoring(version.content_hash);
    try {
      const content = await readVersion(currentPath, version.content_hash);
      await writeFile(currentPath, content);
      addToast('success', `已恢复版本 ${shortHash(version.content_hash)}`);
      // 刷新列表 + 标记当前活跃版本
      const updated = await listVersions(currentPath);
      setVersions(updated);
      setActiveHash(version.content_hash);
      // 通知编辑器刷新内容
      window.dispatchEvent(new CustomEvent('vfs:file-changed', {
        detail: { path: currentPath },
      }));
    } catch (err) {
      logError(`TimelinePanel: 恢复版本失败: ${err}`);
      addToast('error', `恢复失败: ${err}`);
    } finally {
      setRestoring(null);
    }
  }, [currentPath, addToast]);

  if (!currentPath) return null;

  return (
    <div className="timeline-panel">
      <div className="timeline-panel__header">
        <span className="timeline-panel__title">📋 时间线</span>
        {loading && <span className="timeline-panel__loading">加载中...</span>}
      </div>

      {versions.length === 0 ? (
        <p className="timeline-panel__empty">
          {loading ? '加载中...' : '暂无历史版本'}
        </p>
      ) : (
        <div className="timeline-panel__list">
          {versions.map((v) => {
            const isActive = v.content_hash === activeHash;
            return (
              <div
                key={v.content_hash}
                className={`timeline-item ${isActive ? 'timeline-item--latest' : ''}`}
              >
                <span className="timeline-item__dot">
                  <Icon icon={isActive ? 'circle' : 'circle'} />
                </span>
                <div className="timeline-item__info">
                  <span className="timeline-item__time">
                    {v.created_at.replace('T', ' ').slice(0, 19)}
                  </span>
                  <span className="timeline-item__size">{fmtSize(v.size)}</span>
                  <span className="timeline-item__hash" title={v.content_hash}>
                    #{shortHash(v.content_hash)}
                  </span>
                </div>
                {!isActive && (
                  <button
                    className="timeline-item__restore icon-btn"
                    title="恢复此版本"
                    disabled={restoring === v.content_hash}
                    onClick={() => handleRestore(v)}
                  >
                    <Icon icon={restoring === v.content_hash ? 'spinner' : 'restore'} />
                  </button>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

export default TimelinePanel;
