import { useParams, useNavigate } from 'react-router-dom';
import { getRenderer } from '../registry/registry';

function Nav() {
  const { renderer, content } = useParams();
  const navigate = useNavigate();
  const rendererDef = renderer ? getRenderer(renderer) : undefined;

  // 当前标签信息
  const hasTab = rendererDef && content;

  return (
    <nav className="app-nav">
      {hasTab ? (
        <span
          style={{
            padding: '2px 12px', fontSize: '0.8125rem',
            background: 'var(--gray-200)', borderRadius: 4,
            display: 'flex', alignItems: 'center', gap: 6,
          }}
        >
          {rendererDef!.icon} {content}
        </span>
      ) : (
        <span style={{ color: 'var(--gray-500)', fontSize: '0.8125rem' }}>
          无打开的文件
        </span>
      )}
    </nav>
  );
}

export default Nav;