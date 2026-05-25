// layouts/WelcomeView.tsx — 欢迎页
//
// 显示快速入口和最近 VFS 文件列表，作为应用启动后的默认视图。

import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { error as logError } from '@tauri-apps/plugin-log';
import { listDir, getInfo } from '../api/vfs';
import { getRendererByExtension } from '../registry/registry';
import type { VfsNode, VfsInfo } from '../types';
import styles from './WelcomeView.module.css';

// =========================================================================
// 工具
// =========================================================================

function fmtSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** 从 VFS 节点列表中提取文件（非目录），并按修改时间降序排列 */
function getRecentFiles(nodes: VfsNode[], limit = 10): VfsNode[] {
  return nodes
    .filter(n => n.node_type === 'file')
    .sort((a, b) => b.modified_at.localeCompare(a.modified_at))
    .slice(0, limit);
}

// =========================================================================
// 组件
// =========================================================================

function WelcomeView() {
  const navigate = useNavigate();
  const [recentFiles, setRecentFiles] = useState<VfsNode[]>([]);
  const [vfsInfo, setVfsInfo] = useState<VfsInfo | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    // 并行加载 VFS 根目录和 VFS 信息
    Promise.all([
      listDir('(vfs)/C').catch((err) => {
        logError(`WelcomeView: 加载文件列表失败: ${err}`);
        return [] as VfsNode[];
      }),
      getInfo().catch((err) => {
        logError(`WelcomeView: 获取 VFS 信息失败: ${err}`);
        return null;
      }),
    ]).then(([nodes, info]) => {
      setRecentFiles(getRecentFiles(nodes));
      setVfsInfo(info);
      setLoading(false);
    });
  }, []);

  const handleOpenFile = (node: VfsNode) => {
    const ext = '.' + (node.name.split('.').pop() ?? '');
    const renderer = getRendererByExtension(ext);
    if (renderer) {
      navigate(`/app/${renderer.name}/${node.id}`);
    }
  };

  const handleNewPython = () => {
    navigate('/app/window/py/new');
  };

  return (
    <div className={styles.container}>
      <div className={styles.hero}>
        <h1 className={styles.heroTitle}>🧮 Solver</h1>
        <p className={styles.heroDesc}>高性能计算与数据分析工作台</p>
      </div>

      <section className={styles.section}>
        <h2 className={styles.sectionTitle}>快速操作</h2>
        <div className={styles.actions}>
          <button className="btn btn-primary" onClick={handleNewPython}>
            🐍 新建 Python 脚本
          </button>
          <button className="btn" onClick={() => navigate('/app/window/setting')}>
            ⚙ 设置
          </button>
          <button className="btn" onClick={() => navigate('/app/window/ViewsPage')}>
            📋 运行历史
          </button>
        </div>
      </section>

      <section className={styles.section}>
        <h2 className={styles.sectionTitle}>
          最近文件
          {loading && <span className={styles.loadingHint}>加载中...</span>}
        </h2>
        {recentFiles.length === 0 ? (
          <p className={styles.emptyHint}>
            {loading ? '加载中...' : '暂无文件，从侧边栏创建或导入'}
          </p>
        ) : (
          <div className={styles.fileList}>
            {recentFiles.map((node) => (
              <div
                key={node.id}
                className={styles.fileItem}
                onClick={() => handleOpenFile(node)}
              >
                <span>🐍</span>
                <span className={styles.fileName}>{node.name}</span>
                {node.size != null && (
                  <span className={styles.fileSize}>{fmtSize(node.size)}</span>
                )}
              </div>
            ))}
          </div>
        )}
      </section>

      {vfsInfo && (
        <section>
          <h2 className={styles.sectionTitle}>VFS 状态</h2>
          <div className={styles.statGrid}>
            <div className={styles.statCard}>
              <span className={styles.statLabel}>存储用量</span>
              <span className={styles.statValue}>
                {fmtSize(vfsInfo.c_used)} / {fmtSize(vfsInfo.c_total)}
              </span>
            </div>
            <div className={styles.statCard}>
              <span className={styles.statLabel}>节点数</span>
              <span className={styles.statValue}>{vfsInfo.c_node_count}</span>
            </div>
          </div>
        </section>
      )}
    </div>
  );
}

export default WelcomeView;
