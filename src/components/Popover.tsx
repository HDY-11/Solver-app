import { useState, useRef } from 'react';

interface PopoverProps {
  children: React.ReactNode;
  content: React.ReactNode;
}

function Popover({ children, content }: PopoverProps) {
  const [visible, setVisible] = useState(false);
  const triggerRef = useRef<HTMLDivElement>(null);

  const timerRef = useRef<number | null>(null);

  const handleMouseEnter = () => {
    if (timerRef.current) clearTimeout(timerRef.current);
    
    timerRef.current = setTimeout(() => {
      setVisible(true);
    }, 2000);
  };

  const handleMouseLeave = () => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    timerRef.current = setTimeout(() => {
      setVisible(false);
    }, 100);
  };

  return (
    <div
      ref={triggerRef}
      style={{ position: 'relative', display: 'inline-block' }}
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
    >
      {children}
      {visible && (
        <div
          style={{
            position: 'absolute',
            top: '100%',
            left: '50%',
            transform: 'translateX(-50%)',
            marginTop: 8,
            padding: '6px 12px',
            background: '#333',
            color: '#fff',
            borderRadius: 4,
            fontSize: 14,
            whiteSpace: 'nowrap',
            zIndex: 1000,
          }}
        >
          {content}
        </div>
      )}
    </div>
  );
}

export default Popover;