import { createContext, useContext, useState, ReactNode } from 'react';

interface WindowContextType {
  detachedWindows: string[];
  detachWindow: (path: string) => void;
  closeWindow: (path: string) => void;
}

const WindowContext = createContext<WindowContextType | null>(null);

export function WindowProvider({ children }: { children: ReactNode }) {
  const [detachedWindows, setDetachedWindows] = useState<string[]>([]);

  const detachWindow = (path: string) => setDetachedWindows(prev => [...prev, path]);
  const closeWindow = (path: string) => setDetachedWindows(prev => prev.filter(p => p !== path));

  return (
    <WindowContext.Provider value={{ detachedWindows, detachWindow, closeWindow }}>
      {children}
    </WindowContext.Provider>
  );
}

export function useWindow() {
  const ctx = useContext(WindowContext);
  if (!ctx) throw new Error('useWindow 必须在 WindowProvider 内使用');
  return ctx;
}