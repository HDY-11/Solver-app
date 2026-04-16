import React from 'react';
import FilePickerNative from '../components/FilePicker';

const Home: React.FC = () => {
  const handleFileSelect = (fileName: string) => {
    console.log('选中的文件:', fileName);
    // 注意：这里只能获取文件名，不能获取完整路径
  };

  return (
    <div style={{ padding: '20px' }}>
      <h1>文件选择器</h1>
      
      <div style={{ marginBottom: '20px' }}>
        <h3>基础文件选择</h3>
        <FilePickerNative 
          onFileSelect={handleFileSelect}
          buttonText="📁 选择文件"
        />
      </div>

      <div style={{ marginBottom: '20px' }}>
        <h3>选择图片文件</h3>
        <FilePickerNative 
          onFileSelect={handleFileSelect}
          buttonText="🖼️ 选择图片"
        />
      </div>

      <div style={{ marginBottom: '20px' }}>
        <h3>多文件选择</h3>
        <FilePickerNative 
          onFileSelect={handleFileSelect}
          buttonText="📄 选择多个文件"
          multiple={true}
        />
      </div>
    </div>
  );
};

export default Home;