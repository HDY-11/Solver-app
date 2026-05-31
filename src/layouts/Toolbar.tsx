// layouts/Toolbar.tsx
//
// 根据当前 URL 路径匹配 renderer 并渲染其工具栏。
// 因为 Toolbar 在 <Routes> 外部，不能使用 useParams()，
// 改用 useLocation() 手动解析路径。

import { useLocation } from 'react-router-dom';
import { getRenderer } from '../registry/registry';

/** 从 /app/py/xxx 格式路径中提取 renderer 名称 */
function getRendererFromPath(pathname: string): string | null {
  const parts = pathname.split('/').filter(Boolean);
  // 期望格式: /app/:renderer/...
  if (parts.length >= 2 && parts[0] === 'app') {
    return parts[1];
  }
  return null;
}

/** 从 /app/py/xxx 格式路径中提取 content（编码后的 VFS 路径，支持多段） */
function getContentFromPath(pathname: string): string | null {
  const parts = pathname.split('/').filter(Boolean);
  if (parts.length >= 3 && parts[0] === 'app') {
    return parts.slice(2).join('/');
  }
  return null;
}

function Toolbar() {
  const { pathname } = useLocation();
  const renderer = getRendererFromPath(pathname);
  const content = getContentFromPath(pathname);
  const rendererDef = renderer ? getRenderer(renderer) : undefined;

  return (
    <div className="app-toolbar">
      {rendererDef?.toolbar?.({ nodeId: content ?? null })}
    </div>
  );
}

export default Toolbar;