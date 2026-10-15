import { useEffect, useRef, useState } from 'react';
import { Terminal as XTerm } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { SearchAddon } from '@xterm/addon-search';
import { Unicode11Addon } from '@xterm/addon-unicode11';
import { WebglAddon } from '@xterm/addon-webgl';
import { ChevronDown, ChevronUp, X } from 'lucide-react';
import '@xterm/xterm/css/xterm.css';

const THEME = {
  background: '#0E0E12',
  foreground: '#E4E4EE',
  cursor: '#F5E0DC',
  cursorAccent: '#0E0E12',
  selectionBackground: 'rgba(137, 180, 250, 0.25)',
  selectionForeground: '#FFFFFF',
  black: '#181820',
  brightBlack: '#585B70',
  red: '#F38BA8',
  brightRed: '#F38BA8',
  green: '#A6E3A1',
  brightGreen: '#A6E3A1',
  yellow: '#F9E2AF',
  brightYellow: '#F9E2AF',
  blue: '#89B4FA',
  brightBlue: '#89B4FA',
  magenta: '#CBA6F7',
  brightMagenta: '#F5C2E7',
  cyan: '#89DCEB',
  brightCyan: '#94E2D5',
  white: '#BAC2DE',
  brightWhite: '#FFFFFF',
};

const isTauri = () => {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
};

function sanitizeTitle(title: string): string {
  return title.replace(/\s+/g, ' ').trim().slice(0, 48);
}

