import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface VfsNode {
  id: number;
  name: string;
  node_type: string;
  size: number | null;
  modified_at: string;
}

interface VfsInfo {
  c_exists: boolean;
  c_used: number;
  c_total: number;
  c_node_count: number;
}

const byteFmt = (bytes: number) => {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
};

const VfsTestPage: React.FC = () => {
  const [writePath, setWritePath] = useState('(vfs)/C/测试/hello.txt');
  const [writeContent, setWriteContent] = useState('Hello, VFS! 这是一条测试。');
  const [result, setResult] = useState('');
  const [nodes, setNodes] = useState<VfsNode[]>([]);
  const [vfsInfo, setVfsInfo] = useState<VfsInfo | null>(null);

  const refreshInfo = async () => {
    try {
      const info = await invoke<VfsInfo>('vfs_info');
      setVfsInfo(info);
    } catch (e) {
      setResult(`获取 VFS 信息失败: ${e}`);
    }
  };

  const refreshList = async (path: string) => {
    try {
      const list = await invoke<VfsNode[]>('vfs_list_dir', { path });
      setNodes(list);
      setResult(`列出 ${path} 成功 (${list.length} 项)`);
    } catch (e) {
      setResult(`列出目录失败: ${e}`);
      setNodes([]);
    }
  };

  const handleWrite = async () => {
    try {
      await invoke('vfs_write', { path: writePath, content: writeContent });
      setResult(`写入成功: ${writePath}`);
      await refreshInfo();
      await refreshList('(vfs)/C');
    } catch (e) {
      setResult(`写入失败: ${e}`);
    }
  };

  const handleRead = async (path: string) => {
    try {
      const content = await invoke<string>('vfs_read', { path });
      setResult(`读取成功:\n${content}`);
    } catch (e) {
      setResult(`读取失败: ${e}`);
    }
  };

  const handleDelete = async (path: string) => {
    try {
      await invoke('vfs_delete', { path });
      setResult(`删除成功: ${path}`);
      await refreshInfo();
      await refreshList('(vfs)/C');
    } catch (e) {
      setResult(`删除失败: ${e}`);
    }
  };

  const handleCheck = async (path: string) => {
    try {
      const exists = await invoke<boolean>('vfs_exists', { path });
      setResult(`${path} ${exists ? '存在' : '不存在'}`);
    } catch (e) {
      setResult(`检查失败: ${e}`);
    }
  };

  const handleCreateDir = async (path: string) => {
    try {
      await invoke('vfs_create_dir', { path });
      setResult(`创建目录成功: ${path}`);
      await refreshInfo();
      await refreshList('(vfs)/C');
    } catch (e) {
      setResult(`创建目录失败: ${e}`);
    }
  };

  return (
    <div style={{ padding: 24, maxWidth: 900, margin: '0 auto', fontFamily: 'monospace' }}>
      <h1>VFS 测试面板</h1>
      
      {/* VFS 信息 */}
      <div style={{ background: '#f5f5f5', padding: 16, borderRadius: 8, marginBottom: 16 }}>
        <h3>📊 VFS 状态</h3>
        {vfsInfo ? (
          <div>
            <p>C 盘状态: {vfsInfo.c_exists ? '✅ 正常' : '❌ 不存在'}</p>
            <p>节点数: {vfsInfo.c_node_count}</p>
            <p>已用空间: {byteFmt(vfsInfo.c_used)} / {byteFmt(vfsInfo.c_total)}</p>
            <div style={{ background: '#ddd', borderRadius: 4, height: 8, marginTop: 8 }}>
              <div style={{
                background: vfsInfo.c_used / vfsInfo.c_total > 0.9 ? '#ff4444' : '#4CAF50',
                width: `${(vfsInfo.c_used / vfsInfo.c_total * 100).toFixed(1)}%`,
                height: '100%',
                borderRadius: 4,
              }} />
            </div>
          </div>
        ) : (
          <p>未加载</p>
        )}
        <button onClick={refreshInfo} style={{ marginTop: 8 }}>刷新状态</button>
      </div>

      {/* 文件列表 */}
      <div style={{ background: '#fafafa', padding: 16, borderRadius: 8, marginBottom: 16 }}>
        <h3>📁 (vfs)/C 目录内容</h3>
        <button onClick={() => refreshList('(vfs)/C')} style={{ marginBottom: 12 }}>刷新列表</button>
        <button onClick={() => handleCreateDir('(vfs)/C/新文件夹')} style={{ marginLeft: 8, marginBottom: 12 }}>+ 新建文件夹</button>
        {nodes.length === 0 ? (
          <p style={{ color: '#999' }}>目录为空</p>
        ) : (
          <table style={{ width: '100%', borderCollapse: 'collapse' }}>
            <thead>
              <tr style={{ textAlign: 'left', borderBottom: '1px solid #ccc' }}>
                <th>类型</th>
                <th>名称</th>
                <th>大小</th>
                <th>修改时间</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {nodes.map(n => (
                <tr key={n.id} style={{ borderBottom: '1px solid #eee' }}>
                  <td>{n.node_type === 'folder' ? '📁' : '📄'}</td>
                  <td>{n.name}</td>
                  <td>{n.size != null ? byteFmt(n.size) : '-'}</td>
                  <td>{n.modified_at}</td>
                  <td>
                    {n.node_type === 'folder' ? (
                      <button onClick={() => refreshList(`(vfs)/C/${n.name}`)}>📂</button>
                    ) : (
                      <button onClick={() => handleRead(`(vfs)/C/${n.name}`)}>📖</button>
                    )}
                    <button onClick={() => handleDelete(`(vfs)/C/${n.name}`)} style={{ marginLeft: 4 }}>🗑️</button>
                    <button onClick={() => handleCheck(`(vfs)/C/${n.name}`)} style={{ marginLeft: 4 }}>❓</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {/* 写入测试 */}
      <div style={{ background: '#fafafa', padding: 16, borderRadius: 8, marginBottom: 16 }}>
        <h3>✍️ 写入测试</h3>
        <label>路径: </label>
        <input
          value={writePath}
          onChange={e => setWritePath(e.target.value)}
          style={{ width: '100%', marginBottom: 8, padding: 4 }}
        />
        <label>内容: </label>
        <textarea
          value={writeContent}
          onChange={e => setWriteContent(e.target.value)}
          style={{ width: '100%', height: 60, marginBottom: 8, padding: 4 }}
        />
        <button onClick={handleWrite}>写入 VFS</button>
      </div>

      {/* 结果 */}
      <div style={{ background: '#1e1e1e', color: '#d4d4d4', padding: 16, borderRadius: 8 }}>
        <h3 style={{ color: '#fff' }}>📋 操作结果</h3>
        <pre style={{ whiteSpace: 'pre-wrap', margin: 0 }}>{result || '等待操作...'}</pre>
      </div>
    </div>
  );
};

export default VfsTestPage;