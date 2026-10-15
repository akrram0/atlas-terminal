export type ShellKind = 'powershell' | 'pwsh';
export type CursorStyle = 'block' | 'bar' | 'underline';

export interface AppSettings {
  /** Backend shell for new tabs. Falls back to powershell.exe if missing. */
  shell: ShellKind;
  /** Startup directory for new tabs. Empty = backend default. */
  cwd: string;
  /** Primary font family (dropdown). Fallback stack is built in code. */
  fontFamily: string;
  /** Default font size for terminals (zoom adjusts this globally). */
  fontSize: number;
  /** Line height multiplier. Applied live. */
  lineHeight: number;
  /** Cursor shape. Applied live. */
  cursorStyle: CursorStyle;
  /** Cursor blink. Applied live. */
  cursorBlink: boolean;
  /** GPU (WebGL2) renderer. Auto-falls back to the DOM renderer. Applied live. */
  gpu: boolean;
  /** Window background alpha 0.3–1. Applied live via CSS var. */
  opacity: number;
  /** Backdrop blur px 0–40. Applied live via CSS var. */
  blur: number;
  /** Scrollback lines for new tabs. */
  scrollback: number;
}

export const FONT_CHOICES = [
  'JetBrainsMono Nerd Font',
  'CaskaydiaCove Nerd Font',
  'Cascadia Code NF',
  'FiraCode Nerd Font',
  'Consolas',
] as const;

/** Full xterm stack built internally: choice first, guaranteed monospace last. */
export function fontStack(primary: string): string {
  const name = (primary || '').trim() || 'Consolas';
  if (name.includes(',')) return name.slice(0, 512); // already a full stack
  return `'${name}', Consolas, monospace`;
}

export function shellDisplayName(shell: ShellKind): string {
  return shell === 'pwsh' ? 'PowerShell 7' : 'PowerShell';
}

/** `C:\WINDOWS\System32` → `System32 — PowerShell`; `~` stays `~`. */
export function formatTabTitle(raw: string, shell: ShellKind): string {
  const title = (raw || '').trim();
  if (!title) return shellDisplayName(shell);
  if (title === '~' || title === '~/' || title === '~\\') return '~';
  const stripped = title.replace(/[\\/]+$/, '');
  const idx = Math.max(stripped.lastIndexOf('\\'), stripped.lastIndexOf('/'));
  const base = idx >= 0 ? stripped.slice(idx + 1) : stripped;
  if (!base) return stripped || shellDisplayName(shell);
  return `${base} — ${shellDisplayName(shell)}`;
}

const LEGACY_FONT_STACKS = new Set([
  "'JetBrainsMono Nerd Font', 'CaskaydiaCove Nerd Font', 'Cascadia Code NF', 'FiraCode Nerd Font', Consolas, monospace",
  "'Cascadia Mono', 'Cascadia Code', Consolas, monospace",
  '"JetBrains Mono Nerd Font", "MesloLGS NF", "SF Mono", Consolas, monospace',
]);

/** Map a stored full stack (or name) back to a dropdown primary. */
function primaryFromStored(value: string): string {
  const m = value.match(/['"]([^'"]+)['"]/);
  const first = (m ? m[1] : value).trim();
  const found = (FONT_CHOICES as readonly string[]).find(
    (c) => c.toLowerCase() === first.toLowerCase()
  );
  if (found) return found;
  // Close variants of the same families
  if (/jetbrains/i.test(first)) return 'JetBrainsMono Nerd Font';
  if (/caskaydia|casca/i.test(first)) return 'CaskaydiaCove Nerd Font';
  if (/fira/i.test(first)) return 'FiraCode Nerd Font';
  return 'Consolas';
}

export const DEFAULT_SETTINGS: AppSettings = {
  shell: 'powershell',
  cwd: '',
  fontFamily: 'CaskaydiaCove Nerd Font',
  fontSize: 13.5,
  lineHeight: 1.25,
  cursorStyle: 'block',
  cursorBlink: true,
  gpu: true,
  opacity: 1,
  blur: 0,
  scrollback: 10000,
};

const STORAGE_KEY = 'atlas.settings';
export const SETTINGS_STORAGE_KEY = STORAGE_KEY;

/** Bump when defaults change in a way stored settings must adopt.
 *  v2: flat chrome (opaque, no blur) became the default look. */
const SCHEMA_VERSION = 2;

const clamp = (n: number, min: number, max: number) =>
  Math.min(max, Math.max(min, n));

const num = (v: unknown, fallback: number, min: number, max: number): number => {
  if (typeof v !== 'number' || !Number.isFinite(v)) return fallback;
  return clamp(v, min, max);
};

export function loadSettings(): AppSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_SETTINGS };
    const p = JSON.parse(raw) as Partial<AppSettings> & { schema?: number };
    // Pre-v2 settings carried the old glass defaults; adopt the flat look.
    const legacyGlass = typeof p.schema !== 'number' || p.schema < SCHEMA_VERSION;
    let fontFamily =
      typeof p.fontFamily === 'string' && p.fontFamily.trim()
        ? p.fontFamily.slice(0, 512)
        : DEFAULT_SETTINGS.fontFamily;
    // Migrate stored full stacks (or legacy names) to dropdown primaries.
    if (fontFamily.includes(',') || !((FONT_CHOICES as readonly string[]) as string[]).includes(fontFamily)) {
      fontFamily = LEGACY_FONT_STACKS.has(fontFamily)
        ? DEFAULT_SETTINGS.fontFamily
        : primaryFromStored(fontFamily);
    }
    const cursorStyle: CursorStyle =
      p.cursorStyle === 'bar' || p.cursorStyle === 'underline' ? p.cursorStyle : 'block';
    return {
      shell: p.shell === 'pwsh' ? 'pwsh' : 'powershell',
      cwd: typeof p.cwd === 'string' ? p.cwd.slice(0, 512) : '',
      fontFamily,
      fontSize: num(p.fontSize, DEFAULT_SETTINGS.fontSize, 8, 32),
      lineHeight: num(p.lineHeight, DEFAULT_SETTINGS.lineHeight, 1.0, 2.0),
      cursorStyle,
      cursorBlink: typeof p.cursorBlink === 'boolean' ? p.cursorBlink : true,
      gpu: typeof p.gpu === 'boolean' ? p.gpu : true,
      opacity: legacyGlass
        ? DEFAULT_SETTINGS.opacity
        : num(p.opacity, DEFAULT_SETTINGS.opacity, 0.3, 1),
      blur: legacyGlass ? DEFAULT_SETTINGS.blur : Math.round(num(p.blur, DEFAULT_SETTINGS.blur, 0, 40)),
      scrollback: Math.round(num(p.scrollback, DEFAULT_SETTINGS.scrollback, 1000, 100000)),
    };
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

export function saveSettings(s: AppSettings) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...s, schema: SCHEMA_VERSION }));
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
