// pages/ViewsPage.tsx — 脚本运行历史与结果查看（注册为 panel）
//
// 不再作为独立页面，而是通过 panel 系统渲染。

import { useEffect, useState } from 'react';
import { error as logError } from '@tauri-apps/plugin-log';
import ResultHistoryItem from '../components/ResultHistoryItem.tsx';
import ResultDetail from '../components/ResultDetail.tsx';
import { EmptyState } from '../components/Loading';
import { onScriptResult } from '../api/events';
import { registerPanel } from '../registry/registry';
import type { ScriptResultPayload } from '../types';

const MAX_RESULTS = 50;

function ViewsPage() {
  const [results, setResults] = useState<ScriptResultPayload[]>([]);
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);

  useEffect(() => {
    let unlisten: Awaited<ReturnType<typeof onScriptResult>> | undefined;

    onScriptResult((payload) => {
      setResults((prev) => {
        const next = [...prev, payload];
        return next.length > MAX_RESULTS ? next.slice(-MAX_RESULTS) : next;
      });
      setSelectedIndex((prev) => (prev === null ? 0 : prev + 1));
    })
      .then((fn) => { unlisten = fn; })
      .catch((err) => logError(`ViewsPage: 监听 script-result 失败: ${err}`));

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  return (
    <div style={{ padding: 16, display: 'flex', height: '100%', gap: 12 }}>
      {/* 左侧：历史记录列表 */}
      <div style={{ width: 240, borderRight: '1px solid var(--gray-200)', overflow: 'auto' }}>
        <h3 style={{ fontSize: '0.875rem', fontWeight: 600, margin: '0 0 8px' }}>运行历史</h3>
        {results.length === 0 ? (
          <EmptyState
            icon="📋"
            title="暂无运行记录"
            description="运行 Python 脚本后，结果将显示在这里"
          />
        ) : (
          <ul style={{ listStyle: 'none', padding: 0, margin: 0 }}>
            {results.map((res, idx) => (
              <ResultHistoryItem
                key={idx}
                result={res}
                isActive={selectedIndex === idx}
                onClick={() => setSelectedIndex(idx)}
              />
            ))}
          </ul>
        )}
      </div>

      {/* 右侧：详细输出 */}
      <div style={{ flex: 1, overflow: 'auto' }}>
        {selectedIndex !== null && results[selectedIndex] ? (
          <ResultDetail result={results[selectedIndex]} />
        ) : (
          <p style={{ color: 'var(--gray-400)', textAlign: 'center', marginTop: 40 }}>
            选择一条记录查看详细输出
          </p>
        )}
      </div>
    </div>
  );
}

// 注册为 panel，路由 /app/window/ViewsPage 可直接访问
registerPanel({
  name: 'ViewsPage',
  component: ViewsPage,
  label: '运行历史',
});

export default ViewsPage;