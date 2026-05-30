// layouts/Sidebar.tsx — 侧边栏容器
//
// 根据左导航栏的模式切换内容：资源管理器 / 运行结果管理器。
// 底部始终显示当前文件的时间线。

import { useState, useEffect, useCallback, useRef, memo } from 'react';
import { useNavigate } from 'react-router-dom';
import { error as logError } from '@tauri-apps/plugin-log';
import { getRendererByExtension } from '../registry/registry';
import { listDir, createDir, writeFile, deleteNode, renameFile, setVersion, syncVault } from '../api/vfs';
import { useToast } from '../hooks/useToast';
import { Icon } from '../utils/icons';
import ConfirmDialog from '../components/ConfirmDialog';
import NewScriptDialog from '../components/NewScriptDialog';
import TimelinePanel from '../components/TimelinePanel';
import RunList from '../components/RunList';
import type { VfsNode } from '../types';
import { fmtSize } from '../types';
import type { NavMode } from './NavBar';
import styles from './Sidebar.module.css';

// ── 辅助 ──────────────────────────────────────

function nodeIcon(node: VfsNode): string {
  if (node.node_type === 'run') return 'chart';
  if (node.node_type === 'folder') return 'folder';
  const ext = node.name.split('.').pop() ?? '';
  const icons: Record<string, string> = {
    py: 'python', r: 'chart', jl: 'chart', sps: 'chart',
    html: 'globe', htm: 'globe', csv: 'table', txt: 'file',
    log: 'scroll', json: 'box', md: 'note',
    png: 'image', jpg: 'image', svg: 'image',
    sav: 'save', h5: 'save', pkl: 'save',
    sh: 'bolt', bat: 'bolt', ps1: 'bolt',
    run: 'chart',
  };
  return icons[ext] ?? 'file';
}

// ── Sidebar ───────────────────────────────────

