// components/NewScriptDialog.tsx — 新建脚本对话框
//
// 提供脚本模板选择：空白脚本 / 数据分析 / simple_motion 物理仿真。

import { useState } from 'react';

// =========================================================================
// 模板定义
// =========================================================================

interface ScriptTemplate {
  id: string;
  name: string;
  icon: string;
  description: string;
  code: string;
}

const TEMPLATES: ScriptTemplate[] = [
  {
    id: 'blank',
    name: '空白脚本',
    icon: '📄',
    description: '从空文件开始',
    code: '# 新建 Python 脚本\n\n',
  },
  {
    id: 'data',
    name: '数据分析',
    icon: '📊',
    description: '含 pandas / numpy / matplotlib 导入',
    code: `# 数据分析脚本
import numpy as np
import pandas as pd
import matplotlib.pyplot as plt

# 在此编写分析代码

`,
  },
  {
    id: 'motion',
    name: '物理仿真',
    icon: '⚛️',
    description: '含 simple_motion 导入',
    code: `# simple_motion 物理仿真脚本
from simple_motion import Motion, Particle3

motion = Motion()
# 创建粒子并添加力模型...

`,
  },
];

// =========================================================================
// 组件
// =========================================================================

interface NewScriptDialogProps {
  open: boolean;
  onSelect: (code: string, name: string) => void;
  onCancel: () => void;
}

function NewScriptDialog({ open, onSelect, onCancel }: NewScriptDialogProps) {
  const [selectedId, setSelectedId] = useState('blank');

  if (!open) return null;

  const selected = TEMPLATES.find(t => t.id === selectedId)!;

  return (
    <div className="confirm-overlay" onClick={onCancel}>
      <div className="confirm-dialog" style={{ minWidth: 420 }} onClick={(e) => e.stopPropagation()}>
        <h3 style={{ fontSize: '1rem', fontWeight: 600, marginBottom: 16 }}>
          🐍 新建 Python 脚本
        </h3>

        {/* 模板列表 */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6, marginBottom: 16 }}>
          {TEMPLATES.map(t => (
            <div
              key={t.id}
              onClick={() => setSelectedId(t.id)}
              style={{
                display: 'flex', alignItems: 'center', gap: 10,
                padding: '8px 12px', borderRadius: 8, cursor: 'pointer',
                border: `1px solid ${selectedId === t.id ? 'var(--primary)' : 'var(--gray-200)'}`,
                background: selectedId === t.id ? 'var(--primary-bg)' : 'transparent',
                transition: 'border-color 0.15s, background 0.15s',
              }}
            >
              <span style={{ fontSize: '1.25rem' }}>{t.icon}</span>
              <div>
                <div style={{ fontSize: '0.875rem', fontWeight: 500 }}>{t.name}</div>
                <div style={{ fontSize: '0.75rem', color: 'var(--gray-500)' }}>
                  {t.description}
                </div>
              </div>
            </div>
          ))}
        </div>

        {/* 预览 */}
        <pre style={{
          background: '#1e1e1e', color: '#d4d4d4',
          borderRadius: 6, padding: 10, fontSize: 12,
          fontFamily: 'var(--font-mono)', maxHeight: 120,
          overflow: 'auto', marginBottom: 16,
          whiteSpace: 'pre-wrap',
        }}>
          {selected.code}
        </pre>

        {/* 按钮 */}
        <div className="confirm-dialog__actions">
          <button className="btn btn-sm" onClick={onCancel}>取消</button>
          <button
            className="btn btn-primary btn-sm"
            onClick={() => onSelect(selected.code, selected.name)}
          >
            创建
          </button>
        </div>
      </div>
    </div>
  );
}

export { TEMPLATES };
export default NewScriptDialog;
