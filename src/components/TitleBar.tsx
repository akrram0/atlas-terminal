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
      {/* Left: tab strip, 10px from the edge. The strip background drags
          the window; clicks still land on tabs/buttons (no-move = click). */}
      {center ? (
        <div
          className="flex-1 flex items-center justify-start min-w-0 pl-[10px] pr-2"
          data-tauri-drag-region
        >
          {center}
        </div>
      ) : (
        <div className="flex-1" data-tauri-drag-region />
      )}

      {/* Right: settings, divider, window controls */}
      <div className="flex items-center shrink-0 pr-1">
        {onSettings && (
          <>
            <button
              className={settingsOpen ? 'window-control window-control-on' : 'window-control'}
              onClick={onSettings}
              aria-label="Settings"
              title="Settings"
            >
              <Settings size={14} strokeWidth={1.5} />
            </button>
            <div className="titlebar-divider" />
          </>
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
