import { useCallback, type ReactNode } from 'react';
import { Minus, Settings, Square, X } from 'lucide-react';

async function getTauriWindow() {
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    return getCurrentWindow();
  } catch {
    try {
      const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow');
      return getCurrentWebviewWindow();
    } catch {
      return null;
    }
  }
}

export function TitleBar({
  center,
  onSettings,
  settingsOpen,
}: {
  center?: ReactNode;
  onSettings?: () => void;
  settingsOpen?: boolean;
}) {
  const handleMinimize = useCallback(async () => {
    try {
      const win = await getTauriWindow();
      await win?.minimize();
    } catch {}
  }, []);

  const handleMaximize = useCallback(async () => {
    try {
      const win = await getTauriWindow();
      await win?.toggleMaximize();
    } catch {}
  }, []);

  const handleClose = useCallback(async () => {
    try {
      const win = await getTauriWindow();
      await win?.close();
    } catch {}
  }, []);

  return (
    <div className="titlebar" data-tauri-drag-region>
      {/* Left: Shell title */}
      <div className="flex items-center gap-2 shrink-0" data-tauri-drag-region>
        <span className="titlebar-title" data-tauri-drag-region>
          Atlas Terminal
        </span>
      </div>

      {/* Center: tab strip (or draggable spacer when no tabs) */}
      {center ? (
        <div className="flex-1 flex items-center justify-center min-w-0 px-2">{center}</div>
      ) : (
        <div className="flex-1" data-tauri-drag-region />
      )}

      {/* Right: Window controls */}
      <div className="flex items-center gap-0.5 shrink-0">
        {onSettings && (
          <button
            className={settingsOpen ? 'window-control window-control-on' : 'window-control'}
            onClick={onSettings}
            aria-label="Settings"
            title="Settings"
          >
            <Settings size={14} strokeWidth={1.5} />
          </button>
        )}
        <button
          className="window-control"
          onClick={handleMinimize}
          aria-label="Minimize"
        >
          <Minus size={14} strokeWidth={1.5} />
        </button>
        <button
          className="window-control"
          onClick={handleMaximize}
          aria-label="Maximize"
        >
          <Square size={11} strokeWidth={1.5} />
        </button>
        <button
          className="window-control window-control-close"
          onClick={handleClose}
          aria-label="Close"
        >
          <X size={14} strokeWidth={1.5} />
        </button>
      </div>
    </div>
  );
}
