// components/NewScriptDialog.tsx — 新建文件对话框
//
// 支持创建任意类型文件。输入文件名（含扩展名），预设模板快速填充。

import { useState } from 'react';

const PRESETS = [
  { ext: '.py', icon: '🐍', label: 'Python', template: '# 新建脚本\n\n' },
  { ext: '.txt', icon: '📄', label: '文本', template: '' },
  { ext: '.json', icon: '📦', label: 'JSON', template: '{\n  \n}\n' },
  { ext: '.md', icon: '📝', label: 'Markdown', template: '# \n\n' },
];

const DEFAULT_TEMPLATE = '';

interface Props {
  open: boolean;
  onSelect: (code: string, name: string) => void;
  onCancel: () => void;
}

function NewScriptDialog({ open, onSelect, onCancel }: Props) {
  const [name, setName] = useState('');
  const [selectedPreset, setSelectedPreset] = useState(0);

  if (!open) return null;

  const handleCreate = () => {
    const finalName = name.trim() || `untitled${PRESETS[selectedPreset].ext}`;
    const template = name.trim() ? (PRESETS.find(p => finalName.endsWith(p.ext))?.template ?? DEFAULT_TEMPLATE) : PRESETS[selectedPreset].template;
    onSelect(template, finalName);
  };

  return (
    <div className="confirm-overlay" onClick={onCancel}>
      <div className="confirm-dialog" style={{ minWidth: 360 }} onClick={e => e.stopPropagation()}>
        <h3 style={{ fontSize: '1rem', fontWeight: 600, marginBottom: 12 }}>📄 新建文件</h3>

        <input
          autoFocus
          value={name}
          onChange={e => setName(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && handleCreate()}
          placeholder="输入文件名，如 script.py"
          style={{
            width: '100%', boxSizing: 'border-box',
            padding: '6px 10px', marginBottom: 10,
            border: '1px solid var(--gray-300)', borderRadius: 6,
            fontSize: '0.875rem', fontFamily: 'var(--font-mono)',
          }}
        />

        <div style={{ display: 'flex', gap: 6, marginBottom: 16, flexWrap: 'wrap' }}>
          {PRESETS.map((p, i) => (
            <button
              key={p.ext}
              className={`btn btn-sm ${i === selectedPreset ? 'btn-primary' : ''}`}
              onClick={() => { setSelectedPreset(i); if (!name.trim()) setName(`untitled${p.ext}`); }}
            >
              {p.icon} {p.label}
            </button>
          ))}
        </div>

        <div className="confirm-dialog__actions">
          <button className="btn btn-sm" onClick={onCancel}>取消</button>
          <button className="btn btn-primary btn-sm" onClick={handleCreate}>创建</button>
        </div>
      </div>
    </div>
  );
}

export { PRESETS };
export default NewScriptDialog;