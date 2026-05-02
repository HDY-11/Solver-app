import React from 'react';
import { ScriptResultPayload } from '../types';

interface ResultHistoryItemProps {
  result: ScriptResultPayload;
  isActive: boolean;
  onClick: () => void;
}

const ResultHistoryItem: React.FC<ResultHistoryItemProps> = ({
  result,
  isActive,
  onClick,
}) => {
  return (
    <li
      className={`result-history-item ${isActive ? 'active' : ''}`}
      onClick={onClick}
      title={result.path}
    >
      <span className="result-history-item__path">{result.path}</span>
    </li>
  );
};

export default ResultHistoryItem;