import React, { useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';

interface FilePickerProps {
  onFileSelect?: (filePath: string) => void;
  buttonText?: string;
  className?: string;
  filters?: {
    name: string;
    extensions: string[];
  }[];
  multiple?: boolean;
  directory?: boolean;
}

const FilePicker: React.FC<FilePickerProps> = ({
  onFileSelect,
  buttonText = '选择文件',
  className = '',
  filters = [],
  multiple = false,
  directory = false,
}) => {
  const [selectedPath, setSelectedPath] = useState<string>('');
  const [error, setError] = useState<string>('');

  const handleFilePick = async () => {
    try {
      setError('');
      
      const selected = await open({
        multiple,
        directory,
        filters: filters.length > 0 ? filters : undefined,
        title: directory ? '选择文件夹' : '选择文件',
      });

      if (selected) {
        const path = Array.isArray(selected) ? selected[0] : selected;
        setSelectedPath(path);
        onFileSelect?.(path);
        
        // 如果允许多选，可以这样处理
        if (multiple && Array.isArray(selected)) {
          console.log('选中的文件:', selected);
        }
      }
    } catch (err) {
      setError('打开文件对话框失败');
      console.error('文件选择错误:', err);
    }
  };

  return (
    <div className={`file-picker ${className}`}>
      <button 
        onClick={handleFilePick}
        className="file-picker-button"
      >
        {buttonText}
      </button>
      
      {selectedPath && (
        <div className="selected-path">
          <span>已选择: </span>
          <code>{selectedPath}</code>
        </div>
      )}
      
      {error && (
        <div className="error-message">
          {error}
        </div>
      )}
    </div>
  );
};

export default FilePicker;