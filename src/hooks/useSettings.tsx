// hooks/useSettings.tsx — 应用设置上下文
//
// 全局单例：从后端加载配置，变更时同步写入磁盘并通知所有消费者。
// 替代各组件各自调 localStorage / invoke read_settings。

import { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from 'react';
import { error as logError } from '@tauri-apps/plugin-log';
import {
  readSettings,
  writeSettings,
  resetSettings as resetBackend,
  DEFAULT_SETTINGS,
  type AppSettings,
} from '../api/config';

interface SettingsContextValue {
  settings: AppSettings;
  update: (partial: Partial<AppSettings>) => Promise<void>;
  reset: () => Promise<void>;
  reload: () => Promise<void>;
}

const SettingsContext = createContext<SettingsContextValue | null>(null);

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);

  // 启动时从后端加载
  const reload = useCallback(async () => {
    try {
      const s = await readSettings();
      setSettings(s);
    } catch (err) {
      logError(`[settings] 加载失败: ${err}`);
    }
  }, []);

  useEffect(() => { reload(); }, [reload]);

  // 更新部分配置 → 写入磁盘 → 通知所有消费者
  const update = useCallback(async (partial: Partial<AppSettings>) => {
    const next = { ...settings, ...partial };
    // 先更新 UI（即时响应）
    setSettings(next);
    try {
      await writeSettings(next);
    } catch (err) {
      logError(`[settings] 写入失败: ${err}`);
      // 写入失败回滚 UI
      setSettings(settings);
      throw err;
    }
  }, [settings]);

  // 重置为默认
  const reset = useCallback(async () => {
    try {
      const def = await resetBackend();
      setSettings(def);
    } catch (err) {
      logError(`[settings] 重置失败: ${err}`);
    }
  }, []);

  return (
    <SettingsContext.Provider value={{ settings, update, reset, reload }}>
      {children}
    </SettingsContext.Provider>
  );
}

/** 消费设置上下文 */
export function useSettings() {
  const ctx = useContext(SettingsContext);
  if (!ctx) throw new Error('useSettings 必须在 SettingsProvider 内使用');
  return ctx;
}
