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
      {/* Left: tab strip, flush with the left edge. The strip background
          drags the window; clicks still land on tabs/buttons (no-move = click). */}
      {center ? (
        <div className="titlebar-left" data-tauri-drag-region>
          {center}
        </div>
      ) : (
        <div className="titlebar-spacer" data-tauri-drag-region />
      )}

      {/* Right: settings + window controls */}
      <div className="titlebar-controls">
        {onSettings && (
          <button
            className={settingsOpen ? 'window-control window-control-on' : 'window-control'}
            onClick={onSettings}
            aria-label="Settings"
            title="Settings (Ctrl+,)"
          >
            <Settings size={13} strokeWidth={1.5} />
          </button>
        )}
        <button
          className="window-control"
          onClick={handleMinimize}
          aria-label="Minimize"
        >
          <Minus size={12} strokeWidth={1.5} />
        </button>
        <button
          className="window-control"
          onClick={handleMaximize}
          aria-label="Maximize"
        >
          <Square size={10} strokeWidth={1.5} />
        </button>
        <button
          className="window-control window-control-close"
          onClick={handleClose}
          aria-label="Close"
        >
          <X size={13} strokeWidth={1.5} />
        </button>
      </div>
    </div>
  );
}