function Sidebar({ mode }: { mode: NavMode }) {
  const navigate = useNavigate();
  const { addToast } = useToast();
  const [rootNodes, setRootNodes] = useState<VfsNode[]>([]);
  const [bRootNodes, setBRootNodes] = useState<VfsNode[]>([]);
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set(['(vfs)/C']));
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [contextMenu, setContextMenu] = useState<{
    node: VfsNode; path: string; x: number; y: number;
  } | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<{
    node: VfsNode; path: string;
  } | null>(null);
  const [renaming, setRenaming] = useState<{
    node: VfsNode; path: string;
  } | null>(null);
  const [renameValue, setRenameValue] = useState('');
  const [versioning, setVersioning] = useState<{
    node: VfsNode; path: string;
  } | null>(null);
  const [versionValue, setVersionValue] = useState('');
  const [showNewScript, setShowNewScript] = useState(false);
  const [creating, setCreating] = useState<{
    parentPath: string; type: 'file' | 'folder';
  } | null>(null);
  const [newName, setNewName] = useState('');
  const [refreshKey, setRefreshKey] = useState(0);
  const [searchQuery, setSearchQuery] = useState('');

  // 过滤后的根节点列表（按模式 + 搜索词）
  const filteredRootNodes = (searchQuery.trim()
    ? rootNodes.filter(n => n.name.toLowerCase().includes(searchQuery.toLowerCase()))
    : rootNodes).filter(n => {
      if (mode === 'files') return n.node_type !== 'run';
      if (mode === 'runs') return n.node_type === 'run';
      return true;
    });

  // ref 保持最新值
  const creatingRef = useRef(creating);
  creatingRef.current = creating;
  const newNameRef = useRef(newName);
  newNameRef.current = newName;

  const refreshRoot = useCallback(async () => {
    try {
      const [cNodes, bNodes] = await Promise.all([
        listDir('(vfs)/C'),
        listDir('(vfs)/B'),
      ]);
      setRootNodes(cNodes);
      setBRootNodes(bNodes);
    } catch (err) {
      logError(`Sidebar: 加载目录失败: ${err}`);
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

  const handleClick = useCallback((node: VfsNode, path: string) => {
    // 选中（视觉高亮）
    setSelectedPath(path);
    setContextMenu(null);

    // 文件夹：切换展开；文件（含 .run）：直接打开
    if (node.node_type === 'folder') {
      setExpandedPaths(prev => {
        const next = new Set(prev);
        next.has(path) ? next.delete(path) : next.add(path);
        return next;
      });
    } else {
      const ext = '.' + (node.name.split('.').pop() ?? '');
      const renderer = getRendererByExtension(ext);
      if (renderer) {
        navigate(`/app/${renderer.name}/${encodeURIComponent(path)}`);
      }
    }
  }, [navigate]);

  const handleContextMenu = useCallback(
    (e: React.MouseEvent, node: VfsNode, path: string) => {
      e.preventDefault();
      e.stopPropagation();
      setContextMenu({ node, path, x: e.clientX, y: e.clientY });
    }, []);

  const closeContextMenu = useCallback(() => setContextMenu(null), []);

  // 从模板创建脚本（使用用户输入的名称，创建到选中文件夹下）
  const handleCreateFromTemplate = useCallback(async (code: string, fileName: string) => {
    setShowNewScript(false);
    const name = fileName.trim() || `untitled_${Date.now()}.txt`;
    // 确定目标父目录：选中文件夹 → 选中文件的父目录 → 根目录
    let targetDir = '(vfs)/C';
    if (selectedPath) {
      // 查找选中节点的类型
      const pathParts = selectedPath.split('/');
      const lastName = pathParts[pathParts.length - 1];
      const selNode = rootNodes.find(n => n.name === lastName);
      if (selNode?.node_type === 'folder' || selNode?.node_type === 'run') {
        targetDir = selectedPath;
      }
    }
    const fullPath = `${targetDir}/${name}`;
    try {
      await writeFile(fullPath, code);
      addToast('success', `已创建: ${name}`);
      await refreshRoot();
      setRefreshKey(k => k + 1);
    } catch (err) {
      logError(`Sidebar: 创建脚本失败: ${err}`);
      addToast('error', `创建失败: ${err}`);
    }
  }, [refreshRoot, addToast, selectedPath, rootNodes]);

  const handleDelete = useCallback(async () => {
    if (!contextMenu) return;
    setConfirmDelete({ node: contextMenu.node, path: contextMenu.path });
    setContextMenu(null);
  }, [contextMenu]);

  // 开始重命名
  const handleRename = useCallback(() => {
    if (!contextMenu) return;
    setRenaming({ node: contextMenu.node, path: contextMenu.path });
    setRenameValue(contextMenu.node.name);
    setContextMenu(null);
  }, [contextMenu]);

  // 提交重命名
  const commitRename = useCallback(async () => {
    if (!renaming || !renameValue.trim() || renameValue.trim() === renaming.node.name) {
      setRenaming(null); return;
    }
    try {
      await renameFile(renaming.path, renameValue.trim());
      addToast('success', `已重命名为 ${renameValue.trim()}`);
      await refreshRoot();
      setRefreshKey(k => k + 1);
    } catch (err) {
      addToast('error', `重命名失败: ${err}`);
    }
    setRenaming(null);
    setRenameValue('');
  }, [renaming, renameValue, refreshRoot, addToast]);

  // 开始编辑版本号
  const handleVersionEdit = useCallback(() => {
    if (!contextMenu) return;
    setVersioning({ node: contextMenu.node, path: contextMenu.path });
    setVersionValue(contextMenu.node.version);
    setContextMenu(null);
  }, [contextMenu]);

  // 提交版本号
  const commitVersion = useCallback(async () => {
    if (!versioning || !versionValue.trim() || versionValue.trim() === versioning.node.version) {
      setVersioning(null); return;
    }
    try {
      await setVersion(versioning.path, versionValue.trim());
      addToast('success', `版本号已更新: ${versionValue.trim()}`);
      await refreshRoot();
      setRefreshKey(k => k + 1);
    } catch (err) {
      addToast('error', `设置版本失败: ${err}`);
    }
    setVersioning(null);
    setVersionValue('');
  }, [versioning, versionValue, refreshRoot, addToast]);

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

  // 稳定事件引用（供 memo 包裹的 TreeNode 使用）
  const handleClickRef = useRef(handleClick);
  const handleContextMenuRef = useRef(handleContextMenu);
  handleClickRef.current = handleClick;
  handleContextMenuRef.current = handleContextMenu;

  const stableClick = useCallback((node: VfsNode, path: string) => {
    handleClickRef.current(node, path);
  }, []);
  const stableContextMenu = useCallback((e: React.MouseEvent, node: VfsNode, path: string) => {
    handleContextMenuRef.current(e, node, path);
  }, []);

  return (
    <aside className="app-sidebar" onClick={closeContextMenu}>
      {mode === 'files' ? (
        <>
          <div className="sidebar-toolbar">
            <span className="sidebar-toolbar__title">工作台</span>
            <div className="sidebar-toolbar__actions">
              <button className="icon-btn" title="新建 Python 脚本"
                onClick={() => setShowNewScript(true)}><Icon icon="file-pen" /></button>
              <button className="icon-btn" title="新建文件夹"
                onClick={() => handleStartCreate('(vfs)/C', 'folder')}><Icon icon="folder-plus" /></button>
              <button className="icon-btn" title="刷新（含B盘同步）"
                onClick={() => { syncVault().finally(refreshRoot); setRefreshKey(k => k + 1); }}><Icon icon="rotate" /></button>
            </div>
          </div>
          <div className={styles.searchBox}>
            <span className={styles.searchIcon}><Icon icon="search" /></span>
            <input className={styles.searchInput} placeholder="搜索文件..."
              value={searchQuery} onChange={(e) => setSearchQuery(e.target.value)} />
          </div>
          <div className="sidebar-tree">
            <TreeNode
              node={{ id: 0, name: 'C:', node_type: 'folder', size: null, modified_at: '', version: '0.1.0' }}
              path="(vfs)/C" depth={0} expandedPaths={expandedPaths} selectedPath={selectedPath}
              preloadedChildren={filteredRootNodes} refreshKey={refreshKey}
              onClick={stableClick} onContextMenu={stableContextMenu} />
            <TreeNode
              node={{ id: -1, name: 'B:', node_type: 'folder', size: null, modified_at: '', version: '0.1.0' }}
              path="(vfs)/B" depth={0} expandedPaths={expandedPaths} selectedPath={selectedPath}
              preloadedChildren={bRootNodes.filter(n => {
                if (mode === 'files') return n.node_type !== 'run';
                if (mode === 'runs') return n.node_type === 'run';
                return true;
              })} refreshKey={refreshKey}
              onClick={stableClick} onContextMenu={stableContextMenu} />
            {creating && (
              <div className={styles.inlineForm}>
                <form onSubmit={e => { e.preventDefault(); submitCreate(); }}>
                  <input className={styles.inlineInput} autoFocus value={newName}
                    onChange={e => setNewName(e.target.value)} onBlur={submitCreate}
                    placeholder={creating.type === 'folder' ? '文件夹名' : '文件名'} />
                </form>
              </div>
            )}
          </div>
          {contextMenu && (
            <div className="context-menu" style={{ left: contextMenu.x, top: contextMenu.y }}>
              {contextMenu.node.node_type === 'folder' && (<>
                <div className="context-menu__item" onClick={() => handleStartCreate(contextMenu.path, 'file')}><Icon icon="file" /> 新建文件</div>
                <div className="context-menu__item" onClick={() => handleStartCreate(contextMenu.path, 'folder')}><Icon icon="folder" /> 新建文件夹</div>
                <div className="context-menu__divider" />
              </>)}
              <div className="context-menu__item" onClick={handleRename}><Icon icon="edit" /> 重命名</div>
              <div className="context-menu__item" onClick={handleVersionEdit}><Icon icon="tag" /> 设置版本</div>
              <div className="context-menu__item" onClick={handleDelete}><Icon icon="trash" /> 删除</div>
            </div>
          )}
          <NewScriptDialog open={showNewScript} onSelect={handleCreateFromTemplate}
            onCancel={() => setShowNewScript(false)} />
          <ConfirmDialog open={confirmDelete !== null} title="确认删除"
            message={`确定要删除「${confirmDelete?.node.name ?? ''}」吗？此操作不可撤销。`}
            danger confirmLabel="删除" onConfirm={confirmDeleteAction}
            onCancel={() => setConfirmDelete(null)} />
          {renaming && (
            <div className="confirm-overlay" onClick={() => setRenaming(null)}>
              <div className="confirm-dialog" style={{ minWidth: 300 }} onClick={e => e.stopPropagation()}>
                <h3 style={{ fontSize: '0.875rem', fontWeight: 600, marginBottom: 10 }}><Icon icon="edit" /> 重命名</h3>
                <input
                  autoFocus
                  className="tree-input"
                  style={{ width: '100%', boxSizing: 'border-box', padding: '6px 10px', marginBottom: 14, border: '1px solid var(--gray-300)', borderRadius: 6, fontSize: '0.875rem', fontFamily: 'var(--font-mono)' }}
                  value={renameValue}
                  onChange={e => setRenameValue(e.target.value)}
                  onKeyDown={e => { if (e.key === 'Enter') commitRename(); if (e.key === 'Escape') setRenaming(null); }}
                />
                <div className="confirm-dialog__actions">
                  <button className="btn btn-sm" onClick={() => setRenaming(null)}>取消</button>
                  <button className="btn btn-primary btn-sm" onClick={commitRename}>确定</button>
                </div>
              </div>
            </div>
          )}
          {versioning && (
            <div className="confirm-overlay" onClick={() => setVersioning(null)}>
              <div className="confirm-dialog" style={{ minWidth: 300 }} onClick={e => e.stopPropagation()}>
                <h3 style={{ fontSize: '0.875rem', fontWeight: 600, marginBottom: 10 }}><Icon icon="tag" /> 设置版本号</h3>
                <input
                  autoFocus
                  style={{ width: '100%', boxSizing: 'border-box', padding: '6px 10px', marginBottom: 14, border: '1px solid var(--gray-300)', borderRadius: 6, fontSize: '0.875rem', fontFamily: 'var(--font-mono)' }}
                  value={versionValue}
                  onChange={e => setVersionValue(e.target.value)}
                  onKeyDown={e => { if (e.key === 'Enter') commitVersion(); if (e.key === 'Escape') setVersioning(null); }}
                  placeholder="例如: 1.0.0"
                />
                <div className="confirm-dialog__actions">
                  <button className="btn btn-sm" onClick={() => setVersioning(null)}>取消</button>
                  <button className="btn btn-primary btn-sm" onClick={commitVersion}>确定</button>
                </div>
              </div>
            </div>
          )}
        </>
      ) : (
        <RunList />
      )}

      {/* 时间线面板：两种模式共享 */}
      <div className="sidebar-divider" />
      <TimelinePanel />
    </aside>
  );
}

// ── TreeNode（memo 包裹）────────────────────

const TreeNode = memo(function TreeNode({
  node, path, depth, expandedPaths, selectedPath,
  preloadedChildren, refreshKey,
  onClick, onContextMenu,
}: {
  node: VfsNode;
  path: string;
  depth: number;
  expandedPaths: Set<string>;
  selectedPath: string | null;
  preloadedChildren?: VfsNode[];
  refreshKey: number;
  onClick: (node: VfsNode, path: string) => void;
  onContextMenu: (e: React.MouseEvent, node: VfsNode, path: string) => void;
}) {
  const [children, setChildren] = useState<VfsNode[] | null>(
    preloadedChildren !== undefined ? preloadedChildren : null
  );
  const [lastRefresh, setLastRefresh] = useState(0);
  const isExpanded = expandedPaths.has(path);
  const isSelected = selectedPath === path;
  const isFolder = node.node_type === 'folder';

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
        onContextMenu={e => onContextMenu(e, node, path)}
      >
        {isFolder && (
          <span className="tree-node__arrow"><Icon icon={isExpanded ? 'chevron-down' : 'chevron-right'} /></span>
        )}
        {!isFolder && <span className="tree-node__arrow" />}
        <span className="tree-node__icon"><Icon icon={nodeIcon(node)} /></span>
        <span className="tree-node__name">{node.name}</span>
        {node.node_type !== 'folder' && (
          <span className="tree-node__version">v{node.version}</span>
        )}
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
              onContextMenu={onContextMenu}
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