async function copySelection(term: XTerm): Promise<boolean> {
  try {
    if (!term.hasSelection()) return false;
    const text = term.getSelection();
    if (!text) return false;
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

async function pasteClipboard(term: XTerm): Promise<boolean> {
  try {
    const text = await navigator.clipboard.readText();
    if (!text) return false;
    term.paste(text);
    return true;
  } catch {
    return false;
  }
}

function initMock(term: XTerm): { dispose: () => void } {
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

  term.write(
    '\x1b[38;2;134;134;139m' +
      '╭──────────────────────────────────────╮\r\n' +
      '│     Atlas Terminal — Dev Mode        │\r\n' +
      '│     Tauri backend not connected      │\r\n' +
      '╰──────────────────────────────────────╯\r\n' +
      '\x1b[0m\r\n'
  );
  writePrompt();

  let currentLine = '';
  const dataDisposable = term.onData((data: string) => {
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

  return dataDisposable;
}

export interface TerminalProps {
  active: boolean;
  /** Applied at construction and live. */
  fontFamily: string;
  /** Global default size (zoom writes back via onZoom). Applied live. */
  fontSize: number;
  /** Applied live. */
  lineHeight: number;
  /** Applied live. */
  cursorStyle: 'block' | 'bar' | 'underline';
  /** Applied live. */
  cursorBlink: boolean;
  /** GPU (WebGL2) renderer, like kitty/Ghostty. Applied live. */
  gpu: boolean;
  /** Applied at construction (new tabs). */
  scrollback: number;
  /** Backend shell for spawn_pty (new sessions, incl. restart). */
  shell: string;
  /** Startup dir for spawn_pty; empty = backend default. */
  cwd: string;
  onZoom: (delta: number) => void;
  onZoomReset: () => void;
  onTitle?: (title: string) => void;
  onRequestClose?: () => void;
}

export function Terminal({
  active,
  fontFamily,
  fontSize,
  lineHeight,
  cursorStyle,
  cursorBlink,
  gpu,
  scrollback,
  shell,
  cwd,
  onZoom,
  onZoomReset,
  onTitle,
  onRequestClose,
}: TerminalProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const restartBtnRef = useRef<HTMLButtonElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const termRef = useRef<XTerm | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const searchRef = useRef<SearchAddon | null>(null);
  const gpuAddonRef = useRef<WebglAddon | null>(null);
  const applyGpuRef = useRef<((on: boolean) => void) | null>(null);
  const restartRef = useRef<(() => void) | null>(null);
  const activeRef = useRef(active);
  activeRef.current = active;
  const onTitleRef = useRef(onTitle);
  onTitleRef.current = onTitle;
  const onZoomRef = useRef(onZoom);
  onZoomRef.current = onZoom;
  const onZoomResetRef = useRef(onZoomReset);
  onZoomResetRef.current = onZoomReset;

  const [exited, setExited] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [searchCase, setSearchCase] = useState(false);
  const [searchNoMatch, setSearchNoMatch] = useState(false);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const term = new XTerm({
      allowTransparency: true,
      fontFamily,
      fontSize,
      lineHeight,
      letterSpacing: 0.2,
      cursorBlink,
      cursorStyle,
      customGlyphs: true,
      theme: THEME,
      allowProposedApi: true,
      scrollback,
    });

    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.loadAddon(new WebLinksAddon());
    // Unicode 11 grapheme/width engine: correct cells for Powerline,
    // Nerd Font icons, and emoji (no overlaps, no phantom columns).
    const unicode11 = new Unicode11Addon();
    term.loadAddon(unicode11);
    term.unicode.activeVersion = '11';
    const searchAddon = new SearchAddon();
    term.loadAddon(searchAddon);
    searchRef.current = searchAddon;

    term.open(container);
    termRef.current = term;
    fitRef.current = fitAddon;

    // GPU renderer (WebGL2, same model as kitty/Ghostty): glyphs are
    // rasterized once into a texture atlas and drawn by the GPU. If the
    // context is lost (driver reset, OOM, sleep) or WebGL2 is missing,
    // xterm.js falls back to the DOM renderer automatically.
    const applyGpu = (on: boolean) => {
      if (on) {
        if (gpuAddonRef.current) return;
        try {
          const addon = new WebglAddon();
          addon.onContextLoss(() => {
            console.warn('[atlas] WebGL context lost — falling back to DOM renderer');
            try {
              addon.dispose();
            } catch {}
            if (gpuAddonRef.current === addon) gpuAddonRef.current = null;
          });
          term.loadAddon(addon);
          gpuAddonRef.current = addon;
        } catch (err) {
          console.warn('[atlas] WebGL renderer unavailable — using DOM renderer', err);
          gpuAddonRef.current = null;
        }
      } else if (gpuAddonRef.current) {
        const addon = gpuAddonRef.current;
        gpuAddonRef.current = null;
        try {
          addon.dispose();
        } catch {}
      }
    };
    applyGpuRef.current = applyGpu;
    if (gpu) applyGpu(true);
    // Only fit when visible: hidden tabs report 0 size and would spawn
    // a tiny PTY. Inactive tabs spawn at xterm defaults (80x24) and
    // get a proper fit when activated (see active effect below).
    if (activeRef.current) {
      try {
        fitAddon.fit();
      } catch {}
    }

    // Auto-resize on container size change (guard: ignore 0-size hidden tabs)
    const resizeObserver = new ResizeObserver(() => {
      if (!activeRef.current) return;
      requestAnimationFrame(() => {
        try {
          fitAddon.fit();
        } catch {}
      });
    });
    resizeObserver.observe(container);

    // ---- Lifecycle tracking (Phase 1: no leaked PTY, no duplicate listeners) ----
    let cancelled = false;
    let restarting = false;
    let spawning = false;
    let sessionId: string | null = null;
    const unlistens: Array<() => void> = [];
    let sessionDisposables: Array<{ dispose: () => void }> = [];
    const persistDisposables: Array<{ dispose: () => void }> = [];

    const closeSession = (sid: string | null) => {
      if (!sid) return;
      import('@tauri-apps/api/core')
        .then(({ invoke }) => invoke('close_pty', { sessionId: sid }).catch(() => {}))
        .catch(() => {});
    };

    const teardownSession = () => {
      while (unlistens.length > 0) {
        const fn = unlistens.pop();
        try {
          fn?.();
        } catch {}
      }
      while (sessionDisposables.length > 0) {
        const d = sessionDisposables.pop();
        try {
          d?.dispose();
        } catch {}
      }
      closeSession(sessionId);
      sessionId = null;
    };

    const spawnSession = async () => {
      // Never attach two sessions to one pane: a second concurrent spawn
      // would double-write every byte (duplicated input echo).
      if (spawning || sessionId) return;
      spawning = true;
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        const { listen } = await import('@tauri-apps/api/event');

        if (cancelled) return;
        if (activeRef.current) {
          try {
            fitAddon.fit();
          } catch {}
        }
        const { cols, rows } = term;

        // Spawn PTY session (shell/cwd frozen at tab creation)
        const sid = await invoke<string>('spawn_pty', {
          cols,
          rows,
          shell,
          cwd: cwd.trim() ? cwd.trim() : null,
        });
        if (cancelled) {
          // Unmounted while spawning: kill immediately, don't attach listeners.
          closeSession(sid);
          return;
        }
        sessionId = sid;

        // Listen for PTY output
        const unlistenData = await listen<string>(`pty-data-${sid}`, (event) => {
          term.write(event.payload);
        });
        if (cancelled) {
          unlistenData();
          closeSession(sid);
          return;
        }
        unlistens.push(unlistenData);

        // Listen for PTY exit
        const unlistenExit = await listen<number>(`pty-exit-${sid}`, () => {
          term.write('\r\n\x1b[38;2;134;134;139m[Process exited — R to restart]\x1b[0m\r\n');
          setExited(true);
        });
        if (cancelled) {
          unlistenData();
          unlistenExit();
          closeSession(sid);
          return;
        }
        unlistens.push(unlistenExit);

        // Send user input to PTY
        const dataDisposable = term.onData((data: string) => {
          invoke('write_pty', { sessionId: sid, data }).catch(() => {});
        });
        sessionDisposables.push(dataDisposable);

        // Handle resize
        const resizeDisposable = term.onResize(
          ({ cols, rows }: { cols: number; rows: number }) => {
            invoke('resize_pty', { sessionId: sid, cols, rows }).catch(() => {});
          }
        );
        sessionDisposables.push(resizeDisposable);
      } catch (err) {
        console.error('Tauri init failed:', err);
        if (!cancelled) {
          sessionDisposables.push(initMock(term));
        }
      } finally {
        spawning = false;
      }
    };

    restartRef.current = () => {
      if (restarting || cancelled) return;
      restarting = true;
      setExited(false);
      try {
        term.reset();
      } catch {}
      teardownSession();
      if (activeRef.current) {
        try {
          fitAddon.fit();
        } catch {}
      }
      void spawnSession().finally(() => {
        restarting = false;
        try {
          term.focus();
        } catch {}
      });
    };

    // Tab title sync: shell emits OSC 0 with cwd on every prompt.
    persistDisposables.push(
      term.onTitleChange((title: string) => {
        const clean = sanitizeTitle(title);
        if (clean) onTitleRef.current?.(clean);
      })
    );

    // Keyboard: clipboard + zoom. Return false to swallow (prevents WebView zoom).
    term.attachCustomKeyEventHandler((e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
      // Copy: Ctrl+Shift+C or Ctrl+Insert
      if ((mod && e.shiftKey && key === 'c') || (mod && !e.shiftKey && e.key === 'Insert')) {
        void copySelection(term);
        return false;
      }
      // Paste: Ctrl+Shift+V or Shift+Insert
      if (
        (mod && e.shiftKey && key === 'v') ||
        (!mod && e.shiftKey && e.key === 'Insert' && !e.altKey)
      ) {
        void pasteClipboard(term);
        return false;
      }
      // Zoom in: Ctrl + '+' / '='
      if (mod && !e.shiftKey && !e.altKey && (e.key === '+' || e.key === '=')) {
        onZoomRef.current(1);
        return false;
      }
      // Zoom in (Shift layout, e.g. Ctrl+Shift+'=' producing '+')
      if (mod && e.shiftKey && !e.altKey && (e.key === '+' || e.key === '=')) {
        // Avoid hijacking Ctrl+Shift+C/V handled above (key checked first).
        onZoomRef.current(1);
        return false;
      }
      // Zoom out: Ctrl + '-' / '_'
      if (mod && !e.altKey && (e.key === '-' || e.key === '_')) {
        onZoomRef.current(-1);
        return false;
      }
      // Zoom reset: Ctrl + '0'
      if (mod && !e.shiftKey && !e.altKey && e.key === '0') {
        onZoomResetRef.current();
        return false;
      }
      // Find: Ctrl+Shift+F (plain Ctrl+F stays with the shell)
      if (mod && e.shiftKey && !e.altKey && key === 'f') {
        setSearchOpen(true);
        return false;
      }
      return true;
    });

    // Right-click: copy selection if any, else paste (Windows Terminal style).
    const onContextMenu = (e: MouseEvent) => {
      e.preventDefault();
      if (term.hasSelection()) {
        void copySelection(term);
      } else {
        void pasteClipboard(term);
      }
    };
    container.addEventListener('contextmenu', onContextMenu);

    // Ctrl+wheel zoom.
    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey) return;
      e.preventDefault();
      onZoomRef.current(e.deltaY < 0 ? 1 : -1);
    };
    container.addEventListener('wheel', onWheel, { passive: false });

    // External focus requests (window refocus, titlebar actions).
    const onFocusRequest = () => {
      if (!activeRef.current) return;
      try {
        term.focus();
      } catch {}
    };
    window.addEventListener('atlas:focus-terminal', onFocusRequest);

    // Initialize Tauri or mock mode
    if (isTauri()) {
      void spawnSession();
    } else {
      sessionDisposables.push(initMock(term));
    }

    return () => {
      cancelled = true;
      restartRef.current = null;
      resizeObserver.disconnect();
      container.removeEventListener('contextmenu', onContextMenu);
      container.removeEventListener('wheel', onWheel);
      window.removeEventListener('atlas:focus-terminal', onFocusRequest);
      teardownSession();
      while (persistDisposables.length > 0) {
        const d = persistDisposables.pop();
        try {
          d?.dispose();
        } catch {}
      }
      termRef.current = null;
      fitRef.current = null;
      searchRef.current = null;
      applyGpuRef.current = null;
      gpuAddonRef.current = null;
      term.dispose();
    };
    // Mount-once: session/restart managed via refs. active/font handled below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Live-apply typography + cursor to the open terminal.
  useEffect(() => {
    const term = termRef.current;
    if (!term) return;
    try {
      term.options.fontFamily = fontFamily;
    } catch {}
    try {
      term.options.fontSize = fontSize;
    } catch {}
    try {
      term.options.lineHeight = lineHeight;
    } catch {}
    try {
      term.options.cursorStyle = cursorStyle;
    } catch {}
    try {
      term.options.cursorBlink = cursorBlink;
    } catch {}
    if (!activeRef.current) return;
    const fitAddon = fitRef.current;
    if (!fitAddon) return;
    const raf = requestAnimationFrame(() => {
      try {
        fitAddon.fit();
      } catch {}
    });
    return () => cancelAnimationFrame(raf);
  }, [fontFamily, fontSize, lineHeight, cursorStyle, cursorBlink]);

  // GPU renderer live toggle (Settings → Appearance).
  useEffect(() => {
    applyGpuRef.current?.(gpu);
  }, [gpu]);

  // Font metrics changed: invalidate the GPU glyph texture atlas so no
  // stale glyphs survive (the classic WebGL "ghost glyph" artifact).
  useEffect(() => {
    try {
      gpuAddonRef.current?.clearTextureAtlas();
    } catch {}
  }, [fontFamily, fontSize]);

  // Refit + focus when this tab becomes visible. Mounted-hidden tabs have
  // 0-size containers, so the real sizing happens here. fit() triggers
  // onResize which notifies the backend (resize_pty).
  useEffect(() => {
    if (!active) return;
    const term = termRef.current;
    const fitAddon = fitRef.current;
    if (!term || !fitAddon) return;
    const raf = requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        try {
          fitAddon.fit();
        } catch {}
        try {
          term.focus();
        } catch {}
      });
    });
    return () => cancelAnimationFrame(raf);
  }, [active]);

  // Move focus to the exit overlay so R/Enter work instead of typing into
  // the dead PTY.
  useEffect(() => {
    if (exited) {
      restartBtnRef.current?.focus();
    }
  }, [exited]);

  // Focus the search input when the bar opens.
  useEffect(() => {
    if (searchOpen) {
      searchInputRef.current?.focus();
      searchInputRef.current?.select();
    }
  }, [searchOpen ]);

  const runSearch = (query: string, caseSensitive: boolean, dir: 1 | -1) => {
    const addon = searchRef.current;
    if (!addon || !query) {
      setSearchNoMatch(false);
      return;
    }
    const opts = { caseSensitive, regex: false, wholeWord: false, incremental: true };
    const found = dir > 0 ? addon.findNext(query, opts) : addon.findPrevious(query, opts);
    setSearchNoMatch(!found);
  };

  const closeSearch = () => {
    try {
      searchRef.current?.clearDecorations();
    } catch {}
    setSearchOpen(false);
    setSearchQuery('');
    setSearchNoMatch(false);
    try {
      termRef.current?.focus();
    } catch {}
  };

  const handleRestart = () => {
    restartRef.current?.();
  };

  const handleOverlayKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'r' || e.key === 'R' || e.key === 'Enter') {
      e.preventDefault();
      handleRestart();
    }
  };

  return (
    <div className="terminal-pane">
      <div ref={containerRef} className="terminal-xterm" />
      {searchOpen && (
        <div className="search-bar">
          <input
            ref={searchInputRef}
            className={searchNoMatch ? 'search-input search-nomatch' : 'search-input'}
            value={searchQuery}
            placeholder="Find"
            spellCheck={false}
            onChange={(e) => {
              const q = e.target.value;
              setSearchQuery(q);
              runSearch(q, searchCase, 1);
            }}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                runSearch(searchQuery, searchCase, e.shiftKey ? -1 : 1);
              } else if (e.key === 'Escape') {
                e.preventDefault();
                closeSearch();
              }
            }}
          />
          <button
            className={searchCase ? 'search-btn search-btn-on' : 'search-btn'}
            title="Match case"
            onClick={() => {
              const next = !searchCase;
              setSearchCase(next);
              runSearch(searchQuery, next, 1);
            }}
          >
            Aa
          </button>
          <button
            className="search-btn"
            title="Previous (Shift+Enter)"
            onClick={() => runSearch(searchQuery, searchCase, -1)}
          >
            <ChevronUp size={14} strokeWidth={2} />
          </button>
          <button
            className="search-btn"
            title="Next (Enter)"
            onClick={() => runSearch(searchQuery, searchCase, 1)}
          >
            <ChevronDown size={14} strokeWidth={2} />
          </button>
          <button className="search-btn" title="Close (Esc)" onClick={closeSearch}>
            <X size={14} strokeWidth={2} />
          </button>
        </div>
      )}
      {exited && (
        <div className="exit-overlay" onKeyDown={handleOverlayKeyDown}>
          <div className="exit-card">
            <div className="exit-title">Process exited</div>
            <div className="exit-actions">
              <button
                ref={restartBtnRef}
                className="exit-btn exit-btn-primary"
                onClick={handleRestart}
              >
                Restart (R)
              </button>
              <button className="exit-btn" onClick={onRequestClose}>
                Close tab
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
