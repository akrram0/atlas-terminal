import { useEffect, useRef, useCallback } from 'react';
import { Terminal as XTerm } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import '@xterm/xterm/css/xterm.css';

const THEME = {
  background: 'transparent',
  foreground: '#ECEFF4',
  cursor: '#0A84FF',
  cursorAccent: '#121212',
  selectionBackground: 'rgba(10, 132, 255, 0.30)',
  selectionForeground: '#FFFFFF',
  black: '#1E1E22',
  brightBlack: '#5A5D66',
  red: '#FF6B6B',
  brightRed: '#FF8787',
  green: '#32D74B',
  brightGreen: '#4CD964',
  yellow: '#FFD60A',
  brightYellow: '#FFE043',
  blue: '#0A84FF',
  brightBlue: '#409CFF',
  magenta: '#BF5AF2',
  brightMagenta: '#DA8FFF',
  cyan: '#64D2FF',
  brightCyan: '#8BE0FF',
  white: '#E5E5EA',
  brightWhite: '#FFFFFF',
};

const isTauri = () => {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
};

export function Terminal() {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<XTerm | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const sessionIdRef = useRef<string | null>(null);

  const initTauri = useCallback(async (term: XTerm, fitAddon: FitAddon) => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const { listen } = await import('@tauri-apps/api/event');

      fitAddon.fit();
      const { cols, rows } = term;

      // Spawn PTY session
      const sessionId = await invoke<string>('spawn_pty', { cols, rows });
      sessionIdRef.current = sessionId;

      // Listen for PTY output
      await listen<string>(`pty-data-${sessionId}`, (event) => {
        term.write(event.payload);
      });

      // Listen for PTY exit
      await listen<number>(`pty-exit-${sessionId}`, (_event) => {
        term.write('\r\n\x1b[38;2;134;134;139m[Process exited]\x1b[0m\r\n');
      });

      // Send user input to PTY
      term.onData((data: string) => {
        invoke('write_pty', { sessionId, data }).catch(() => {});
      });

      // Handle resize
      term.onResize(({ cols, rows }: { cols: number; rows: number }) => {
        invoke('resize_pty', { sessionId, cols, rows }).catch(() => {});
      });
    } catch (err) {
      console.error('Tauri init failed:', err);
      initMock(term);
    }
  }, []);

  const initMock = useCallback((term: XTerm) => {
    // Browser dev mode: show a welcome message and echo input
    const user = 'user';
    const machine = 'atlas';
    const cwd = '~/Desktop/Project';
    
    const writePrompt = () => {
      term.write(
        '\x1b[38;2;50;215;75m●\x1b[0m ' +
        `\x1b[38;2;134;134;139m${user}@${machine}\x1b[0m ` +
        `\x1b[38;2;10;132;255m\uF07B\x1b[0m ` +
        `\x1b[38;2;200;200;205m${cwd}\x1b[0m ` +
        '\x1b[38;2;134;134;139m>\x1b[0m '
      );
    };

    term.write('\x1b[38;2;134;134;139m' +
      '╭──────────────────────────────────────╮\r\n' +
      '│     Atlas Terminal — Dev Mode        │\r\n' +
      '│     Tauri backend not connected      │\r\n' +
      '╰──────────────────────────────────────╯\r\n' +
      '\x1b[0m\r\n');
    writePrompt();

    let currentLine = '';
    term.onData((data: string) => {
      if (data === '\r') {
        term.write('\r\n');
        if (currentLine.trim()) {
          term.write(`\x1b[38;2;134;134;139mecho: ${currentLine}\x1b[0m\r\n`);
        }
        currentLine = '';
        writePrompt();
      } else if (data === '\x7f') {
        // Backspace
        if (currentLine.length > 0) {
          currentLine = currentLine.slice(0, -1);
          term.write('\b \b');
        }
      } else if (data >= ' ') {
        currentLine += data;
        term.write(data);
      }
    });
  }, []);

  useEffect(() => {
    if (!containerRef.current) return;

    const term = new XTerm({
      allowTransparency: true,
      fontFamily: '"JetBrains Mono Nerd Font", "MesloLGS NF", "SF Mono", Consolas, monospace',
      fontSize: 13.5,
      lineHeight: 1.25,
      letterSpacing: 0,
      cursorBlink: true,
      cursorStyle: 'block',
      theme: THEME,
      allowProposedApi: true,
      scrollback: 10000,
    });

    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.loadAddon(new WebLinksAddon());

    term.open(containerRef.current);
    fitAddon.fit();

    termRef.current = term;
    fitAddonRef.current = fitAddon;

    // Auto-resize on container size change
    const resizeObserver = new ResizeObserver(() => {
      requestAnimationFrame(() => {
        try {
          fitAddon.fit();
        } catch {}
      });
    });
    resizeObserver.observe(containerRef.current);

    // Initialize Tauri or mock mode
    if (isTauri()) {
      initTauri(term, fitAddon);
    } else {
      initMock(term);
    }

    return () => {
      resizeObserver.disconnect();
      term.dispose();
    };
  }, [initTauri, initMock]);

  return (
    <div
      ref={containerRef}
      style={{ width: '100%', height: '100%' }}
    />
  );
}
