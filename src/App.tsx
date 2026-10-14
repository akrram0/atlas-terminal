import { useCallback, useEffect, useRef, useState } from 'react';
import { TitleBar } from './components/TitleBar';
import { TabBar, type TabInfo } from './components/TabBar';
import { Terminal } from './components/Terminal';
import { openSettingsWindow } from './openSettings';
import {
  applyGlassVars,
  fontStack,
  formatTabTitle,
  loadSettings,
  saveSettings,
  SETTINGS_STORAGE_KEY,
  type AppSettings,
} from './settings';

function newTabId() {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
    return crypto.randomUUID();
  }
  return `tab-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

/** Ask the active terminal to take keyboard focus (solid cursor). */
function focusActiveTerminal() {
  window.dispatchEvent(new Event('atlas:focus-terminal'));
}

function App() {
  const counterRef = useRef(1);
  const [tabs, setTabs] = useState<TabInfo[]>(() => [
    { id: newTabId(), title: 'Tab 1' },
  ]);
  const [activeId, setActiveId] = useState<string>(() => tabs[0].id);
  const [settings, setSettings] = useState<AppSettings>(() => loadSettings());
  const activeIdRef = useRef(activeId);
  activeIdRef.current = activeId;
  const tabsRef = useRef(tabs);
  tabsRef.current = tabs;
  const settingsRef = useRef(settings);
  settingsRef.current = settings;

  const updateSettings = useCallback((next: AppSettings) => {
    setSettings(next);
    saveSettings(next);
  }, []);

  const zoomBy = useCallback(
    (delta: number) => {
      const s = settingsRef.current;
      const size = Math.min(32, Math.max(8, Math.round((s.fontSize + delta) * 2) / 2));
      if (size !== s.fontSize) updateSettings({ ...s, fontSize: size });
    },
    [updateSettings]
  );

  const zoomReset = useCallback(() => {
    const s = settingsRef.current;
    if (s.fontSize !== 13.5) updateSettings({ ...s, fontSize: 13.5 });
  }, [updateSettings]);

  const addTab = useCallback(() => {
    counterRef.current += 1;
    const tab: TabInfo = {
      id: newTabId(),
      title: `Tab ${counterRef.current}`,
    };
    setTabs((prev) => [...prev, tab]);
    setActiveId(tab.id);
  }, []);

  const closeTab = useCallback((id: string) => {
    setTabs((prev) => {
      if (prev.length <= 1) {
        // Closing the last tab opens a fresh one (no orphan window).
        counterRef.current += 1;
        const fresh: TabInfo = {
          id: newTabId(),
          title: `Tab ${counterRef.current}`,
        };
        setActiveId(fresh.id);
        return [fresh];
      }
      const idx = prev.findIndex((t) => t.id === id);
      const next = prev.filter((t) => t.id !== id);
      if (id === activeIdRef.current && idx !== -1) {
        // Focus neighbor: right, else left.
        const neighbor = next[Math.min(idx, next.length - 1)];
        setActiveId(neighbor.id);
      }
      return next;
    });
  }, []);

  const selectTab = useCallback((id: string) => {
    setActiveId(id);
  }, []);

  const cycleTab = useCallback((dir: 1 | -1) => {
    const list = tabsRef.current;
    if (list.length <= 1) return;
    const idx = list.findIndex((t) => t.id === activeIdRef.current);
    const next = list[(idx + dir + list.length) % list.length];
    setActiveId(next.id);
  }, []);

  const handleTabTitle = useCallback((id: string, raw: string) => {
    const short = formatTabTitle(raw, settingsRef.current.shell);
    setTabs((prev) => {
      const tab = prev.find((t) => t.id === id);
      if (!tab || (tab.title === short && tab.fullTitle === raw)) return prev;
      return prev.map((t) => (t.id === id ? { ...t, title: short, fullTitle: raw } : t));
    });
  }, []);

  // Live-apply glass vars on boot and every change.
  useEffect(() => {
    applyGlassVars(settings.opacity, settings.blur);
  }, [settings.opacity, settings.blur]);

  // Sync settings saved by the native Settings window (same origin).
  useEffect(() => {
    const onStorage = (e: StorageEvent) => {
      if (e.key === SETTINGS_STORAGE_KEY) {
        setSettings(loadSettings());
      }
    };
    window.addEventListener('storage', onStorage);
    return () => window.removeEventListener('storage', onStorage);
  }, []);

  // Refocus the terminal whenever this window regains focus (alt-tab,
  // Settings close, titlebar buttons) so the cursor stays solid.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        unlisten = await getCurrentWindow().onFocusChanged(({ payload }) => {
          if (payload) focusActiveTerminal();
        });
      } catch {}
    })();
    return () => unlisten?.();
  }, []);

  // Shortcuts (window-level so they work regardless of xterm focus).
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      // Settings: Ctrl+,
      if (mod && !e.shiftKey && !e.altKey && e.key === ',') {
        e.preventDefault();
        void openSettingsWindow();
        return;
      }
      // New tab: Ctrl+Shift+T
      if (mod && e.shiftKey && (e.key === 'T' || e.key === 't')) {
        e.preventDefault();
        addTab();
        return;
      }
      // Close tab: Ctrl+Shift+W
      if (mod && e.shiftKey && (e.key === 'W' || e.key === 'w')) {
        e.preventDefault();
        closeTab(activeIdRef.current);
        return;
      }
      // Cycle: Ctrl+Tab next, Ctrl+Shift+Tab prev
      if (e.ctrlKey && !e.metaKey && e.key === 'Tab') {
        e.preventDefault();
        cycleTab(e.shiftKey ? -1 : 1);
        return;
      }
      // Jump: Alt+1..9
      if (e.altKey && !e.ctrlKey && !e.metaKey && /^[1-9]$/.test(e.key)) {
        const n = parseInt(e.key, 10) - 1;
        const list = tabsRef.current;
        if (n < list.length) {
          e.preventDefault();
          setActiveId(list[n].id);
        }
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [addTab, closeTab, cycleTab]);

  return (
    <div className="terminal-window">
      <TitleBar
        onSettings={() => void openSettingsWindow()}
        center={
          <TabBar
            tabs={tabs}
            activeId={activeId}
            onSelect={selectTab}
            onClose={closeTab}
            onNew={addTab}
          />
        }
      />
      <div className="terminal-container terminal-stack">
        {tabs.map((tab) => {
          const isActive = tab.id === activeId;
          return (
            <div
              key={tab.id}
              style={{
                display: isActive ? 'flex' : 'none',
                flex: 1,
                minHeight: 0,
                minWidth: 0,
              }}
            >
              <Terminal
                active={isActive}
                fontFamily={fontStack(settings.fontFamily)}
                fontSize={settings.fontSize}
                lineHeight={settings.lineHeight}
                cursorStyle={settings.cursorStyle}
                cursorBlink={settings.cursorBlink}
                scrollback={settings.scrollback}
                shell={settings.shell}
                cwd={settings.cwd}
                onZoom={zoomBy}
                onZoomReset={zoomReset}
                onTitle={(title) => handleTabTitle(tab.id, title)}
                onRequestClose={() => closeTab(tab.id)}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}

export default App;
