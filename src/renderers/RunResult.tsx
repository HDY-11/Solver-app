// renderers/RunResult.tsx — .run 文件渲染器
//
// 显示脚本运行结果。.run 文件内容为 JSON，包含 script_path / stdout / stderr。

import { useState, useEffect } from 'react';
import { error as logError } from '@tauri-apps/plugin-log';
import { registerRenderer } from '../registry/registry';
import { readFile } from '../api/vfs';
import { Loading } from '../components/Loading';
import type { RendererProps } from '../registry/types';

interface RunRecord {
  script_path: string;
  script_version?: string;
  stdout: string;
  stderr: string;
}

function RunResult({ nodeId }: RendererProps) {
  const vfsPath = nodeId ? decodeURIComponent(nodeId) : null;
  const [record, setRecord] = useState<RunRecord | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!vfsPath) return;
    setLoading(true);
    readFile(vfsPath)
      .then((raw) => {
        try { setRecord(JSON.parse(raw)); }
        catch { setRecord(null); }
      })
      .catch((err) => logError(`RunResult: 加载失败: ${err}`))
      .finally(() => setLoading(false));
  }, [vfsPath]);

  if (loading) return <Loading text="加载运行结果..." />;
  if (!nodeId) return <div style={{ padding: 24, color: 'var(--gray-500)' }}>选择运行记录查看</div>;
  if (!record) return <div style={{ padding: 24, color: 'var(--gray-500)' }}>无法解析运行记录</div>;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', fontFamily: 'var(--font-mono)' }}>
      {/* 脚本信息 */}
      <div style={{ padding: '10px 16px', fontSize: 12, color: 'var(--gray-500)',
        borderBottom: '1px solid var(--gray-300)', background: 'var(--gray-100)', flexShrink: 0,
        display: 'flex', alignItems: 'center', gap: 12 }}>
        <span>📄 脚本: {record.script_path}</span>
        {record.script_version && (
          <span style={{ fontSize: 11, color: 'var(--gray-400)', background: 'var(--gray-200)',
            padding: '1px 6px', borderRadius: 3 }}>
            v{record.script_version}
          </span>
        )}
      </div>

      {/* stdout */}
      <div style={{ flex: 1, overflow: 'auto', padding: 16, fontSize: 13,
        background: '#1e1e1e', color: '#d4d4d4', whiteSpace: 'pre-wrap' }}>
        {record.stdout || <span style={{ color: '#888' }}>（无输出）</span>}
      </div>

      {/* stderr（如果有） */}
      {record.stderr && (
        <div style={{ maxHeight: 200, overflow: 'auto', padding: 16, fontSize: 13,
          background: '#2d1b1b', color: '#f48771', whiteSpace: 'pre-wrap',
          borderTop: '1px solid var(--gray-300)' }}>
          <div style={{ fontSize: 11, color: '#c44', marginBottom: 6 }}>⚠ stderr</div>
          {record.stderr}
        </div>
      )}
    </div>
  );
}

registerRenderer({
  name: 'run',
  extensions: ['.run'],
  component: RunResult,
  icon: '📊',
  label: '运行结果',
});

export default RunResult;
