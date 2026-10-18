# Atlas Terminal

A blisteringly fast, GPU-accelerated native terminal emulator for Windows — **zero webview, zero Electron, zero HTML/CSS**. Window chrome, tabs, split panes, command palette, and the terminal grid itself are all rendered directly on DirectX via GPUI.

[![CI & Release Pipeline](https://github.com/akrram0/atlas-terminal/actions/workflows/native.yml/badge.svg)](https://github.com/akrram0/atlas-terminal/actions/workflows/native.yml)
[![Latest Release](https://img.shields.io/github/v/release/akrram0/atlas-terminal?color=C05A28&logo=github)](https://github.com/akrram0/atlas-terminal/releases/latest)
![Windows 11](https://img.shields.io/badge/Windows-11%20%7C%2010-0078D6?logo=windows&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-stable-000000?logo=rust&logoColor=white)
![gpui](https://img.shields.io/badge/UI-gpui%200.2-7C3AED)
![FPS](https://img.shields.io/badge/Display-Up%20to%20240Hz-success)

---

## Preview

<p align="center">
  <img src="assets/screenshots/atlas-light-fastfetch.png" alt="Atlas Terminal - Light / Paper Theme with fastfetch" width="49%">
  <img src="assets/screenshots/atlas-dark-opencode.png" alt="Atlas Terminal - Dark Theme running OpenCode TUI" width="49%">
</p>

*Left: Warm Paper theme running fastfetch with process tabs. Right: Matte Dark theme running OpenCode with smooth 10px rounded corners and full SGR mouse interaction.*

---

## Download

Get the latest release for Windows (x64):

* 📦 **[Download Windows Installer (.exe)](https://github.com/akrram0/atlas-terminal/releases/latest/download/AtlasTerminal-Setup-0.1.15.exe)** *(recommended: automatic shortcuts, silent background upgrades)*
* 🗜️ **[Download Portable (.zip)](https://github.com/akrram0/atlas-terminal/releases/latest/download/atlas-terminal-windows-x64.zip)**
* 🚀 **[All Releases & Changelogs](https://github.com/akrram0/atlas-terminal/releases)**

---

## Core Highlights

### ⚡ 240Hz High-FPS Motion & Inertial Scrolling
- **1ms Windows OS Scheduler Resolution**: Activates `timeBeginPeriod(1)` at startup, bypassing Windows 15.6ms timer quantisation.
- **Adaptive Cadence**: Renders at **4ms (~240Hz)** during active scrolling and animations, stepping down to 16ms for organic cursor breathing and 33ms when idle (0% CPU).
- **Critically Damped Kinetic Scroll**: Inertial wheel physics with flick velocity boost and continuous ceil-aligned subpixel row interpolation (`(pos - pos.ceil()) * cell_height`) for jitter-free history browsing.

### 🖱️ Full SGR Mouse Reporting & TUI Support
- Comprehensive support for mouse reporting modes (`?1000h`, `?1002h`, `?1003h`, `?1006h`).
- Interactive TUI applications (**OpenCode**, **Lazygit**, **Micro**, **Helix**) are fully clickable, draggable, and scrollable.
- Intelligent selection system: Shift-click overrides reporting for native terminal text copy/selection.

### 🎨 Apple/macOS-Inspired Restrained Aesthetic
- Frameless native window with client-side titlebar and seamless Windows 11 DWM integration.
- Smooth **10px** inner rounded corners that adapt dynamically to the terminal or TUI background color without corner bleeding or fractional pixel clipping.
- Tactile button micro-interactions with physical depression feedback and spring release on all interactive controls.
- Dual curated themes: **Matte Dark** and **Warm Paper**, plus automatic system dark/light switching.

### 🔄 In-App Background Auto-Updater
- One-click update detection from GitHub Releases with non-blocking streaming background downloads.
- RFC 6234 cryptographic SHA-256 checksum verification against official release manifests.
- Silent Inno Setup restart applying upgrades seamlessly in-place.

### 🪟 Tabs & Split Panes
- Multi-tab PTY management with process-aware titles and icons (`pwsh`, `cmd`, `wsl`, `opencode`, etc.).
- Flexible vertical and horizontal split panes with independent cwd, titles, and session lifecycles.
- Fast Command Palette (`Ctrl+K` or `Ctrl+Shift+P`) for fuzzy action discovery.

---

## Architecture & Stack

| Layer | Technology | Role |
| :--- | :--- | :--- |
| **Window & GPU Engine** | `gpui 0.2.2` | Zed's native GPU framework (DirectX / DirectWrite on Windows) |
| **Terminal Core** | `alacritty_terminal 0.26` | VTE parser, cell grid, scrollback buffer, attributes |
| **PTY Layer** | `portable-pty 0.8` | Windows ConPTY integration |
| **Async Runtime** | `smol 2` | Lock-free PTY channel and background executor |
| **Icons & Fonts** | Vector SVGs + Nerd Font | Embedded Solar Linear vector icons & Symbols Nerd Font |

---

## Project Structure

```
atlas-terminal/
├── assets/
│   ├── icons/                  # Solar Linear stroked vector SVGs
│   ├── screenshots/            # Showcase screenshots
│   └── SymbolsNerdFontMono.ttf # Bundled Nerd Font glyphs
├── atlas-native/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs             # Window setup, 1ms timer, DWM corner rounding
│       ├── view.rs             # AtlasView: chrome, 240Hz ticker, tabs, palette
│       ├── session.rs          # ConPTY reader thread & Alacritty term state
│       ├── updater.rs          # Background streaming updater & SHA-256 validation
│       ├── keymap.rs           # Keystroke mapping & SGR mouse encoding
│       ├── theme.rs            # Dual-theme palette definitions & interpolation
│       └── ui/                 # Controls, settings, palette, animations
├── installer/
│   └── atlas-terminal.iss      # Inno Setup Windows installer script
└── .github/workflows/
    └── native.yml              # CI/CD: build, test & automated release pipeline
```

---

## Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Ctrl+K` / `Ctrl+Shift+P` | Open Command Palette |
| `Ctrl+Shift+T` / `Ctrl+Shift+N` | New Tab |
| `Ctrl+Shift+W` | Close Tab / Pane |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Next / Previous Tab |
| `Ctrl+Shift+D` | Split Horizontal |
| `Ctrl+Shift+E` | Split Vertical |
| `Ctrl+Shift+M` | Maximize / Restore Active Pane |
| `Ctrl+Shift+C` | Copy Selection to Clipboard |
| `Ctrl+Shift+V` | Paste from Clipboard |
| `Ctrl+,` | Toggle Settings Page |
| `R` | Restart Exited Process Pane |

---

## Building from Source

Atlas Terminal builds automatically on GitHub Actions (`windows-latest`). To build locally on Windows:

```powershell
# Prerequisites: Rust stable toolchain with MSVC
git clone https://github.com/akrram0/atlas-terminal.git
cd atlas-terminal/atlas-native
cargo run --release
```

---

## License

MIT / Apache-2.0
