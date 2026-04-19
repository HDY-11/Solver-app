import React, { createContext, useContext, useState, ReactNode } from 'react';

interface BarContextType {
  barContent: ReactNode | null;
  setBarContent: (content: ReactNode | null) => void;
}

const BarContext = createContext<BarContextType | undefined>(undefined);

export const BarProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  const [barContent, setBarContent] = useState<ReactNode | null>(null);

  return (
    <BarContext.Provider value={{ barContent, setBarContent }}>
      {children}
    </BarContext.Provider>
  );
};

export const useBar = () => {
  const context = useContext(BarContext);
  if (!context) {
    throw new Error('useBar must be used within a BarProvider');
  }
  return context;
};

export const useBarContent = (content: ReactNode | null) => {
  const { setBarContent } = useBar();

  React.useEffect(() => {
    setBarContent(content);
    return () => setBarContent(null);
  }, [content, setBarContent]);
};