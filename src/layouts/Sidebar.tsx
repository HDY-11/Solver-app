// layouts/Sidebar.tsx

import { useState, useEffect, useCallback, useRef, memo } from 'react';
import { useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { getRendererByExtension } from '../registry/registry';

// ── 类型 ──────────────────────────────────────

interface VfsNode {
  id: number;
  name: string;
  node_type: 'file' | 'folder' | 'run';
  size: number | null;
  modified_at: string;
}

// ── API ───────────────────────────────────────

const vfs = {
  async listChildren(path: string): Promise<VfsNode[]> {
    return invoke<VfsNode[]>('vfs_list_dir', { path });
  },
  async createDir(path: string): Promise<void> {
    return invoke<void>('vfs_create_dir', { path });
  },
  async createFile(path: string): Promise<void> {
    return invoke<void>('vfs_write', { path, content: '' });
  },
  async deleteNode(path: string): Promise<void> {
    return invoke<void>('vfs_delete', { path });
  },
};

// ── 辅助 ──────────────────────────────────────

function nodeIcon(node: VfsNode): string {
  if (node.node_type === 'run') return '📊';
  if (node.node_type === 'folder') return '📁';
  const ext = node.name.split('.').pop() ?? '';
  const icons: Record<string, string> = {
    py: '🐍', r: '📊', jl: '🔢', sps: '📈',
    html: '🌐', htm: '🌐', csv: '📋', txt: '📄',
    log: '📜', json: '📦', md: '📝',
    png: '🖼️', jpg: '🖼️', svg: '🖼️',
    sav: '💾', h5: '💾', pkl: '💾',
    sh: '⚡', bat: '⚡', ps1: '⚡',
  };
  return icons[ext] ?? '📄';
}

function fmtSize(bytes: number | null): string {
  if (bytes === null || bytes === undefined) return '';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

// ── Sidebar ───────────────────────────────────

function Sidebar() {
  const navigate = useNavigate();
  const [rootNodes, setRootNodes] = useState<VfsNode[]>([]);
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set(['(vfs)/C']));
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [contextMenu, setContextMenu] = useState<{
    node: VfsNode; path: string; x: number; y: number;
  } | null>(null);
  const [creating, setCreating] = useState<{
    parentPath: string; type: 'file' | 'folder';
  } | null>(null);
  const [newName, setNewName] = useState('');
  const [refreshKey, setRefreshKey] = useState(0);

  // ref 保持最新值
  const creatingRef = useRef(creating);
  creatingRef.current = creating;
  const newNameRef = useRef(newName);
  newNameRef.current = newName;

  const refreshRoot = useCallback(async () => {
    const nodes = await vfs.listChildren('(vfs)/C');
    setRootNodes(nodes);
  }, []);

  useEffect(() => {
    refreshRoot();
  }, [refreshRoot]);

  // 提交创建
  const submitCreate = useCallback(async () => {
    const currentCreating = creatingRef.current;
    const name = newNameRef.current.trim();

    if (!name || !currentCreating) return;

    const fullPath = `${currentCreating.parentPath}/${name}`;
    if (currentCreating.type === 'folder') {
      await vfs.createDir(fullPath);
    } else {
      await vfs.createFile(fullPath);
    }
    setExpandedPaths(prev => new Set([...prev, currentCreating.parentPath]));
    await refreshRoot();
    setRefreshKey(k => k + 1);
    setCreating(null);
    setNewName('');
  }, [refreshRoot]);

  const handleDoubleClick = useCallback((node: VfsNode, path: string) => {
    if (node.node_type === 'folder' || node.node_type === 'run') {
      setExpandedPaths(prev => {
        const next = new Set(prev);
        next.has(path) ? next.delete(path) : next.add(path);
        return next;
      });
    } else {
      const ext = '.' + (node.name.split('.').pop() ?? '');
      const renderer = getRendererByExtension(ext);
      if (renderer) {
        navigate(`/app/${renderer.name}/${node.id}`);
      }
    }
  }, [navigate]);

  const handleClick = useCallback((_node: VfsNode, path: string) => {
    setSelectedPath(prev => prev === path ? prev : path);
    setContextMenu(null);
  }, []);

  const handleContextMenu = useCallback(
    (e: React.MouseEvent, node: VfsNode, path: string) => {
      e.preventDefault();
      e.stopPropagation();
      setContextMenu({ node, path, x: e.clientX, y: e.clientY });
    }, []);

  const closeContextMenu = useCallback(() => setContextMenu(null), []);

  const handleDelete = useCallback(async () => {
    if (!contextMenu) return;
    await vfs.deleteNode(contextMenu.path);
    setExpandedPaths(prev => {
      const next = new Set(prev);
      next.delete(contextMenu.path);
      return next;
    });
    await refreshRoot();
    setRefreshKey(k => k + 1);
    setContextMenu(null);
  }, [contextMenu, refreshRoot]);

  const handleStartCreate = useCallback(
    (parentPath: string, type: 'file' | 'folder') => {
      setCreating({ parentPath, type });
      setNewName('');
      setContextMenu(null);
    }, []);

  // 稳定事件引用
  const handleClickRef = useRef(handleClick);
  const handleDoubleClickRef = useRef(handleDoubleClick);
  const handleContextMenuRef = useRef(handleContextMenu);
  handleClickRef.current = handleClick;
  handleDoubleClickRef.current = handleDoubleClick;
  handleContextMenuRef.current = handleContextMenu;

  const stableClick = useCallback((node: VfsNode, path: string) => {
    handleClickRef.current(node, path);
  }, []);
  const stableDoubleClick = useCallback((node: VfsNode, path: string) => {
    handleDoubleClickRef.current(node, path);
  }, []);
  const stableContextMenu = useCallback((e: React.MouseEvent, node: VfsNode, path: string) => {
    handleContextMenuRef.current(e, node, path);
  }, []);

  return (
    <aside className="app-sidebar" onClick={closeContextMenu}>
      <div className="sidebar-toolbar">
        <span className="sidebar-toolbar__title">工作台</span>
        <div className="sidebar-toolbar__actions">
          <button
            className="icon-btn"
            title="新建文件夹"
            onClick={() => handleStartCreate('(vfs)/C', 'folder')}
          >📁+</button>
          <button
            className="icon-btn"
            title="刷新"
            onClick={() => {
              refreshRoot();
              setRefreshKey(k => k + 1);
            }}
          >🔄</button>
        </div>
      </div>

      <div className="sidebar-tree">
        <TreeNode
          node={{ id: 0, name: 'C:', node_type: 'folder', size: null, modified_at: '' }}
          path="(vfs)/C"
          depth={0}
          expandedPaths={expandedPaths}
          selectedPath={selectedPath}
          preloadedChildren={rootNodes}
          refreshKey={refreshKey}
          onClick={stableClick}
          onDoubleClick={stableDoubleClick}
          onContextMenu={stableContextMenu}
          onCreateRequest={handleStartCreate}
        />

        {creating && (
          <div style={{ padding: `2px 8px 2px 24px` }}>
            <form onSubmit={e => { e.preventDefault(); submitCreate(); }}>
              <input
                className="tree-input"
                autoFocus
                value={newName}
                onChange={e => setNewName(e.target.value)}
                onBlur={submitCreate}
                placeholder={creating.type === 'folder' ? '文件夹名' : '文件名'}
              />
            </form>
          </div>
        )}
      </div>

      {contextMenu && (
        <div className="context-menu" style={{ left: contextMenu.x, top: contextMenu.y }}>
          {contextMenu.node.node_type === 'folder' && (
            <>
              <div className="context-menu__item"
                onClick={() => handleStartCreate(contextMenu.path, 'file')}>
                📄 新建文件
              </div>
              <div className="context-menu__item"
                onClick={() => handleStartCreate(contextMenu.path, 'folder')}>
                📁 新建文件夹
              </div>
              <div className="context-menu__divider" />
            </>
          )}
          <div className="context-menu__item" onClick={handleDelete}>
            🗑️ 删除
          </div>
        </div>
      )}
    </aside>
  );
}

// ── TreeNode（memo 包裹）────────────────────

const TreeNode = memo(function TreeNode({
  node, path, depth, expandedPaths, selectedPath,
  preloadedChildren, refreshKey,
  onClick, onDoubleClick, onContextMenu, onCreateRequest,
}: {
  node: VfsNode;
  path: string;
  depth: number;
  expandedPaths: Set<string>;
  selectedPath: string | null;
  preloadedChildren?: VfsNode[];
  refreshKey: number;
  onClick: (node: VfsNode, path: string) => void;
  onDoubleClick: (node: VfsNode, path: string) => void;
  onContextMenu: (e: React.MouseEvent, node: VfsNode, path: string) => void;
  onCreateRequest: (parentPath: string, type: 'file' | 'folder') => void;
}) {
  const [children, setChildren] = useState<VfsNode[] | null>(
    preloadedChildren !== undefined ? preloadedChildren : null
  );
  const [lastRefresh, setLastRefresh] = useState(0);
  const isExpanded = expandedPaths.has(path);
  const isSelected = selectedPath === path;
  const isFolder = node.node_type === 'folder' || node.node_type === 'run';

  // refreshKey 变化 → 清除缓存，触发重新加载
  useEffect(() => {
    if (refreshKey !== lastRefresh && children !== null) {
      setChildren(null);
      setLastRefresh(refreshKey);
    }
  }, [refreshKey, lastRefresh, children]);

  // preloadedChildren 变化时更新子节点（根节点专用）
  useEffect(() => {
    if (preloadedChildren !== undefined && node.id === 0) {
      setChildren(preloadedChildren);
    }
  }, [preloadedChildren, node.id]);

  // 展开时懒加载
  useEffect(() => {
    if (isExpanded && isFolder && children === null) {
      if (preloadedChildren !== undefined && node.id === 0) {
        setChildren(preloadedChildren);
      } else {
        vfs.listChildren(path).then(setChildren);
      }
    }
  }, [isExpanded, isFolder, children, path, preloadedChildren, node.id]);

  const nodeClasses = [
    'tree-node',
    isSelected ? 'tree-node--selected' : '',
  ].filter(Boolean).join(' ');

  return (
    <div>
      <div
        className={nodeClasses}
        style={{ paddingLeft: 8 + depth * 16 }}
        onClick={() => onClick(node, path)}
        onDoubleClick={() => onDoubleClick(node, path)}
        onContextMenu={e => onContextMenu(e, node, path)}
      >
        {isFolder && (
          <span className="tree-node__arrow">{isExpanded ? '▼' : '▶'}</span>
        )}
        {!isFolder && <span className="tree-node__arrow" />}
        <span className="tree-node__icon">{nodeIcon(node)}</span>
        <span className="tree-node__name">{node.name}</span>
        {node.size != null && (
          <span className="tree-node__size">{fmtSize(node.size)}</span>
        )}
      </div>

      {isExpanded && (
        <div>
          {children?.map(child => (
            <TreeNode
              key={child.id}
              node={child}
              path={`${path}/${child.name}`}
              depth={depth + 1}
              expandedPaths={expandedPaths}
              selectedPath={selectedPath}
              refreshKey={refreshKey}
              onClick={onClick}
              onDoubleClick={onDoubleClick}
              onContextMenu={onContextMenu}
              onCreateRequest={onCreateRequest}
            />
          ))}
          {children?.length === 0 && (
            <div className="tree-node__empty" style={{ paddingLeft: 8 + (depth + 1) * 16 }}>
              空
            </div>
          )}
        </div>
      )}
    </div>
  );
});

export default Sidebar;