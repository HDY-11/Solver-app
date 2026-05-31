// components/RunList.tsx — 运行结果列表
//
// 显示所有 .run 文件，按修改时间倒序排列。支持右键删除/重命名。

import { useState, useEffect, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { error as logError } from '@tauri-apps/plugin-log';
import { listDir, deleteNode, renameFile } from '../api/vfs';
import { useToast } from '../hooks/useToast';
import { Icon } from '../utils/icons';
import { fmtSize } from '../types';
import type { VfsNode } from '../types';

interface RunNode extends VfsNode {
  _volume: string;
}

function RunList() {
  const navigate = useNavigate();
  const { addToast } = useToast();
  const [runs, setRuns] = useState<RunNode[]>([]);
  const [loading, setLoading] = useState(true);
  const [contextMenu, setContextMenu] = useState<{ node: VfsNode; x: number; y: number } | null>(null);
  const [renaming, setRenaming] = useState<{ node: VfsNode } | null>(null);
  const [renameValue, setRenameValue] = useState('');

  const loadRuns = useCallback(async () => {
    try {
      const [cRuns, bRuns, aRuns] = await Promise.all([
        listDir('(vfs)/C/运行记录').catch(() => [] as VfsNode[]),
        listDir('(vfs)/B/运行记录').catch(() => [] as VfsNode[]),
        listDir('(vfs)/A/运行记录').catch(() => [] as VfsNode[]),
      ]);
      const all = [
        ...cRuns.map(n => ({ ...n, _volume: 'C' })),
        ...bRuns.map(n => ({ ...n, _volume: 'B' })),
        ...aRuns.map(n => ({ ...n, _volume: 'A' })),
      ];
      setRuns(
        all
          .filter((n) => n.node_type === 'file' && n.name.endsWith('.run'))
          .sort((a, b) => b.modified_at.localeCompare(a.modified_at))
      );
    } catch { setRuns([]); }
    finally { setLoading(false); }
  }, []);

  useEffect(() => { loadRuns(); }, [loadRuns]);

  const handleContext = (e: React.MouseEvent, node: VfsNode) => {
    e.preventDefault();
    setContextMenu({ node, x: e.clientX, y: e.clientY });
  };

  const handleDelete = async () => {
    if (!contextMenu) return;
    try {
      const vol = (contextMenu.node as RunNode)._volume;
      const path = `(vfs)/${vol}/运行记录/${contextMenu.node.name}`;
      await deleteNode(path);
      addToast('success', `已删除 ${contextMenu.node.name}`);
      setContextMenu(null);
      loadRuns();
    } catch (err) { addToast('error', `删除失败: ${err}`); }
  };

  const startRename = () => {
    if (!contextMenu) return;
    setRenaming({ node: contextMenu.node });
    setRenameValue(contextMenu.node.name);
    setContextMenu(null);
  };

  const commitRename = async () => {
    if (!renaming || !renameValue.trim() || renameValue.trim() === renaming.node.name) {
      setRenaming(null); return;
    }
    try {
      const vol = (renaming.node as RunNode)._volume;
      const path = `(vfs)/${vol}/运行记录/${renaming.node.name}`;
      await renameFile(path, renameValue.trim());
      addToast('success', `已重命名`);
      setRenaming(null);
      loadRuns();
    } catch (err) { addToast('error', `重命名失败: ${err}`); }
  };

  return (
    <div className="run-list">
      <div className="sidebar-toolbar">
        <span className="sidebar-toolbar__title">运行结果</span>
        <div className="sidebar-toolbar__actions">
          <button className="icon-btn" title="刷新" onClick={loadRuns}><Icon icon="rotate" /></button>
        </div>
      </div>

      {loading ? (
        <p className="timeline-panel__empty">加载中...</p>
      ) : runs.length === 0 ? (
        <p className="timeline-panel__empty">暂无运行记录</p>
      ) : (
        <div className="run-list__items">
          {runs.map((node) => (
            <div
              key={node.id}
              className="run-item"
              onClick={() => navigate(`/app/run/${encodeURIComponent(`(vfs)/${node._volume}/运行记录/${node.name}`)}`)}
              onContextMenu={(e) => handleContext(e, node)}
            >
              <span className="run-item__icon"><Icon icon="chart" /></span>
              <div className="run-item__info">
                <span className="run-item__name">
                  {node._volume !== 'C' && <span style={{ color: 'var(--blue-500)', fontSize: '0.65rem', marginRight: 4 }}>[{node._volume}]</span>}
                  {node.name}
                </span>
                <span className="run-item__meta">
                  v{node.version} · {node.modified_at.replace('T', ' ').slice(0, 19)}
                  {node.size != null && ` · ${fmtSize(node.size)}`}
                </span>
              </div>
            </div>
          ))}
        </div>
      )}
      {contextMenu && (
        <div className="context-menu" style={{ left: contextMenu.x, top: contextMenu.y }}>
          <div className="context-menu__item" onClick={startRename}><Icon icon="edit" /> 重命名</div>
          <div className="context-menu__item" onClick={handleDelete}><Icon icon="trash" /> 删除</div>
        </div>
      )}
      {renaming && (
        <div className="confirm-overlay" onClick={() => setRenaming(null)}>
          <div className="confirm-dialog" style={{ minWidth: 300 }} onClick={e => e.stopPropagation()}>
            <h3 style={{ fontSize: '0.875rem', fontWeight: 600, marginBottom: 10 }}><Icon icon="edit" /> 重命名</h3>
            <input autoFocus style={{ width: '100%', boxSizing: 'border-box', padding: '6px 10px', marginBottom: 14, border: '1px solid var(--gray-300)', borderRadius: 6, fontSize: '0.875rem', fontFamily: 'var(--font-mono)' }}
              value={renameValue} onChange={e => setRenameValue(e.target.value)}
              onKeyDown={e => { if (e.key === 'Enter') commitRename(); if (e.key === 'Escape') setRenaming(null); }} />
            <div className="confirm-dialog__actions">
              <button className="btn btn-sm" onClick={() => setRenaming(null)}>取消</button>
              <button className="btn btn-primary btn-sm" onClick={commitRename}>确定</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default RunList;
