import { useEffect, useState } from 'react';
import { X } from 'lucide-react';
import {
  DEFAULT_SETTINGS,
  FONT_CHOICES,
  loadSettings,
  saveSettings,
  type AppSettings,
  type CursorStyle,
  type ShellKind,
} from '../settings';

type Section = 'general' | 'appearance' | 'typography';

async function hideWindow() {
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().hide();
  } catch {}
}

function Stepper({
  value,
  min,
  max,
  step,
  format,
  onChange,
}: {
  value: number;
  min: number;
  max: number;
  step: number;
  format: (n: number) => string;
  onChange: (n: number) => void;
}) {
  return (
    <div className="sw-stepper">
      <button
        className="sw-step-btn"
        aria-label="Decrease"
        onClick={() => onChange(Math.max(min, Math.round((value - step) * 100) / 100))}
      >
        −
      </button>
      <span className="sw-step-value">{format(value)}</span>
      <button
        className="sw-step-btn"
        aria-label="Increase"
        onClick={() => onChange(Math.min(max, Math.round((value + step) * 100) / 100))}
      >
        +
      </button>
    </div>
  );
}

export function SettingsWindow() {
  const [settings, setSettings] = useState<AppSettings>(() => loadSettings());
  const [section, setSection] = useState<Section>('general');

  useEffect(() => {
    saveSettings(settings);
  }, [settings]);

  const set = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
    setSettings((prev) => ({ ...prev, [key]: value }));
  };

  return (
    <div className="sw-root">
      <div className="sw-titlebar" data-tauri-drag-region>
        <span className="sw-title" data-tauri-drag-region>
          Settings
        </span>
        <button className="window-control" onClick={hideWindow} aria-label="Close settings">
          <X size={14} strokeWidth={1.5} />
        </button>
      </div>

      <div className="sw-segments">
        {(['general', 'appearance', 'typography'] as Section[]).map((s) => (
          <button
            key={s}
            className={section === s ? 'sw-segment sw-segment-on' : 'sw-segment'}
            onClick={() => setSection(s)}
          >
            {s === 'general' ? 'General' : s === 'appearance' ? 'Appearance' : 'Typography'}
          </button>
        ))}
      </div>

      <div className="sw-content">
        {section === 'general' && (
          <>
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
            <div className="settings-row">
              <span className="settings-label">Scrollback lines (new tabs)</span>
              <Stepper
                value={settings.scrollback}
                min={1000}
                max={100000}
                step={1000}
                format={(n) => `${n.toLocaleString()}`}
                onChange={(n) => set('scrollback', n)}
              />
            </div>
          </>
        )}

        {section === 'appearance' && (
          <>
            <label className="settings-row">
              <span className="settings-label">Theme</span>
              <select className="settings-input" value="dark" disabled title="More themes soon">
                <option value="dark">Cupertino Dark</option>
              </select>
            </label>
            <div className="settings-row">
              <span className="settings-label">
                Opacity <span className="settings-value">{Math.round(settings.opacity * 100)}%</span>
              </span>
              <input
                type="range"
                className="sw-range"
                min={30}
                max={100}
                step={1}
                value={Math.round(settings.opacity * 100)}
                style={{ ['--fill' as string]: `${Math.round(settings.opacity * 100)}%` }}
                onChange={(e) => set('opacity', Number(e.target.value) / 100)}
              />
            </div>
            <div className="settings-row">
              <span className="settings-label">
                Blur <span className="settings-value">{settings.blur}px</span>
              </span>
              <input
                type="range"
                className="sw-range"
                min={0}
                max={40}
                step={1}
                value={settings.blur}
                style={{ ['--fill' as string]: `${(settings.blur / 40) * 100}%` }}
                onChange={(e) => set('blur', Number(e.target.value))}
              />
            </div>
            <label className="settings-row">
              <span className="settings-label">Cursor</span>
              <select
                className="settings-input"
                value={settings.cursorStyle}
                onChange={(e) => set('cursorStyle', e.target.value as CursorStyle)}
              >
                <option value="block">Block</option>
                <option value="bar">Bar</option>
                <option value="underline">Underline</option>
              </select>
            </label>
            <label className="sw-check-row">
              <input
                type="checkbox"
                className="sw-check"
                checked={settings.cursorBlink}
                onChange={(e) => set('cursorBlink', e.target.checked)}
              />
              <span className="settings-label">Blinking cursor</span>
            </label>
            <label className="sw-check-row">
              <input
                type="checkbox"
                className="sw-check"
                checked={settings.gpu}
                onChange={(e) => set('gpu', e.target.checked)}
              />
              <span className="settings-label">GPU acceleration (WebGL)</span>
            </label>
            <div className="settings-row">
              <span className="settings-hint">
                Hardware glyph rendering — smoother at high scroll speed. Falls back
                to the software renderer automatically.
              </span>
            </div>
          </>
        )}

        {section === 'typography' && (
          <>
            <label className="settings-row">
              <span className="settings-label">Font family</span>
              <select
                className="settings-input"
                value={settings.fontFamily}
                onChange={(e) => set('fontFamily', e.target.value)}
              >
                {FONT_CHOICES.map((f) => (
                  <option key={f} value={f}>
                    {f}
                  </option>
                ))}
              </select>
            </label>
            <div className="settings-row">
              <span className="settings-label">Font size</span>
              <Stepper
                value={settings.fontSize}
                min={13}
                max={16}
                step={0.5}
                format={(n) => `${n}px`}
                onChange={(n) => set('fontSize', n)}
              />
            </div>
            <div className="settings-row">
              <span className="settings-label">Line height</span>
              <Stepper
                value={settings.lineHeight}
                min={1.0}
                max={1.6}
                step={0.05}
                format={(n) => n.toFixed(2)}
                onChange={(n) => set('lineHeight', n)}
              />
            </div>
          </>
        )}
      </div>

      <div className="sw-footer">
        <span className="settings-hint">Shell, directory and scrollback apply to new tabs.</span>
        <button
          className="exit-btn sw-reset"
          onClick={() => setSettings({ ...DEFAULT_SETTINGS })}
        >
          Reset Defaults
        </button>
      </div>
    </div>
  );
}
