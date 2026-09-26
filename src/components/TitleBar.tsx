import { useCallback } from 'react';
import { Minus, Square, X } from 'lucide-react';

let tauriWindow: any = null;
try {
  // Dynamic import handled at module level for Tauri context
  import('@tauri-apps/api/window').then((mod) => {
    tauriWindow = mod;
  }).catch(() => {});
} catch {}

export function TitleBar() {

  const handleMinimize = useCallback(async () => {
    try {
      const win = tauriWindow?.getCurrentWebviewWindow?.() ?? tauriWindow?.appWindow;
      await win?.minimize();
    } catch {}
  }, []);

  const handleMaximize = useCallback(async () => {
    try {
      const win = tauriWindow?.getCurrentWebviewWindow?.() ?? tauriWindow?.appWindow;
      await win?.toggleMaximize();
    } catch {}
  }, []);

  const handleClose = useCallback(async () => {
    try {
      const win = tauriWindow?.getCurrentWebviewWindow?.() ?? tauriWindow?.appWindow;
      await win?.close();
    } catch {}
  }, []);

  return (
    <div className="titlebar" data-tauri-drag-region>
      {/* Left: Shell title */}
      <div className="flex items-center gap-2" data-tauri-drag-region>
        <span className="titlebar-title" data-tauri-drag-region>
          Atlas Terminal
        </span>
      </div>

      {/* Center spacer - draggable */}
      <div className="flex-1" data-tauri-drag-region />

      {/* Right: Window controls */}
      <div className="flex items-center gap-0.5">
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
