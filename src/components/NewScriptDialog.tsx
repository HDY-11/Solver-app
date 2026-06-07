// components/NewScriptDialog.tsx — 新建文件对话框
// 修改类型: 修改 — 新增 .cmdv 控制台文件预设

import { useState } from 'react';
import { Icon } from '../utils/icons';

const PRESETS = [
  { ext: '.py', icon: 'python', label: 'Python', template: '# 新建脚本\n\n' },
  { ext: '.txt', icon: 'file', label: '文本', template: '' },
  { ext: '.json', icon: 'box', label: 'JSON', template: '{\n  \n}\n' },
  { ext: '.md', icon: 'note', label: 'Markdown', template: '# \n\n' },
  { ext: '.html', icon: 'globe', label: 'HTML', template: '<!DOCTYPE html>\n<html>\n<head>\n  <meta charset="UTF-8">\n  <title></title>\n</head>\n<body>\n  \n</body>\n</html>\n' },
  { ext: '.csv', icon: 'table', label: 'CSV', template: '' },
  { ext: '.cmdv', icon: 'terminal', label: '控制台', template: '{"records":[],"createdAt":""}' },  // ← 新增
];

interface Props {
  open: boolean;
  onSelect: (code: string, name: string) => void;
  onCancel: () => void;
  existingNames?: string[];
}

function NewScriptDialog({ open, onSelect, onCancel, existingNames = [] }: Props) {
  const [name, setName] = useState('');
  const [selectedPreset, setSelectedPreset] = useState(0);

  if (!open) return null;

  const ext = PRESETS[selectedPreset].ext;

  const autoIncrement = (base: string): string => {
    const baseName = base.replace(/\.[^.]+$/, '');
    let candidate = `${baseName}${ext}`;
    if (!existingNames.includes(candidate)) return candidate;
    let i = 1;
    while (existingNames.includes(`${baseName}(${i})${ext}`)) i++;
    return `${baseName}(${i})${ext}`;
  };

  const handleCreate = () => {
    const raw = name.trim();
    const base = raw ? raw.replace(/\.[^.]+$/, '') : 'item';
    const finalName = autoIncrement(base);
    const template = PRESETS[selectedPreset].template;
    onSelect(template, finalName);
  };

  const handlePresetChange = (i: number) => {
    setSelectedPreset(i);
    const raw = name.trim();
    if (!raw || PRESETS.some(p => raw.endsWith(p.ext))) {
      const base = raw ? raw.replace(/\.[^.]+$/, '') : '';
      setName(base ? `${base}${PRESETS[i].ext}` : '');
    }
  };

  return (
    <div className="confirm-overlay" onClick={onCancel}>
      <div className="confirm-dialog" style={{ minWidth: 380 }} onClick={e => e.stopPropagation()}>
        <h3 style={{ fontSize: '1rem', fontWeight: 600, marginBottom: 12 }}><Icon icon="file" /> 新建文件</h3>

        <input
          autoFocus
          value={name}
          onChange={e => setName(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && handleCreate()}
          placeholder={`item${ext}`}
          style={{
            width: '100%', boxSizing: 'border-box',
            padding: '6px 10px', marginBottom: 10,
            border: '1px solid var(--gray-300)', borderRadius: 6,
            fontSize: '0.875rem', fontFamily: 'var(--font-mono)',
          }}
        />

        <div style={{ display: 'flex', gap: 4, marginBottom: 16, flexWrap: 'wrap' }}>
          {PRESETS.map((p, i) => (
            <button
              key={p.ext}
              className={`toolbar-btn ${i === selectedPreset ? 'toolbar-btn--primary' : ''}`}
              onClick={() => handlePresetChange(i)}
              style={{ borderRadius: 6 }}
            >
              <Icon icon={p.icon} /> {p.label}
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

export default NewScriptDialog;
