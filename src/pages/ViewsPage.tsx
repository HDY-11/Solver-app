// src/pages/ViewsPage.tsx
import React, { useEffect, useState } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
<<<<<<< HEAD
import ResultHistoryItem from '../components/ResultHistoryItem.tsx';
import ResultDetail from '../components/ResultDetail.tsx';
=======
import ResultHistoryItem from '../components/ResultHistoryItem';
import ResultDetail from '../components/ResultDetail';
>>>>>>> 738041c (0.2.05)
import { ScriptResultPayload } from '../types';

interface ViewsPageProps {
  display: boolean;
}

const MAX_RESULTS = 50;

const ViewsPage: React.FC<ViewsPageProps> = ({ display }) => {
  const [results, setResults] = useState<ScriptResultPayload[]>([]);
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;

    const setupListener = async () => {
      unlisten = await listen<ScriptResultPayload>('script-result', (event) => {
        setResults((prev) => {
          const next = [...prev, event.payload];
          // 保留最近 MAX_RESULTS 条
          return next.length > MAX_RESULTS ? next.slice(-MAX_RESULTS) : next;
        });
        // 自动选中最新一条
        setSelectedIndex((prev) => (prev === null ? 0 : prev + 1));
      });
    };

    setupListener();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  return (
    <div
      className="page-container"
      style={{ display: display ? undefined : 'none' }}
    >
      <div className="View">
        {/* 左侧：历史记录列表 */}
        <div className="result-list">
          <h3 className="result-list__title">运行历史</h3>
          {results.length === 0 ? (
            <p className="result-list__empty">暂无运行记录</p>
          ) : (
            <ul className="result-list__items">
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
        <div className="result-content">
          {selectedIndex !== null && results[selectedIndex] ? (
            <ResultDetail result={results[selectedIndex]} />
          ) : (
            <p className="result-content__placeholder">选择一条记录查看详细输出</p>
          )}
        </div>
      </div>
    </div>
  );
};

export default ViewsPage;