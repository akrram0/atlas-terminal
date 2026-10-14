import type { AppSettings, ShellKind } from '../settings';

interface SettingsPanelProps {
  settings: AppSettings;
  onChange: (next: AppSettings) => void;
  onClose: () => void;
  onReset: () => void;
}

export function SettingsPanel({ settings, onChange, onClose, onReset }: SettingsPanelProps) {
  const set = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
    onChange({ ...settings, [key]: value });
  };

  return (
    <div className="settings-scrim" onClick={onClose}>
      <div className="settings-card" onClick={(e) => e.stopPropagation()}>
        <div className="settings-header">
          <span className="settings-title">Settings</span>
          <button className="search-btn" onClick={onClose} aria-label="Close settings">
            ✕
          </button>
        </div>

        <label className="settings-row">
          <span className="settings-label">Shell (new tabs)</span>
          <select
            className="settings-input"
            value={settings.shell}
            onChange={(e) => set('shell', e.target.value as ShellKind)}
          >
            <option value="powershell">Windows PowerShell</option>
            <option value="pwsh">PowerShell 7 (pwsh)</option>
          </select>
        </label>

        <label className="settings-row">
          <span className="settings-label">Startup directory (new tabs)</span>
          <input
            className="settings-input"
            value={settings.cwd}
            placeholder="Default"
            spellCheck={false}
            onChange={(e) => set('cwd', e.target.value)}
          />
        </label>

        <label className="settings-row">
          <span className="settings-label">Font family</span>
          <input
            className="settings-input"
            value={settings.fontFamily}
            spellCheck={false}
            onChange={(e) => set('fontFamily', e.target.value)}
          />
        </label>

        <div className="settings-row">
          <span className="settings-label">
            Opacity <span className="settings-value">{Math.round(settings.opacity * 100)}%</span>
          </span>
          <input
            type="range"
            min={30}
            max={100}
            step={1}
            value={Math.round(settings.opacity * 100)}
            onChange={(e) => set('opacity', Number(e.target.value) / 100)}
          />
        </div>

        <div className="settings-row">
          <span className="settings-label">
            Blur <span className="settings-value">{settings.blur}px</span>
          </span>
          <input
            type="range"
            min={0}
            max={40}
            step={1}
            value={settings.blur}
            onChange={(e) => set('blur', Number(e.target.value))}
          />
        </div>

        <label className="settings-row">
          <span className="settings-label">Scrollback lines (new tabs)</span>
          <input
            className="settings-input"
            type="number"
            min={1000}
            max={100000}
            step={1000}
            value={settings.scrollback}
            onChange={(e) => {
              const n = Math.round(Number(e.target.value));
              if (Number.isFinite(n)) set('scrollback', n);
            }}
          />
        </label>

        <div className="settings-footer">
          <span className="settings-hint">Shell, directory and scrollback apply to new tabs.</span>
          <button className="exit-btn" onClick={onReset}>
            Reset defaults
          </button>
        </div>
      </div>
    </div>
  );
}
