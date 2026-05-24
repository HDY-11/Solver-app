import { registerRenderer } from '../registry/registry';
import type { RendererProps } from '../registry/types';

function HtmlViewer({ nodeId }: RendererProps) {
  return <div style={{ padding: 24 }}>🌐 HTML 查看器 — 节点 {nodeId}</div>;
}

registerRenderer({
  name: 'html',
  extensions: ['.html', '.htm'],
  component: HtmlViewer,
  icon: '🌐',
  label: '浏览器',
});

export default HtmlViewer;