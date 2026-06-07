// renderers/useConsole.ts — .cmdv 控制台状态管理 Hook（v3）
//
// 职责：
//   1. 初始化/销毁 xterm.js 终端实例（scrollback=10000, FitAddon, WebLinksAddon）
//   2. 管理会话数据（输入/输出/时间戳/退出码）
//   3. 命令执行生命周期（Enter 发送, Ctrl+C 中断）
//   4. R13 交互式输入支持（后端 emit lua-input-request → 前端 listen → 显示 ?  提示符）
//   5. 全局命令历史（localStorage，500 条上限，去重）
//   6. 保存/加载 VFS 中的 .cmdv JSON 文件
//   7. 导出 HTML/MD/TXT 到真实文件系统
//   8. R4 标签关闭 + 应用退出时自动保存

import { useRef, useEffect, useCallback, useState } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { error as logError } from '@tauri-apps/plugin-log';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { readFile, writeFile } from '../api/vfs';
import { luaExec, luaSendInput, luaInterrupt, cmdvExport } from '../api/console';
import type { CmdvSession, CmdvRecord, LuaExecResult } from '../types';

// =========================================================================
// 常量
// =========================================================================

const HISTORY_KEY = 'cmdv_command_history';
const HISTORY_MAX = 500;
const SCROLLBACK_LINES = 10_000; // v3: 从 5000 增大到 10000

/** xterm.js 终端主题（深色，与编辑器风格协调） */
const TERMINAL_THEME = {
  background: '#1a1a2e',
  foreground: '#e0e0e0',
  cursor: '#e94560',
  cursorAccent: '#1a1a2e',
  selectionBackground: '#0f3460',
  black: '#1a1a2e',
  red: '#e94560',
  green: '#5cb85c',
  yellow: '#f0ad4e',
  blue: '#5bc0de',
  magenta: '#c77dff',
  cyan: '#00b4d8',
  white: '#e0e0e0',
  brightBlack: '#4a4a5a',
  brightRed: '#ff6b6b',
  brightGreen: '#6bff6b',
  brightYellow: '#ffd93d',
  brightBlue: '#6bc5d9',
  brightMagenta: '#d4a5ff',
  brightCyan: '#5ce1e6',
  brightWhite: '#ffffff',
};

// =========================================================================
// 命令历史管理（纯函数）
// =========================================================================

