import React, { useRef, useEffect, useState } from 'react';
import { City } from '../types';

interface TSPCanvasProps {
  cities: City[];
  path: number[];
  width: number;
  height: number;
}

const TSPCanvas: React.FC<TSPCanvasProps> = ({ cities, path, width, height }) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [scale, setScale] = useState(1);
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const [isDragging, setIsDragging] = useState(false);
  const [dragStart, setDragStart] = useState({ x: 0, y: 0 });

  const getBounds = () => {
    if (cities.length === 0) return { minX: 0, maxX: width, minY: 0, maxY: height };
    const xs = cities.map(c => c.x);
    const ys = cities.map(c => c.y);
    return {
      minX: Math.min(...xs),
      maxX: Math.max(...xs),
      minY: Math.min(...ys),
      maxY: Math.max(...ys),
    };
  };

  const toCanvasCoords = (x: number, y: number): [number, number] => {
    const bounds = getBounds();
    const rangeX = bounds.maxX - bounds.minX;
    const rangeY = bounds.maxY - bounds.minY;
    
    const scaleX = (width * 0.8) / (rangeX || 1);
    const scaleY = (height * 0.8) / (rangeY || 1);
    const baseScale = Math.min(scaleX, scaleY);
    
    const centerX = width / 2;
    const centerY = height / 2;
    const midX = (bounds.minX + bounds.maxX) / 2;
    const midY = (bounds.minY + bounds.maxY) / 2;
    
    let canvasX = centerX + (x - midX) * baseScale * scale;
    let canvasY = centerY + (y - midY) * baseScale * scale;
    
    canvasX += offset.x;
    canvasY += offset.y;
    
    return [canvasX, canvasY];
  };

  useEffect(() => {
    draw();
  }, [cities, path, width, height, scale, offset]);

  const drawArrow = (ctx: CanvasRenderingContext2D, fromX: number, fromY: number, toX: number, toY: number) => {
    const angle = Math.atan2(toY - fromY, toX - fromX);
    const arrowSize = 10;
    
    const arrowX = toX - arrowSize * 0.5 * Math.cos(angle);
    const arrowY = toY - arrowSize * 0.5 * Math.sin(angle);
    
    ctx.beginPath();
    ctx.moveTo(arrowX, arrowY);
    ctx.lineTo(
      arrowX - arrowSize * Math.cos(angle - Math.PI / 6),
      arrowY - arrowSize * Math.sin(angle - Math.PI / 6)
    );
    ctx.lineTo(
      arrowX - arrowSize * Math.cos(angle + Math.PI / 6),
      arrowY - arrowSize * Math.sin(angle + Math.PI / 6)
    );
    ctx.fillStyle = '#ff4444';
    ctx.fill();
  };

  const draw = () => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    
    ctx.clearRect(0, 0, width, height);
    
    // 绘制背景网格
    ctx.strokeStyle = '#e0e0e0';
    ctx.lineWidth = 0.5;
    const gridSize = 50;
    for (let x = 0; x < width; x += gridSize) {
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, height);
      ctx.stroke();
    }
    for (let y = 0; y < height; y += gridSize) {
      ctx.beginPath();
      ctx.moveTo(0, y);
      ctx.lineTo(width, y);
      ctx.stroke();
    }
    
    // 绘制路径
    if (path.length > 1) {
      ctx.strokeStyle = '#ff4444';
      ctx.lineWidth = 3;
      
      for (let i = 0; i < path.length - 1; i++) {
        const fromCity = cities[path[i]];
        const toCity = cities[path[i + 1]];
        if (fromCity && toCity) {
          const [fromX, fromY] = toCanvasCoords(fromCity.x, fromCity.y);
          const [toX, toY] = toCanvasCoords(toCity.x, toCity.y);
          
          ctx.beginPath();
          ctx.moveTo(fromX, fromY);
          ctx.lineTo(toX, toY);
          ctx.stroke();
          drawArrow(ctx, fromX, fromY, toX, toY);
        }
      }
      
      // 绘制回到起点的路径
      if (path.length === cities.length && path.length > 0) {
        const fromCity = cities[path[path.length - 1]];
        const toCity = cities[path[0]];
        if (fromCity && toCity) {
          const [fromX, fromY] = toCanvasCoords(fromCity.x, fromCity.y);
          const [toX, toY] = toCanvasCoords(toCity.x, toCity.y);
          
          ctx.beginPath();
          ctx.moveTo(fromX, fromY);
          ctx.lineTo(toX, toY);
          ctx.stroke();
          drawArrow(ctx, fromX, fromY, toX, toY);
        }
      }
    }
    
    // 绘制城市点
    cities.forEach((city, index) => {
      const [x, y] = toCanvasCoords(city.x, city.y);
      
      ctx.beginPath();
      ctx.arc(x, y, 8, 0, 2 * Math.PI);
      ctx.fillStyle = '#4CAF50';
      ctx.fill();
      ctx.strokeStyle = '#ffffff';
      ctx.lineWidth = 2;
      ctx.stroke();
      
      ctx.fillStyle = '#ffffff';
      ctx.font = 'bold 12px Arial';
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText(index.toString(), x, y);
    });
  };

  const handleWheel = (e: React.WheelEvent) => {
    const delta = e.deltaY > 0 ? 0.9 : 1.1;
    setScale(prev => Math.min(Math.max(prev * delta, 0.1), 5));
  };

  const handleMouseDown = (e: React.MouseEvent) => {
    setIsDragging(true);
    setDragStart({ x: e.clientX - offset.x, y: e.clientY - offset.y });
  };

  const handleMouseMove = (e: React.MouseEvent) => {
  if (!isDragging) return;
    setOffset({ x: e.clientX - dragStart.x, y: e.clientY - dragStart.y });
  };

  const handleMouseUp = () => {
    setIsDragging(false);
  };

  const resetView = () => {
    setScale(1);
    setOffset({ x: 0, y: 0 });
  };

  return (
    <div className="canvas-wrapper" style={{ position: 'relative', display: 'inline-block' }}>
      <canvas
        ref={canvasRef}
        width={width}
        height={height}
        style={{ border: '1px solid #ccc', cursor: isDragging ? 'grabbing' : 'grab' }}
        onWheel={handleWheel}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
      />
      <button 
        onClick={resetView}
        style={{
          position: 'absolute',
          bottom: 10,
          right: 10,
          backgroundColor: 'rgba(0,0,0,0.7)',
          color: 'white',
          border: 'none',
          borderRadius: 4,
          padding: '5px 10px',
          cursor: 'pointer',
          fontSize: 12
        }}
      >
        重置视图
      </button>
    </div>
  );
};

export default TSPCanvas;