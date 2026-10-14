export type ShellKind = 'powershell' | 'pwsh';

export interface AppSettings {
  /** Backend shell for new tabs. Falls back to powershell.exe if missing. */
  shell: ShellKind;
  /** Startup directory for new tabs. Empty = backend default. */
  cwd: string;
  /** Applied live to all open terminals. */
  fontFamily: string;
  /** Window background alpha 0.3–1. Applied live via CSS var. */
  opacity: number;
  /** Backdrop blur px 0–40. Applied live via CSS var. */
  blur: number;
  /** Scrollback lines for new tabs. */
  scrollback: number;
}

const LEGACY_FONT_STACKS = new Set([
  // Phase 5 default (system-first, predates icon-glyph requirement)
  "'Cascadia Mono', 'Cascadia Code', Consolas, monospace",
  // Original default
  '"JetBrains Mono Nerd Font", "MesloLGS NF", "SF Mono", Consolas, monospace',
]);

export const DEFAULT_SETTINGS: AppSettings = {
  shell: 'powershell',
  cwd: '',
  // Nerd Fonts first so icon/powerline glyphs resolve when installed;
  // canvas renderer measures the first *available* family, so missing
  // fonts fall through to Consolas with exact metrics (no drift).
  // Install a Nerd Font (e.g. CaskaydiaCove NF) to end tofu boxes.
  fontFamily:
    "'JetBrainsMono Nerd Font', 'CaskaydiaCove Nerd Font', 'Cascadia Code NF', 'FiraCode Nerd Font', Consolas, monospace",
  opacity: 0.6,
  blur: 30,
  scrollback: 10000,
};

const STORAGE_KEY = 'atlas.settings';

const clamp = (n: number, min: number, max: number) =>
  Math.min(max, Math.max(min, n));

export function loadSettings(): AppSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_SETTINGS };
    const p = JSON.parse(raw) as Partial<AppSettings>;
    let fontFamily =
      typeof p.fontFamily === 'string' && p.fontFamily.trim()
        ? p.fontFamily.slice(0, 512)
        : DEFAULT_SETTINGS.fontFamily;
    // Migrate installs still on a previous default stack.
    if (LEGACY_FONT_STACKS.has(fontFamily)) {
      fontFamily = DEFAULT_SETTINGS.fontFamily;
    }
    return {
      shell: p.shell === 'pwsh' ? 'pwsh' : 'powershell',
      cwd: typeof p.cwd === 'string' ? p.cwd.slice(0, 512) : '',
      fontFamily,
      opacity:
        typeof p.opacity === 'number' && Number.isFinite(p.opacity)
          ? clamp(p.opacity, 0.3, 1)
          : DEFAULT_SETTINGS.opacity,
      blur:
        typeof p.blur === 'number' && Number.isFinite(p.blur)
          ? clamp(Math.round(p.blur), 0, 40)
          : DEFAULT_SETTINGS.blur,
      scrollback:
        typeof p.scrollback === 'number' && Number.isFinite(p.scrollback)
          ? clamp(Math.round(p.scrollback), 1000, 100000)
          : DEFAULT_SETTINGS.scrollback,
    };
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

export function saveSettings(s: AppSettings) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(s));
  } catch {}
}

/** Live-applies glass vars consumed by .terminal-window in globals.css. */
export function applyGlassVars(opacity: number, blur: number) {
  try {
    const root = document.documentElement;
    root.style.setProperty('--atlas-bg-alpha', String(clamp(opacity, 0.3, 1)));
    root.style.setProperty('--atlas-blur', `${clamp(Math.round(blur), 0, 40)}px`);
  } catch {}
}