function loadHistory(): string[] {
  try {
    const raw = localStorage.getItem(HISTORY_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function pushHistory(command: string): void {
  const trimmed = command.trim();
  if (!trimmed) return;
  const history = loadHistory().filter((h) => h !== trimmed);
  history.unshift(trimmed);
  if (history.length > HISTORY_MAX) {
    history.length = HISTORY_MAX;
  }
  localStorage.setItem(HISTORY_KEY, JSON.stringify(history));
}

// =========================================================================
// 默认会话结构
// =========================================================================

function createEmptySession(): CmdvSession {
  return {
    records: [],
    createdAt: new Date().toISOString(),
  };
}

// =========================================================================
// Hook
// =========================================================================

export function useConsole(vfsPath: string | null) {
  const terminalRef = useRef<HTMLDivElement>(null);
  const terminal = useRef<Terminal | null>(null);
  const fitAddon = useRef<FitAddon | null>(null);
  const [isRunning, setIsRunning] = useState(false);
  const [session, setSession] = useState<CmdvSession>(createEmptySession());

  // 挂载 ref（用于闭包中获取最新值）
  const sessionRef = useRef(session);
  const isRunningRef = useRef(isRunning);
  const pendingInputUnlisten = useRef<UnlistenFn | null>(null);
  const historyIndexRef = useRef(-1);
  const currentInputRef = useRef('');
  // R13: 交互式输入模式标志
  const isInputModeRef = useRef(false);
  const inputBufferRef = useRef('');

  sessionRef.current = session;
  isRunningRef.current = isRunning;

  // ── 初始化终端 ──

  const initTerminal = useCallback(() => {
    if (!terminalRef.current || terminal.current) return;

    const term = new Terminal({
      theme: TERMINAL_THEME,
      fontSize: 14,
      fontFamily: 'var(--font-mono), "Cascadia Code", Consolas, monospace',
      cursorBlink: true,
      cursorStyle: 'bar',
      allowTransparency: false,
      scrollback: SCROLLBACK_LINES,
      tabStopWidth: 4,
    });

    const fit = new FitAddon();
    const webLinks = new WebLinksAddon();

    term.loadAddon(fit);
    term.loadAddon(webLinks);
    term.open(terminalRef.current);

    const resizeObserver = new ResizeObserver(() => {
      try { fit.fit(); } catch { /* ignored */ }
    });
    resizeObserver.observe(terminalRef.current);

    fit.fit();
    term.focus();

    terminal.current = term;
    fitAddon.current = fit;

    return () => {
      resizeObserver.disconnect();
      term.dispose();
      terminal.current = null;
    };
  }, []);

  // ── 终端提示符（v3: 使用 writeln 批量写入）──

  const writePrompt = useCallback(() => {
    terminal.current?.writeln('\r\n\x1b[92mlua>\x1b[0m ');
  }, []);

  // ── 回放历史输出 ──

  const replayOutput = useCallback((records: CmdvRecord[]) => {
    const term = terminal.current;
    if (!term) return;
    for (const rec of records) {
      term.writeln(`\x1b[92mlua>\x1b[0m ${rec.input}`);
      if (rec.output) {
        term.write(rec.output);
        if (!rec.output.endsWith('\n')) term.writeln('');
      }
      if (rec.exitCode !== undefined && rec.exitCode !== 0) {
        term.writeln(`\x1b[91m[退出码: ${rec.exitCode}]\x1b[0m`);
      }
    }
    writePrompt();
  }, [writePrompt]);

  // ── 加载 .cmdv 文件 ──

  const loadSession = useCallback(async (path: string) => {
    try {
      const raw = await readFile(path);
      const parsed: CmdvSession = JSON.parse(raw);
      if (!Array.isArray(parsed.records)) {
        throw new Error('无效的 .cmdv 文件格式');
      }
      setSession(parsed);
      setTimeout(() => replayOutput(parsed.records), 100);
    } catch {
      const empty = createEmptySession();
      setSession(empty);
      setTimeout(() => {
        terminal.current?.writeln('\x1b[90m[新控制台会话已就绪 — 输入 Lua 代码开始]\x1b[0m');
        writePrompt();
      }, 100);
    }
  }, [replayOutput, writePrompt]);

  // ── 保存会话到 VFS ──

  const saveSession = useCallback(async () => {
    if (!vfsPath) return;
    const data = JSON.stringify(sessionRef.current, null, 2);
    await writeFile(vfsPath, data);
  }, [vfsPath]);

  // ── 执行单条命令 ──

  const executeCommand = useCallback(async (code: string, tabId: string) => {
    const term = terminal.current;
    if (!term || !code.trim()) return;

    setIsRunning(true);
    isRunningRef.current = true;

    const record: CmdvRecord = {
      input: code,
      output: '',
      timestamp: new Date().toISOString(),
    };

    try {
      const result: LuaExecResult = await luaExec(tabId, code);
      record.output = result.output ?? '';
      record.exitCode = result.exitCode;

      if (result.output) {
        term.writeln(result.output);
      }
      if (result.exitCode !== 0) {
        term.writeln(`\x1b[91m[退出码: ${result.exitCode}]\x1b[0m`);
      }
    } catch (err) {
      const msg = String(err);
      record.output = msg;
      record.exitCode = -1;
      term.writeln(`\x1b[91m[错误: ${msg}]\x1b[0m`);
    }

    const nextSession = {
      ...sessionRef.current,
      records: [...sessionRef.current.records, record],
    };
    setSession(nextSession);
    sessionRef.current = nextSession;

    setIsRunning(false);
    isRunningRef.current = false;

    pushHistory(code);
    writePrompt();
  }, [writePrompt]);

  // ── 中断执行 ──

  const handleInterrupt = useCallback(async (tabId: string) => {
    try {
      await luaInterrupt(tabId);
      terminal.current?.writeln('\r\n\x1b[93m^C — 已中断\x1b[0m');
    } catch (err) {
      logError(`[useConsole] 中断失败: ${err}`);
    }
    setIsRunning(false);
    isRunningRef.current = false;
    writePrompt();
  }, [writePrompt]);

  // ── 清屏 ──

  const clearTerminal = useCallback(() => {
    terminal.current?.clear();
    writePrompt();
  }, [writePrompt]);

  // ── 导出 ──

  const exportSession = useCallback(async (format: 'html' | 'md' | 'txt') => {
    if (!vfsPath) return;
    await cmdvExport(vfsPath, format);
  }, [vfsPath]);

  // ── R13: 设置交互式输入监听 ──

  const setupInputRequestListener = useCallback((tabId: string) => {
    // 清理旧的监听
    if (pendingInputUnlisten.current) {
      pendingInputUnlisten.current();
      pendingInputUnlisten.current = null;
    }

    listen<{ tab_id: string; prompt?: string }>('lua-input-request', (event) => {
      if (event.payload.tab_id !== tabId) return;

      const term = terminal.current;
      if (!term) return;

      // 进入交互输入模式
      isInputModeRef.current = true;

      // 显示特殊提示符 ? （区别于正常 lua>）
      term.write('\x1b[93m?\x1b[0m ');
      inputBufferRef.current = '';
    }).then((fn) => {
      pendingInputUnlisten.current = fn;
    });
  }, []);

  // ── 键盘输入处理 ──

  const setupKeyboardHandler = useCallback((tabId: string) => {
    const term = terminal.current;
    if (!term) return;

    let inputBuffer = '';

    term.onData((data) => {
      // ── Ctrl+C 在任何模式下都可用 ──
      if (data === '\x03') {
        // R13: 如果在交互输入模式，发送空输入以中断
        if (isInputModeRef.current) {
          isInputModeRef.current = false;
          luaSendInput(tabId, '').catch(() => {});
          term.writeln('^C');
          writePrompt();
          return;
        }
        // 正常模式 — 中断执行
        if (isRunningRef.current) {
          handleInterrupt(tabId);
        } else {
          inputBuffer = '';
          currentInputRef.current = '';
          term.writeln('^C');
          writePrompt();
        }
        return;
      }

      // ── R13: 交互输入模式 ──
      if (isInputModeRef.current) {
        if (data === '\r') {
          // Enter — 发送输入
          const input = inputBufferRef.current;
          isInputModeRef.current = false;
          term.writeln('');
          luaSendInput(tabId, input).catch((err) => {
            logError(`[useConsole] 发送交互输入失败: ${err}`);
          });
          inputBufferRef.current = '';
          writePrompt();
        } else if (data === '\x7f') {
          // Backspace
          if (inputBufferRef.current.length > 0) {
            inputBufferRef.current = inputBufferRef.current.slice(0, -1);
            term.write('\b \b');
          }
        } else if (data.length === 1 && data.charCodeAt(0) >= 32) {
          inputBufferRef.current += data;
          term.write(data);
        }
        return;
      }

      // ── 正常模式（非运行中）──
      if (isRunningRef.current) return;

      if (data === '\r') {
        // Enter — 执行命令
        const code = inputBuffer.trim();
        term.writeln('');
        inputBuffer = '';
        currentInputRef.current = '';

        if (code) {
          historyIndexRef.current = -1;
          executeCommand(code, tabId);
        } else {
          writePrompt();
        }
      } else if (data === '\x7f') {
        // Backspace
        if (inputBuffer.length > 0) {
          inputBuffer = inputBuffer.slice(0, -1);
          currentInputRef.current = inputBuffer;
          term.write('\b \b');
        }
      } else if (data === '\x1b[A') {
        // Up arrow — 上一条历史
        const history = loadHistory();
        if (history.length > 0) {
          if (historyIndexRef.current < history.length - 1) {
            historyIndexRef.current++;
          }
          const h = history[historyIndexRef.current] ?? history[0];
          term.write('\r\x1b[92mlua>\x1b[0m \x1b[K');
          term.write(h);
          inputBuffer = h;
          currentInputRef.current = h;
        }
      } else if (data === '\x1b[B') {
        // Down arrow — 下一条历史
        const history = loadHistory();
        if (historyIndexRef.current > 0) {
          historyIndexRef.current--;
          const h = history[historyIndexRef.current];
          term.write('\r\x1b[92mlua>\x1b[0m \x1b[K');
          term.write(h);
          inputBuffer = h;
          currentInputRef.current = h;
        } else if (historyIndexRef.current === 0) {
          historyIndexRef.current = -1;
          term.write('\r\x1b[92mlua>\x1b[0m \x1b[K');
          inputBuffer = '';
          currentInputRef.current = '';
        }
      } else if (data.length === 1 && data.charCodeAt(0) >= 32) {
        // Printable character
        inputBuffer += data;
        currentInputRef.current = inputBuffer;
        term.write(data);
      }
    });
  }, [executeCommand, handleInterrupt, writePrompt]);

  // ── R4: 标签关闭 + 应用退出时自动保存 ──

  const setupAutoSave = useCallback((tabId: string) => {
    const handleBeforeUnload = () => {
      const currentSession = sessionRef.current;
      if (currentSession.records.length > 0) {
        const data = JSON.stringify(currentSession, null, 2);
        // 同步写入（beforeunload 不支持 async）
        writeFile(tabId, data).catch((err) => {
          logError(`[useConsole] 退出时保存失败: ${err}`);
        });
      }
    };
    window.addEventListener('beforeunload', handleBeforeUnload);
    return () => window.removeEventListener('beforeunload', handleBeforeUnload);
  }, []);

  // ── 生命周期 ──

  useEffect(() => {
    if (!vfsPath) return;

    const cleanup = initTerminal();
    loadSession(vfsPath);

    const setupTimer = setTimeout(() => {
      setupKeyboardHandler(vfsPath);
      setupInputRequestListener(vfsPath);
    }, 200);

    const cleanupAutoSave = setupAutoSave(vfsPath);

    return () => {
      clearTimeout(setupTimer);
      if (pendingInputUnlisten.current) {
        pendingInputUnlisten.current();
        pendingInputUnlisten.current = null;
      }
      cleanupAutoSave();
      // R4: 标签关闭时保存
      const currentSession = sessionRef.current;
      if (currentSession.records.length > 0) {
        const data = JSON.stringify(currentSession, null, 2);
        writeFile(vfsPath, data).catch((err) => {
          logError(`[useConsole] 关闭时保存失败: ${err}`);
        });
      }
      cleanup?.();
    };
  }, [vfsPath, initTerminal, loadSession, setupKeyboardHandler, setupInputRequestListener, setupAutoSave]);

  return {
    terminalRef,
    isRunning,
    session,
    saveSession,
    exportSession,
    clearTerminal,
  };
}
