// layouts/Sidebar.tsx — VFS 文件树侧边栏

import { useState, useEffect, useCallback, useRef, memo } from 'react';
import { useNavigate } from 'react-router-dom';
import { error as logError } from '@tauri-apps/plugin-log';
import { getRendererByExtension } from '../registry/registry';
import { listDir, createDir, writeFile, deleteNode } from '../api/vfs';
import { useToast } from '../hooks/useToast';
import ConfirmDialog from '../components/ConfirmDialog';
import NewScriptDialog from '../components/NewScriptDialog';
import type { VfsNode } from '../types';
import styles from './Sidebar.module.css';

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
  const { addToast } = useToast();
  const [rootNodes, setRootNodes] = useState<VfsNode[]>([]);
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set(['(vfs)/C']));
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [contextMenu, setContextMenu] = useState<{
    node: VfsNode; path: string; x: number; y: number;
  } | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<{
    node: VfsNode; path: string;
  } | null>(null);
  const [showNewScript, setShowNewScript] = useState(false);
  const [creating, setCreating] = useState<{
    parentPath: string; type: 'file' | 'folder';
  } | null>(null);
  const [newName, setNewName] = useState('');
  const [refreshKey, setRefreshKey] = useState(0);
  const [searchQuery, setSearchQuery] = useState('');

  // 过滤后的根节点列表
  const filteredRootNodes = searchQuery.trim()
    ? rootNodes.filter(n => n.name.toLowerCase().includes(searchQuery.toLowerCase()))
    : rootNodes;

  // ref 保持最新值
  const creatingRef = useRef(creating);
  creatingRef.current = creating;
  const newNameRef = useRef(newName);
  newNameRef.current = newName;

  const refreshRoot = useCallback(async () => {
    try {
      const nodes = await listDir('(vfs)/C');
      setRootNodes(nodes);
    } catch (err) {
      logError(`Sidebar: 加载根目录失败: ${err}`);
    }
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
    try {
      if (currentCreating.type === 'folder') {
        await createDir(fullPath);
      } else {
        await writeFile(fullPath, '');
      }
      addToast('success', `已创建: ${name}`);
    } catch (err) {
      logError(`Sidebar: 创建失败: ${err}`);
      addToast('error', `创建失败: ${err}`);
    }
    setExpandedPaths(prev => new Set([...prev, currentCreating.parentPath]));
    await refreshRoot();
    setRefreshKey(k => k + 1);
    setCreating(null);
    setNewName('');
  }, [refreshRoot, addToast]);

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
        // path 已是完整 VFS 路径如 (vfs)/C/folder/script.py，编码后放入 URL
        navigate(`/app/${renderer.name}/${encodeURIComponent(path)}`);
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

  // 从模板创建脚本
  const handleCreateFromTemplate = useCallback(async (code: string, _templateName: string) => {
    setShowNewScript(false);
    // 生成默认文件名
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
    const name = `script_${timestamp}.py`;
    const fullPath = `(vfs)/C/${name}`;
    try {
      await writeFile(fullPath, code);
      addToast('success', `已创建: ${name}`);
      await refreshRoot();
      setRefreshKey(k => k + 1);
    } catch (err) {
      logError(`Sidebar: 创建脚本失败: ${err}`);
      addToast('error', `创建失败: ${err}`);
    }
  }, [refreshRoot, addToast]);

  const handleDelete = useCallback(async () => {
    if (!contextMenu) return;
    // 先关闭右键菜单，弹出确认对话框
    setConfirmDelete({ node: contextMenu.node, path: contextMenu.path });
    setContextMenu(null);
  }, [contextMenu]);

  // 确认删除后的实际操作
  const confirmDeleteAction = useCallback(async () => {
    if (!confirmDelete) return;
    try {
      await deleteNode(confirmDelete.path);
      addToast('success', `已删除: ${confirmDelete.node.name}`);
    } catch (err) {
      logError(`Sidebar: 删除失败: ${err}`);
      addToast('error', `删除失败: ${err}`);
    }
    setExpandedPaths(prev => {
      const next = new Set(prev);
      next.delete(confirmDelete.path);
      return next;
    });
    await refreshRoot();
    setRefreshKey(k => k + 1);
    setConfirmDelete(null);
  }, [confirmDelete, refreshRoot, addToast]);

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
            title="新建 Python 脚本"
            onClick={() => setShowNewScript(true)}
          >📄+</button>
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

      {/* 搜索过滤 */}
      <div className={styles.searchBox}>
        <input
          className={styles.searchInput}
          placeholder="🔍 搜索文件..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
        />
      </div>

      <div className="sidebar-tree">
        <TreeNode
          node={{ id: 0, name: 'C:', node_type: 'folder', size: null, modified_at: '', version: '0.1.0' }}
          path="(vfs)/C"
          depth={0}
          expandedPaths={expandedPaths}
          selectedPath={selectedPath}
          preloadedChildren={filteredRootNodes}
          refreshKey={refreshKey}
          onClick={stableClick}
          onDoubleClick={stableDoubleClick}
          onContextMenu={stableContextMenu}
          onCreateRequest={handleStartCreate}
        />

        {creating && (
          <div className={styles.inlineForm}>
            <form onSubmit={e => { e.preventDefault(); submitCreate(); }}>
              <input
                className={styles.inlineInput}
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

      {/* 确认删除对话框 */}
      <ConfirmDialog
        open={confirmDelete !== null}
        title="确认删除"
        message={`确定要删除「${confirmDelete?.node.name ?? ''}」吗？此操作不可撤销。`}
        danger
        confirmLabel="删除"
        onConfirm={confirmDeleteAction}
        onCancel={() => setConfirmDelete(null)}
      />

      {/* 新建脚本对话框 */}
      <NewScriptDialog
        open={showNewScript}
        onSelect={handleCreateFromTemplate}
        onCancel={() => setShowNewScript(false)}
      />
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
        listDir(path).then(setChildren).catch((err) => {
          logError(`Sidebar: 加载子节点失败 (${path}): ${err}`);
        });
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