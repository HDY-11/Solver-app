import React from 'react';
import { ScriptResultPayload } from '../types';

interface ResultDetailProps {
  result: ScriptResultPayload;
}

const ResultDetail: React.FC<ResultDetailProps> = ({ result }) => {
  return (
    <div className="result-detail">
      <h4 className="result-detail__path">{result.path}</h4>
      <pre className="result-detail__stdout">
        <code>{result.stdout || '（无输出）'}</code>
      </pre>
      {result.stderr && (
        <pre className="result-detail__stderr">
          <code>{result.stderr}</code>
        </pre>
      )}
    </div>
  );
};

export default ResultDetail;