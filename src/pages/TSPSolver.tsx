import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import TSPCanvas from '../components/TSPCanvas';
import { City, TspData } from '../types';

interface SovResult {
  path: number[];
  distance: number;
}

interface TSPSolverProps {
  onNavigate?: (page: string) => void;
}

const TSPSolver: React.FC<TSPSolverProps> = ({ onNavigate }) => {
  const [tspData, setTspData] = useState<TspData | null>(null);
  const [path, setPath] = useState<number[]>([]);
  const [totalDistance, setTotalDistance] = useState<number | null>(null);
  const [numCities, setNumCities] = useState<number>(10);
  const [width, setWidth] = useState<number>(800);
  const [height, setHeight] = useState<number>(600);
  const [loading, setLoading] = useState<boolean>(false);
  const [solving, setSolving] = useState<boolean>(false);

  const generateProblem = async () => {
    setLoading(true);
    try {
      const data = await invoke<TspData>('generate_tsp_data', {
        n: numCities,
        width,
        height
      });
      setTspData(data);
      setPath([]);
      setTotalDistance(null);
    } catch (error) {
      console.error('Failed to generate TSP data:', error);
    } finally {
      setLoading(false);
    }
  };

  const solveGreedy = async () => {
    if (!tspData) return;
    
    setSolving(true);
    try {
      const result = await invoke<SovResult>('solve_greedy', {
        data: {
          cities: tspData.cities,
          adjacency_list: tspData.adjacency_list
        },
        startCity: null
      });
      setPath(result.path);
      setTotalDistance(result.distance);
    } catch (error) {
      console.error('贪心算法求解失败:', error);
    } finally {
      setSolving(false);
    }
  };

  const solveGreedyBest = async () => {
    if (!tspData) return;
    
    setSolving(true);
    try {
      const result = await invoke<SovResult>('solve_greedy_best', {
        data: {
          cities: tspData.cities,
          adjacency_list: tspData.adjacency_list
        }
      });
      setPath(result.path);
      setTotalDistance(result.distance);
    } catch (error) {
      console.error('最优起点贪心算法求解失败:', error);
    } finally {
      setSolving(false);
    }
  };

  const solveMyMST = async () => {
    if (!tspData) return;
    
    setSolving(true);
    try {
      const result = await invoke<SovResult>('solve_my_mst', {
        data: {
          cities: tspData.cities,
          adjacency_list: tspData.adjacency_list
        }
      });
      setPath(result.path);
      setTotalDistance(result.distance);
    } catch (error) {
      console.error('MST+DFS序算法求解失败:', error);
    } finally {
      setSolving(false);
    }
  };

  const solveButtons = [
    { id: 'greedy', label: '贪心算法 (从0开始)', action: solveGreedy},
    { id: 'greedy-best', label: '贪心算法 (最优起点)', action: solveGreedyBest },
    { id: 'MST-DFS', label: 'MST+DFS序', action: solveMyMST}
  ];

  return (
    <div style={{ flex: 1, overflowY: 'auto', padding: '20px' }}>
      <div className="controls">
        <div className="control-group">
          <label>城市数量:</label>
          <input
            type="number"
            min="2"
            max="50"
            value={numCities}
            onChange={(e) => setNumCities(parseInt(e.target.value) || 10)}
          />
        </div>
        
        <div className="control-group">
          <label>画布宽度:</label>
          <input
            type="number"
            min="400"
            max="1200"
            value={width}
            onChange={(e) => setWidth(parseInt(e.target.value) || 800)}
          />
        </div>
        
        <div className="control-group">
          <label>画布高度:</label>
          <input
            type="number"
            min="300"
            max="800"
            value={height}
            onChange={(e) => setHeight(parseInt(e.target.value) || 600)}
          />
        </div>
        
        <button onClick={generateProblem} disabled={loading}>
          {loading ? '生成中...' : '生成新问题'}
        </button>
      </div>

      {totalDistance !== null && (
        <div className="distance-info">
          <strong>当前路径总距离: {totalDistance.toFixed(2)}</strong>
        </div>
      )}

      <div className="solve-buttons">
        <h3>解题算法</h3>
        <div className="button-group">
          {solveButtons.map(btn => (
            <button 
              key={btn.id} 
              onClick={btn.action}
              disabled={solving || !tspData}
            >
              {solving ? '求解中...' : btn.label}
            </button>
          ))}
        </div>
      </div>

      <div className="canvas-container">
        {tspData && (
          <TSPCanvas
            cities={tspData.cities}
            path={path}
            width={width}
            height={height}
          />
        )}
      </div>
    </div>
  );
};

export default TSPSolver;