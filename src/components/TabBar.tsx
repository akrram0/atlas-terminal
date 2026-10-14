import { Plus, X } from 'lucide-react';

export interface TabInfo {
  id: string;
  title: string;
}

interface TabBarProps {
  tabs: TabInfo[];
  activeId: string;
  onSelect: (id: string) => void;
  onClose: (id: string) => void;
  onNew: () => void;
}

export function TabBar({ tabs, activeId, onSelect, onClose, onNew }: TabBarProps) {
  return (
    <div className="tabbar" data-tauri-drag-region={false}>
      {tabs.map((tab) => {
        const isActive = tab.id === activeId;
        return (
          <div
            key={tab.id}
            className={isActive ? 'tab tab-active' : 'tab'}
            onClick={() => onSelect(tab.id)}
            onAuxClick={(e) => {
              // Middle-click closes tab (Kitty-like)
              if (e.button === 1) {
                e.preventDefault();
                onClose(tab.id);
              }
            }}
            title={tab.title}
          >
            <span className="tab-title">{tab.title}</span>
            <button
              className="tab-close"
              aria-label={`Close ${tab.title}`}
              onClick={(e) => {
                e.stopPropagation();
                onClose(tab.id);
              }}
            >
              <X size={12} strokeWidth={2} />
            </button>
          </div>
        );
      })}
      <button className="tab-new" onClick={onNew} aria-label="New tab" title="New tab (Ctrl+Shift+T)">
        <Plus size={14} strokeWidth={1.5} />
      </button>
    </div>
  );
}
