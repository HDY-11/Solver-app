// components/TimelinePanel.tsx — 版本时间线面板
//
// 显示在 Sidebar 下半部分。自动随当前打开文件切换版本列表。
// 点击版本条目可恢复历史版本内容。

import { useState, useEffect, useCallback } from 'react';
import { useLocation } from 'react-router-dom';
import { error as logError } from '@tauri-apps/plugin-log';
import { listVersions, readVersion, writeFile } from '../api/vfs';
import { useToast } from '../hooks/useToast';
import { fmtSize } from '../types';
import type { VfsVersion } from '../types';

/** 从 URL 解析当前文件的 VFS 路径 */
function getCurrentPath(pathname: string): string | null {
  const parts = pathname.split('/').filter(Boolean);
  // /app/py/(vfs)/C/脚本/test.py → parts = ['app', 'py', '(vfs)', 'C', '脚本', 'test.py']
  if (parts.length >= 3 && parts[0] === 'app') {
    // parts[1] 是 renderer 名, parts[2..] 是 VFS 路径的编码片段
    const encoded = parts.slice(2).join('/');
    try { return decodeURIComponent(encoded); } catch { return null; }
  }
  return null;
}

/** 截短哈希显示 */
function shortHash(hash: string): string {
  return hash.slice(0, 8);
}

function TimelinePanel() {
  const { pathname } = useLocation();
  const { addToast } = useToast();
  const [versions, setVersions] = useState<VfsVersion[]>([]);
  const [loading, setLoading] = useState(false);
  const [restoring, setRestoring] = useState<string | null>(null);
  // 当前活跃版本（编辑器中显示的那个），默认为最新
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
        // 最新版本（列表第一个）即为当前活跃版本
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
                <span className="timeline-item__dot">{isActive ? '●' : '○'}</span>
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
                    {restoring === v.content_hash ? '⏳' : '↩'}
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
