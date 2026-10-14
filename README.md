# Atlas Terminal

A Kitty-inspired terminal emulator for Windows with a modern Apple-style glass UI.
Built with **Tauri 2 + React + xterm.js**.

![Windows](https://img.shields.io/badge/Windows-0078D6?logo=windows&logoColor=white)
![Tauri 2](https://img.shields.io/badge/Tauri-2-FFC131?logo=tauri&logoColor=black)
![React 19](https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=black)

## What is this?

Atlas Terminal is a native Windows terminal emulator (not a web wrapper around an
existing console). It spawns real PowerShell processes in pseudoterminals (PTYs),
renders them with xterm.js on a GPU-composited canvas, and wraps everything in a
borderless acrylic glass window with tabs, search, zoom, and settings.

## Features

- **Real PTYs** — one `powershell.exe` / `pwsh.exe` per tab via ConPTY (`portable-pty`)
- **Tabs** — multi-session, hidden tabs stay alive with scrollback intact
- **Custom prompt** — status dot (exit code), `user@machine`, `~/path`, injected at spawn
- **Live tab titles** — shell reports cwd via OSC 0 on every prompt
- **Dead-tab overlay** — exited processes show Restart (R) / Close instead of a dead pane
- **Find bar** — `Ctrl+Shift+F`, incremental search, match-case, prev/next
- **Font zoom** — `Ctrl + / - / 0` and `Ctrl+wheel`, persisted as the default
- **Clipboard** — `Ctrl+Shift+C / V`, `Shift+Insert`, right-click copies selection else pastes
- **Settings panel** — shell, startup directory, font family, opacity, blur, scrollback
- **Glass UI** — borderless transparent window, acrylic/mica, custom titlebar

## Architecture

```
┌─────────────────────────────────────────────────┐
│  Frontend (React 19 + xterm.js, canvas renderer) │
│  App.tsx ── tabs state, shortcuts, settings      │
│  Terminal.tsx ── one xterm instance per tab       │
│  TabBar / TitleBar / SettingsPanel                │
└────────────── Tauri IPC (invoke + events) ───────┘
┌─────────────────────────────────────────────────┐
│  Backend (Rust, Tauri 2)                         │
│  lib.rs ── commands: spawn/write/resize/close     │
│  pty.rs ── PtyManager: HashMap<session, PTY>      │
│  prompt.rs ── PowerShell prompt + OSC title       │
└────────────────── portable-pty ─────────────────┘
         │                        │
   ConPTY master            powershell.exe child
```

### How it works

1. **Spawn** — `Terminal` mounts, fits xterm to its container, and calls
   `spawn_pty { cols, rows, shell, cwd }`. The backend opens a ConPTY pair,
   spawns the shell (`-NoLogo -NoExit`, optional startup dir), writes the
   custom prompt script into the PTY, and stores the session (writer + pair +
   child) in `PtyManager` under a UUID. A dedicated reader thread per session
   streams output back as `pty-data-{id}` events.
2. **Input** — xterm `onData` forwards keystrokes via `write_pty`. There is no
   local echo; everything displayed comes back through the PTY, so the screen
   always matches shell state.
3. **Resize** — `FitAddon` measures the container; xterm `onResize` notifies the
   backend (`resize_pty`) so the shell reflows. Activating a hidden tab refits
   (hidden containers report 0 size, so inactive tabs spawn at 80×24 and resize
   on show — no tiny PTYs).
4. **Titles** — the injected `prompt` function sets the console title and emits
   `OSC 0;path BEL` each render; xterm's `onTitleChange` flows up to the tab strip.
5. **Lifecycle** — unmounting a tab calls `close_pty` (kill + remove, idempotent).
   If the shell exits on its own (`exit`), the reader thread auto-removes the
   session so no `powershell.exe` leaks. A spawn guard guarantees one session
   per pane — double-attached listeners (duplicated bytes) are impossible by
   construction.
6. **Search / clipboard / zoom** are pure frontend: `addon-search` decorations,
   `navigator.clipboard`, and `term.options.fontSize` + refit.

### Backend commands (`src-tauri/src/lib.rs`)

| Command      | Args                            | Effect                                  |
| ------------ | ------------------------------- | --------------------------------------- |
| `spawn_pty`  | `cols, rows, shell?, cwd?`       | Open PTY, spawn shell, return session id|
| `write_pty`  | `sessionId, data`               | Write keystrokes to the PTY             |
| `resize_pty` | `sessionId, cols, rows`         | Resize the PTY                          |
| `close_pty`  | `sessionId`                     | Kill child, drop session (idempotent)   |

Events: `pty-data-{id} (string)`, `pty-exit-{id}`.

Shell allowlist: `powershell` → `powershell.exe`; `pwsh` → `pwsh.exe` with
automatic fallback to `powershell.exe` when missing. Startup dirs are validated
with `is_dir()` and silently ignored when invalid.

## Project structure

```
atlas-terminal/
├── src/
│   ├── App.tsx                  # tabs, global shortcuts, settings state
│   ├── settings.ts              # settings model + localStorage + glass vars
│   ├── main.tsx                 # React entry
│   ├── styles/globals.css        # glass theme, tabs, overlays, panels
│   └── components/
│       ├── Terminal.tsx         # xterm lifecycle, PTY session, find/clipboard/zoom
│       ├── TabBar.tsx           # tab strip
│       ├── TitleBar.tsx         # frameless bar, drag region, window controls
│       └── SettingsPanel.tsx    # settings UI
├── src-tauri/
│   ├── tauri.conf.json          # window (borderless/transparent), bundle
│   ├── capabilities/default.json# Tauri permissions
│   └── src/
│       ├── main.rs              # entry → lib::run()
│       ├── lib.rs               # Tauri commands + app setup
│       ├── pty.rs               # PtyManager: spawn/write/resize/close
│       └── prompt.rs            # PowerShell prompt + OSC title sync
├── .github/workflows/build.yml  # Windows CI: build, bundle check, smoke test
└── dist/                        # frontend build output (Tauri frontendDist)
```

## Keyboard shortcuts

| Keys                          | Action                     |
| ----------------------------- | -------------------------- |
| `Ctrl+Shift+T` / `Ctrl+Shift+W` | New tab / close tab      |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Next / previous tab        |
| `Alt+1..9`                    | Jump to tab                |
| Middle-click tab              | Close tab                  |
| `Ctrl+Shift+F`, `Enter`, `Esc` | Find, next match, close   |
| `Ctrl+Shift+C` / `Ctrl+Shift+V` | Copy / paste             |
| `Ctrl +`, `-`, `0`, `Ctrl+wheel` | Zoom in / out / reset   |
| `R` / `Enter` on exited tab   | Restart process            |

Plain `Ctrl+C` is never hijacked — `SIGINT` keeps working.

## Settings

Gear icon in the titlebar. Persisted in `localStorage` (`atlas.settings`;
font-size default in `atlas.terminal.fontSize`):

- **Shell / startup directory / scrollback** — apply to new tabs
- **Font family** — applies live to all open terminals
  (default: `'Cascadia Mono', 'Cascadia Code', Consolas, monospace` —
  inbox Windows fonts with exact cell metrics; Nerd Fonts opt-in here)
- **Opacity / blur** — live via CSS variables on the glass window

## Building

> Project rule: **no local Tauri/Rust toolchain — all native builds run in
> GitHub Actions.** Frontend-only work (`tsc`, `vite build`) runs locally.

Every push builds on `windows-latest`: pnpm install → `tauri build` →
NSIS/MSI/exe artifacts (upload fails the job if empty) → **smoke test** that
launches the exe and asserts it is still alive after 20s (a startup panic exits
silently under `windows_subsystem`, so this is the "it opens" proof).

```bash
pnpm install          # frontend deps
pnpm exec tsc --noEmit
pnpm exec vite build  # frontend bundle (what CI embeds)
```

CI produces the signed-by-nobody (SmartScreen will prompt on first run):
`AtlasTerminal-Windows` artifact with installer + portable exe.

## Requirements (runtime)

- Windows 10/11 64-bit + WebView2 (inbox on Win10 1803+ / Win11)
- PowerShell 5.1 inbox; PowerShell 7 (`pwsh`) optional via Settings
- A Nerd Font (e.g. **CaskaydiaCove NF**, JetBrainsMono NF) for icon/powerline
  glyphs — without one, symbols render as tofu boxes. The default font stack
  prefers Nerd Fonts and falls back to Consolas; install one and restart.

## Roadmap

- [x] Single-tab MVP · lifecycle hardening · tabs · titles/restart/clipboard/zoom
- [x] Find bar · settings · launch smoke test
- [ ] Split panes (Kitty flagship)
- [ ] WebGL renderer toggle (removed: mis-measured fallback fonts on fractional DPR)
- [ ] Backend config file (today: localStorage only)
- [ ] Auto-updater, single-instance, tab drag-reorder
