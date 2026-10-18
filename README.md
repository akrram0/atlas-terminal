# Atlas Terminal

A native, GPU-accelerated terminal emulator for Windows — **no webview, no CSS,
no HTML**. Window chrome, tabs, split panes, the command palette and the
terminal grid itself are all drawn from Rust code on the GPU.

[![CI & Release Pipeline](https://github.com/akrram0/atlas-terminal/actions/workflows/native.yml/badge.svg)](https://github.com/akrram0/atlas-terminal/actions/workflows/native.yml)
[![Latest Release](https://img.shields.io/github/v/release/akrram0/atlas-terminal?color=C05A28&logo=github)](https://github.com/akrram0/atlas-terminal/releases/latest)
![Windows](https://img.shields.io/badge/Windows-0078D6?logo=windows&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-stable-000000?logo=rust&logoColor=white)
![gpui](https://img.shields.io/badge/UI-gpui%200.2-7C3AED)

## Download

Get the latest release for Windows (x64):

* 📦 **[Download Windows Installer (.exe)](https://github.com/akrram0/atlas-terminal/releases/latest/download/AtlasTerminal-Setup-0.1.12.exe)**
* 🗜️ **[Download Portable (.zip)](https://github.com/akrram0/atlas-terminal/releases/latest/download/atlas-terminal-windows-x64.zip)**
* 🚀 **[All Releases & Changelog](https://github.com/akrram0/atlas-terminal/releases)**


## Stack

| Layer | Crate | Notes |
| --- | --- | --- |
| Window, UI, GPU rendering | `gpui 0.2.2` | Zed's UI framework (DirectX on Windows) |
| Terminal emulation | `alacritty_terminal 0.26` | VTE parser, grid, scrollback, attributes |
| PTY | `portable-pty 0.8` | ConPTY on Windows |
| Async plumbing | `smol 2` | PTY reader thread → UI channel |

There is no stylesheet anywhere: a theme is a Rust struct
(`atlas-native/src/theme.rs`) — one source of truth for chrome, the ANSI
palette and the cursor (light / dark).

## Features

- **Tabs** — each tab owns a real PTY; titles come from the shell (OSC 0/2)
  with a Rename dialog on top; hidden tabs keep running.
- **Split panes** — side-by-side panes per tab (sidebar split icon, command
  palette or the pane `⋯` menu); click a pane to focus it, close panes
  independently. Every pane keeps its own PTY, title, cwd and exit state.
- **Profiles** — PowerShell and Ubuntu (WSL). The status bar and breadcrumb
  track each pane's live working directory and git branch; WSL paths are read
  from the shell title (`/mnt/c/…`) and mapped back for git lookups.
- **Command palette** — `Ctrl+Shift+P`, the titlebar search pill or the pane
  `⋯` menu. New / close / split tabs and panes, rename, theme, settings,
  sidebar, font size — typed filtering, ↑/↓ + Enter.
- **Settings page** — an in-app page, not a popup: light/dark theme, font
  size, cursor style (Auto / Block / Bar / Under), About + GitHub link.
- **Terminal core** — full keymap (arrows including DECCKM, F1–F12, control
  chars, Alt=Meta), 256-color palette, bold/italic/underline/strikethrough,
  wide chars, block/beam/underline cursor, inverse video, scrollback
  (`Shift+PageUp/PageDown` and wheel), bracketed paste.
- **Chrome** — frameless window with a custom titlebar (drag region, native
  minimize / maximize / close), collapsible sidebar (46 px rail), breadcrumb
  header, status bar (shell, cwd/branch, grid size, state).

## Project layout

```
atlas-native/
  Cargo.toml
  src/
    main.rs      — window setup (frameless, caption slots, rounded corners)
    view.rs      — AtlasView: chrome, palette, settings page, TerminalElement
    session.rs   — Session: portable-pty + alacritty Term + reader thread
    keymap.rs    — Keystroke → PTY bytes
    theme.rs     — light/dark theme structs (chrome + ANSI + cursor)
    cwd.rs       — live working directory (PEB read)
.github/workflows/native.yml — CI: build + smoke test + rolling release
```

## Building (CI only)

There is intentionally no local Rust toolchain requirement: every push to
`main` that touches `atlas-native/**` builds on `windows-latest`
(`.github/workflows/native.yml`), smoke-tests the exe, uploads the
`atlas-native-poc` artifact and refreshes the rolling release above.

If the toolchain is installed anyway, `cargo run` inside `atlas-native/`
works; a release build additionally wants `GPUI_FXC_PATH` (Windows SDK
`fxc.exe`) on this gpui version — the CI locates it automatically.

## Known limits (POC)

- No text selection or buffer search yet.
- No IME composition in the palette / rename inputs.
- No font-family picker (font stack prefers Cascadia-style Nerd Fonts).
- Splits are equal width (no drag-to-resize divider).
