// layouts/Toolbar.tsx

import { useParams } from 'react-router-dom';
import { getRenderer } from '../registry/registry';

function Toolbar() {
  const { renderer, content } = useParams();
  const rendererDef = renderer ? getRenderer(renderer) : undefined;

  return (
    <div className="app-toolbar">
      {rendererDef?.toolbar?.({ nodeId: content ?? null })}
    </div>
  );
}

export default Toolbar;