// hooks/useTabs.tsx — 标签页管理
//
// tabs 是唯一状态。活跃标签从 URL pathname 派生，消除 activePath 竞态和双渲染。

import { createContext, useContext, useState, useCallback, type ReactNode } from 'react';

export interface TabInfo {
  path: string;
  label: string;
  icon: string;
}

interface TabsContextValue {
  tabs: TabInfo[];
  registerTab: (info: TabInfo) => void;
  closeTab: (path: string) => void;
}

const TabsContext = createContext<TabsContextValue | null>(null);

export function TabsProvider({ children }: { children: ReactNode }) {
  const [tabs, setTabs] = useState<TabInfo[]>([]);

  const registerTab = useCallback((info: TabInfo) => {
    setTabs((prev) => {
      if (prev.some((t) => t.path === info.path)) return prev;
      return [...prev, info];
    });
  }, []);

  const closeTab = useCallback((path: string) => {
    setTabs((prev) => prev.filter((t) => t.path !== path));
  }, []);

  return (
    <TabsContext.Provider value={{ tabs, registerTab, closeTab }}>
      {children}
    </TabsContext.Provider>
  );
}

export function useTabs() {
  const ctx = useContext(TabsContext);
  if (!ctx) throw new Error('useTabs 必须在 TabsProvider 内使用');
  return ctx;
}
