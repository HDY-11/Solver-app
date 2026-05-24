import { useNavigate, useLocation } from 'react-router-dom';

function Header() {
  const navigate = useNavigate();
  const location = useLocation();

  return (
    <header className="app-header">
      <div style={{ display: 'flex', gap: 4 }}>
        <button onClick={() => navigate(-1)} title="后退">←</button>
        <button onClick={() => navigate(1)} title="前进">→</button>
      </div>
      <input
        style={{
          flex: 1, margin: '0 12px', padding: '2px 8px',
          border: '1px solid var(--gray-300)', borderRadius: 4,
          fontSize: '0.8125rem', fontFamily: 'var(--font-mono)',
        }}
        value={location.pathname + location.search}
        readOnly
        placeholder="输入路径、命令或 URL"
      />
    </header>
  );
}

export default Header;