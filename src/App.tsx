import { useCallback, useEffect, useRef, useState } from 'react';
import { TitleBar } from './components/TitleBar';
import { TabBar, type TabInfo } from './components/TabBar';
import { Terminal } from './components/Terminal';

function newTabId() {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
    return crypto.randomUUID();
  }
  return `tab-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function App() {
  const counterRef = useRef(1);
  const [tabs, setTabs] = useState<TabInfo[]>(() => [
    { id: newTabId(), title: 'Tab 1' },
  ]);
  const [activeId, setActiveId] = useState<string>(() => tabs[0].id);
  const activeIdRef = useRef(activeId);
  activeIdRef.current = activeId;
  const tabsRef = useRef(tabs);
  tabsRef.current = tabs;

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

  const handleTabTitle = useCallback((id: string, title: string) => {
    setTabs((prev) => {
      const tab = prev.find((t) => t.id === id);
      if (!tab || tab.title === title) return prev;
      return prev.map((t) => (t.id === id ? { ...t, title } : t));
    });
  }, []);

  // Kitty-like shortcuts (window-level so they work regardless of xterm focus).
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
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
