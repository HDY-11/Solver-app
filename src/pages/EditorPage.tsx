import React, { useState, useMemo } from "react";
import Editor from '@monaco-editor/react';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { useBarContent } from '../components/BarContext';
import { info, warn, error } from '@tauri-apps/plugin-log';

interface EditorPageProps {
    defaultLanguage: string;
    display:Boolean;
}

const EditorPage: React.FC<EditorPageProps> = ({
    defaultLanguage = 'python',
    display
}) => {
    const [code, setCode] = useState<string>('');
    const [path, setPath] = useState<string | null>(null);

    const handleEditorChange = (value: string | undefined) => {
        setCode(value || '');
    };

    const handleSave = async () => {
        if (!path) {
            alert('请先加载或指定文件路径');
            warn('未加载文件路径，使用保存')
            return;
        }
        try {
            await invoke('save_script', { code, path });
            alert('保存成功');
            info('保存成功');
        } catch (err) {
            alert(`保存失败:\n${err}`);
            error(`保存失败:\n${err}`);
        }
    };

    const handleLoad = async () => {
        try {
            const selectedPath = await open({
                filters: [{
                    name: 'Python Files',
                    extensions: ['py'],
                }]
            });
            
            if (selectedPath) {
                const content = await invoke<string>('read_script', { path: selectedPath });
                setCode(content);
                setPath(selectedPath);
            }
        } catch (error) {
            alert(`加载文件失败:\n${error}`);
        }
    };

    const handleRun = async () => {
        if (!path) {
            alert('请先保存或加载文件');
            return;
        }
        try {
            await invoke<string>('run_script', { path });
        } catch (error) {
            alert(`执行失败:\n${error}`);
        }
    };

    // 构建工具栏内容，包含按钮组和文件路径显示
    const toolbarContent = useMemo(() => (
        <>
            <div className="bar-actions" style={{display: display ? undefined : 'none'}}>
                <button className="btn btn-sm" onClick={handleLoad}>
                    📂 加载
                </button>
                <button className="btn btn-primary btn-sm" onClick={handleSave}>
                    💾 保存
                </button>
                <button className="btn btn-primary btn-sm" onClick={handleRun}>
                    ▶ 运行
                </button>
            </div>
            {path && (
                <span className="current-file" title={path} style={{display: display ? undefined : 'none'}}>
                    {path}
                </span>
            )}
        </>
    ), [path, code, display]);

    useBarContent(toolbarContent);

    return (
        <div className="page-container" style={{ padding: 0, overflow: 'hidden', display: display? undefined : 'none' }}>
            <Editor
                height="100%"
                defaultLanguage={defaultLanguage}
                value={code}
                onChange={handleEditorChange}
                theme="vs-dark"
                options={{
                    minimap: { enabled: false },
                    fontSize: 14,
                    fontFamily: 'var(--font-mono)',
                    padding: { top: 16, bottom: 16 },
                    scrollBeyondLastLine: false,
                    automaticLayout: true,
                }}
            />

        </div>
    );
};

export default EditorPage;