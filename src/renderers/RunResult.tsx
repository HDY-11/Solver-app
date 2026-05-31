// renderers/RunResult.tsx — .run 文件渲染器
//
// 三态渲染：loading（运行中）→ streaming（实时输出）→ complete（最终结果）

import { useState, useEffect, useRef, useCallback } from 'react';
import { registerRenderer } from '../registry/registry';
import { readFile, writeFile } from '../api/vfs';
import { onRunOutput, onRunComplete } from '../api/events';
import { runScript } from '../api/script';
import { Loading } from '../components/Loading';
import { Icon } from '../utils/icons';
import type { RendererProps } from '../registry/types';
import type { RunRecordContent, RunOutputPayload } from '../types';

// 模块级状态，供 Toolbar 读取（Toolbar 渲染在 RunResult 之外）
let currentRunState: { path: string; record: RunRecordContent | null } = { path: '', record: null };

/** 取最后三段路径用于事件匹配（C/运行记录/2.py.run），区分卷防串台 */
function runNameFromPath(vfsPath: string): string {
  try {
    const parts = decodeURIComponent(vfsPath).split('/');
    return parts.slice(-3).join('/') || parts.pop() || '';
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
      setRecord({ stdout: '', stderr: '无法加载运行结果' });
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
        setStatus('complete');
      })
      .catch(() => {
        setRecord({ stdout: '', stderr: '无法读取运行记录' });
        setStatus('complete');
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
      if (payloadRunName !== runName) return;
      if (payload.error) {
        setRecord({ stdout: '', stderr: payload.error });
        setStatus('complete');
      } else if (currentPathRef.current) {
        loadFile(currentPathRef.current);
      }
    });

    return () => {
      unlistenOutput.then((fn) => fn()).catch(() => {});
      unlistenComplete.then((fn) => fn()).catch(() => {});
    };
  }, [vfsPath, loadFile]);

  if (!nodeId) {
    return <div style={{ padding: 24, color: 'var(--gray-500)' }}>选择运行记录查看</div>;
  }

  // 暴露数据给 Toolbar（模块级变量，Toolbar 在组件外渲染时也能读到）
  currentRunState = { path: vfsPath ?? '', record };

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
  toolbar: () => <RunToolbar />,
});

function RunToolbar() {
  const handleRerun = async () => {
    const vfsPath = currentRunState.path;
    if (!vfsPath) return;
    try {
      const decoded = decodeURIComponent(vfsPath);
      const parts = decoded.split('/');
      const runDirIdx = parts.indexOf('运行记录');
      if (runDirIdx > 0) {
        const volume = parts[runDirIdx - 1];
        const runName = parts[parts.length - 1];
        const scriptName = runName.replace(/\.run$/, '');
        const scriptPath = `(vfs)/${volume}/${scriptName}`;
        const { run_path } = await runScript(scriptPath);
        window.dispatchEvent(new CustomEvent('run-result:rerun', { detail: { run_path } }));
      }
    } catch (err) {
      console.error('重新运行失败:', err);
    }
  };

  const handleSaveOutput = async () => {
    const vfsPath = currentRunState.path;
    const record = currentRunState.record;
    if (!vfsPath || !record) return;
    try {
      const decoded = decodeURIComponent(vfsPath);
      const outPath = decoded.replace(/\.run$/, '.txt');
      const text = [
        record.stdout || '',
        record.stderr ? '\n--- stderr ---\n' + record.stderr : '',
      ].join('');
      await writeFile(outPath, text);
    } catch (err) {
      console.error('保存输出失败:', err);
    }
  };

  return (
    <>
      <button className="toolbar-btn toolbar-btn--primary" onClick={handleRerun}>
        <Icon icon="play" /> 重新运行
      </button>
      <button className="toolbar-btn" onClick={handleSaveOutput}>
        <Icon icon="save" /> 保存输出
      </button>
    </>
  );
}

export default RunResult;
