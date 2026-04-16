import React from "react";
import { useState } from "react";
import Editor from '@monaco-editor/react';
import { open } from '@tauri-apps/plugin-dialog';


interface EditorPageProps {
    defaultLanguage: string;
}


const EditorPage: React.FC<EditorPageProps> = ({
    defaultLanguage = 'python'
}) => {
    const [code, setCode] = useState<string>('');

    const handleEditorChange = (value: string | undefined) => {
        setCode(value || '');
    };

    const handleSave = () => {
        console.log("g");
    }

    const handleLoad = async () => {
        const path = await open({
            filters:[{
                name : 'a',
                extensions : ['py'],
            }]
        })
        console.log(path);
    }

    return (
        <div style={{ height: '100vh', width: '100%' }}>
            <button 
                className="btn"
                onClick={handleSave}
                disabled={false}
            >保存
            </button>
            <button 
                className="btn"
                onClick={handleLoad}
            >加载
            </button>
            <Editor
                height="100%"
                defaultLanguage={defaultLanguage}
                onChange={handleEditorChange}
            />
        </div>
    );
}

export default EditorPage;