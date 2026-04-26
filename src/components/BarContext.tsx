import React, { createContext, useContext, useState, ReactNode, useEffect } from 'react';
import { error } from '@tauri-apps/plugin-log';

/**
 * Bar 上下文的类型定义
 * @interface BarContextType
 * @property {ReactNode | null} barContent - 要在 Bar 中渲染的内容
 * @property {(content: ReactNode | null) => void} setBarContent - 更新 Bar 内容的函数
 */
interface BarContextType {
  barContent: ReactNode | null;
  setBarContent: (content: ReactNode | null) => void;
}

/**
 * Bar 上下文实例
 * 用于在组件树中共享 Bar 的状态
 * 初始值为 undefined，在使用前必须通过 BarProvider 提供
 */
const BarContext = createContext<BarContextType | undefined>(undefined);

/**
 * Bar 提供者组件
 * 
 * 功能说明：
 * - 提供全局的 Bar 状态管理
 * - 通过 Context 向子组件分发 barContent 状态和 setBarContent 更新函数
 * 
 * 使用示例：
 * ```tsx
 * <BarProvider>
 *   <App />
 * </BarProvider>
 * ```
 * 
 * @param {Object} props - 组件属性
 * @param {ReactNode} props.children - 子组件
 * @returns {React.ReactElement} Context Provider 包装的子组件
 */
export const BarProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  // 管理 Bar 内容的 state，初始值为 null 表示没有内容
  const [barContent, setBarContent] = useState<ReactNode | null>(null);

  return (
    <BarContext.Provider value={{ barContent, setBarContent }}>
      {children}
    </BarContext.Provider>
  );
};

/**
 * 自定义 Hook：获取 Bar 上下文
 * 
 * 功能说明：
 * - 获取当前的 barContent 和 setBarContent
 * - 如果未在 BarProvider 内使用，会抛出错误提示
 * 
 * 使用示例：
 * ```tsx
 * const { barContent, setBarContent } = useBar();
 * ```
 * 
 * @returns {BarContextType} 包含 barContent 和 setBarContent 的上下文对象
 * @throws {Error} 如果在 BarProvider 外部调用此 Hook，会抛出错误
 */
export const useBar = (): BarContextType => {
  const context = useContext(BarContext);
  
  // 安全检查：确保 Hook 在 BarProvider 内部使用
  if (!context) {
    error("确保 Hook 在 BarProvider 内部使用")
    throw new Error(
      'useBar must be used within a BarProvider. ' +
      'Please wrap your component tree with <BarProvider>.'
    );
  }
  
  return context;
};

/**
 * 自定义 Hook：设置 Bar 内容（带自动清理）
 * 
 * 功能说明：
 * - 组件挂载时，将传入的 content 设置为 Bar 的内容
 * - 组件卸载时，自动将 Bar 内容恢复为 null（清理内容）
 * - 当 content 发生变化时，会自动更新 Bar 的内容
 * 
 * 适用场景：
 * - 需要在特定组件中动态显示/隐藏 Bar 内容
 * - 组件销毁时自动清理 Bar，防止残留上一次的内容
 * 
 * 使用示例：
 * ```tsx
 * function MyComponent() {
 *   useBarContent(<div>My Bar Content</div>);
 *   return <div>My Component</div>;
 * }
 * ```
 * 
 * 注意事项：
 * - 确保在 BarProvider 的子组件中使用
 * - content 更新会触发 Bar 内容的重新渲染
 * - 组件卸载时会自动清空 Bar，无需手动清理
 * 
 * @param {ReactNode | null} content - 要显示在 Bar 中的内容，传入 null 则清空
 */
export const useBarContent = (content: ReactNode | null): void => {
  const { setBarContent } = useBar();

  useEffect(() => {
    // 组件挂载或 content 更新时，设置 Bar 内容
    setBarContent(content);
    
    // 清理函数：组件卸载时将 Bar 内容恢复为 null
    return () => {
      setBarContent(null);
    };
  }, [content, setBarContent]); // 依赖项：content 变化时重新执行
};