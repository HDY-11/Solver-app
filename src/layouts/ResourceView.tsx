import { useParams, useSearchParams } from 'react-router-dom';
import { getRenderer, getPanel } from '../registry/registry';

function ResourceView() {
  const { renderer, content } = useParams();
  const [searchParams] = useSearchParams();

  // 查渲染器
  const rendererDef = renderer ? getRenderer(renderer) : undefined;
  if (rendererDef) {
    if (rendererDef.name === 'browser') {
      return <rendererDef.component nodeId={searchParams.get('url')} />;
    }
    return <rendererDef.component nodeId={content ?? null} />;
  }

  // 查面板
  const panelDef = content ? getPanel(content) : undefined;
  if (panelDef) {
    return <panelDef.component />;
  }

  return <div style={{ padding: 24 }}>未知资源</div>;
}

export default ResourceView;