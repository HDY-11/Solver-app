// renderers/RunResult.tsx — .run 文件渲染器
//
// 三态渲染：loading（运行中）→ streaming（实时输出）→ complete（最终结果）

import { useState, useEffect, useRef, useCallback } from 'react';
import { registerRenderer } from '../registry/registry';
import { readFile } from '../api/vfs';
import { onRunOutput, onRunComplete } from '../api/events';
import { Loading } from '../components/Loading';
import { Icon } from '../utils/icons';
import type { RendererProps } from '../registry/types';
import type { RunRecordContent, RunOutputPayload } from '../types';

/** 从 VFS 路径提取文件名（用于事件匹配） */
function runNameFromPath(vfsPath: string): string {
  try {
    return decodeURIComponent(vfsPath.split('/').pop() ?? '');
  } catch {
    return vfsPath.split('/').pop() ?? '';
  }
}

function RunResult({ nodeId }: RendererProps) {
  const vfsPath = nodeId ? decodeURIComponent(nodeId) : null;
  const [status, setStatus] = useState<'loading' | 'streaming' | 'complete'>('loading');
  const [record, setRecord] = useState<RunRecordContent | null>(null);
  const [streamOutputs, setStreamOutputs] = useState<RunOutputPayload[]>([]);
  const currentPathRef = useRef(vfsPath);
  currentPathRef.current = vfsPath;

  // 加载已完成的 .run 文件
  const loadFile = useCallback(async (path: string) => {
    try {
      const raw = await readFile(path);
      const r: RunRecordContent = JSON.parse(raw);
      setRecord(r);
      setStreamOutputs(r.outputs || []);
      setStatus('complete');
    } catch {
      setStatus('complete');
    }
  }, []);

  // 初始加载 + 事件监听
  useEffect(() => {
    if (!vfsPath) return;

    const runName = runNameFromPath(vfsPath);
    let loaded = false;

    // 尝试加载已有文件
    setStatus('loading');
    setStreamOutputs([]);
    readFile(vfsPath)
      .then((raw) => {
        const r: RunRecordContent = JSON.parse(raw);
        setRecord(r);
        setStreamOutputs(r.outputs || []);
        loaded = true;
        setStatus(r.stdout || (r.outputs && r.outputs.length > 0) ? 'complete' : 'loading');
      })
      .catch(() => {
        setStatus('loading');
      });

    // 监听实时输出 —— 仅处理当前 run_path 的事件
    const unlistenOutput = onRunOutput((payload) => {
      const payloadRunName = runNameFromPath(payload.run_path);
      if (payloadRunName !== runName) return; // ← 过滤串台
      if (loaded) return; // 文件已加载完成，忽略后续流式事件
      setStreamOutputs((prev) => [...prev, payload]);
      setStatus('streaming');
    });

    // 监听运行完成 —— 仅处理当前 run_path 的事件
    const unlistenComplete = onRunComplete((payload) => {
      const payloadRunName = runNameFromPath(payload.run_path);
      if (payloadRunName !== runName) return; // ← 过滤串台
      if (currentPathRef.current) loadFile(currentPathRef.current);
    });

    return () => {
      unlistenOutput.then((fn) => fn()).catch(() => {});
      unlistenComplete.then((fn) => fn()).catch(() => {});
    };
  }, [vfsPath, loadFile]);

  if (!nodeId) {
    return <div style={{ padding: 24, color: 'var(--gray-500)' }}>选择运行记录查看</div>;
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', fontFamily: 'var(--font-mono)' }}>
      {/* 状态栏 */}
      <div style={{ padding: '10px 16px', fontSize: 12, color: 'var(--gray-500)',
        borderBottom: '1px solid var(--gray-300)', background: 'var(--gray-100)', flexShrink: 0,
        display: 'flex', alignItems: 'center', gap: 12 }}>
        <span><Icon icon="chart" /> 运行结果</span>
        {status === 'loading' && <span style={{ color: '#f0ad4e' }}><Icon icon="spinner" /> 运行中...</span>}
        {status === 'streaming' && <span style={{ color: '#5bc0de' }}><Icon icon="signal" /> 实时输出中...</span>}
        {status === 'complete' && <span style={{ color: '#5cb85c' }}><Icon icon="success" /> 完成</span>}
      </div>

      {/* 实时输出流 */}
      <div style={{ flex: 1, overflow: 'auto', padding: 16, fontSize: 13,
        background: '#1e1e1e', color: '#d4d4d4', whiteSpace: 'pre-wrap' }}>
        {status === 'loading' && streamOutputs.length === 0 && (
          <Loading text="等待脚本输出..." />
        )}
        {streamOutputs.map((out, i) => (
          <div key={i} style={{ marginBottom: 2 }}>
            <span style={{ color: '#888', fontSize: 10 }}>{out.timestamp.slice(11, 19)} </span>
            <span>{out.content}</span>
          </div>
        ))}
        {status === 'complete' && record?.stdout && streamOutputs.length === 0 && (
          <span>{record.stdout}</span>
        )}
        {status === 'complete' && !record?.stdout && streamOutputs.length === 0 && (
          <span style={{ color: '#888' }}>（无输出）</span>
        )}
      </div>

      {/* stderr */}
      {record?.stderr && (
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
  icon: 'chart',
  label: '运行结果',
});

export default RunResult;
