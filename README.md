<p align="center">
  <img src="assets/app-icon.png" alt="Atlas Terminal Logo" width="120" height="120">
</p>

<h1 align="center">Atlas Terminal</h1>

<p align="center">
  <strong>The fastest, most aesthetic GPU-accelerated terminal for Windows.</strong>
</p>

<p align="center">
  <em>DirectX-accelerated via GPUI · 240Hz Adaptive Motion · Zero Webview / Electron · Restrained Apple/macOS Aesthetic</em>
</p>

<p align="center">
  <a href="https://github.com/akrram0/atlas-terminal/actions/workflows/native.yml"><img src="https://github.com/akrram0/atlas-terminal/actions/workflows/native.yml/badge.svg" alt="CI & Release Pipeline"></a>
  <a href="https://github.com/akrram0/atlas-terminal/releases/latest"><img src="https://img.shields.io/github/v/release/akrram0/atlas-terminal?color=C05A28&logo=github" alt="Latest Release"></a>
  <img src="https://img.shields.io/badge/Windows-11%20%7C%2010-0078D6?logo=windows&logoColor=white" alt="Windows 11">
  <img src="https://img.shields.io/badge/Rust-stable-000000?logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/Engine-GPUI%20DirectX-7C3AED" alt="GPUI">
  <img src="https://img.shields.io/badge/Display-Up%20to%20240Hz-success" alt="240Hz">
  <img src="https://img.shields.io/badge/License-MIT%2FApache--2.0-blue" alt="License">
</p>

---

## The Vision

Terminal emulators on Windows have historically been forced into a painful compromise: either sluggish Electron wrappers with bloated memory footprints, or dated, rigid Win32 console wrappers.

**Atlas Terminal rejects that compromise.** Built from scratch in Rust on top of Zed's GPUI framework, Atlas marries raw GPU DirectX rendering with a restrained, dark-first Apple/macOS design language. Every pixel, quad, glyph, and spring animation is calculated in Rust and rasterized directly on your GPU at your display's native refresh rate — up to 240Hz.

---

## Visual Showcase

<p align="center">
  <img src="assets/screenshots/atlas-light-fastfetch.png" alt="Atlas Terminal - Warm Paper Light Theme" width="49%">
  <img src="assets/screenshots/atlas-dark-opencode.png" alt="Atlas Terminal - Matte Dark Theme with OpenCode TUI" width="49%">
</p>

<p align="center">
  <em>Left: <b>Warm Paper</b> theme running fastfetch with process tabs. Right: <b>Matte Dark</b> theme running OpenCode with seamless 10px rounded corners and full SGR mouse interaction.</em>
</p>

---

## Download & Install

Get the latest release for Windows (x64):

* 📦 **[Download Windows Installer (.exe)](https://github.com/akrram0/atlas-terminal/releases/latest/download/AtlasTerminal-Setup-0.1.15.exe)** *(Recommended: Start menu integration, desktop shortcuts, background auto-updates)*
* 🗜️ **[Download Portable (.zip)](https://github.com/akrram0/atlas-terminal/releases/latest/download/atlas-terminal-windows-x64.zip)**
* 🚀 **[All Releases & Release Notes](https://github.com/akrram0/atlas-terminal/releases)**

---

## Why Atlas?

### 🏎️ Unmatched Speed & 240Hz Fluidity
- **1ms Windows OS Timer Precision**: Invokes `timeBeginPeriod(1)` via `winmm.dll` at process startup, eliminating Windows' default 15.6ms timer quantisation.
- **Adaptive Frame Scheduling**: Runs at **4ms (~240Hz)** during active motion (scrolling, cursor gliding, tab collapsing, button presses), stepping down to 16ms for organic cursor breathing and 33ms when quiescent to consume **0% CPU**.
- **Critically Damped Kinetic Scroll**: Calibrated exponential decay (`-18.0 * dt`) with momentum carryover, flick velocity boost, and continuous ceil-aligned subpixel row interpolation (`(pos - pos.ceil()) * cell_height`) for silky, jump-free history review.

### 🎨 Most Aesthetic Terminal UI on Windows
- **Restrained Apple/macOS Aesthetic**: Zero visual clutter, strict 8pt spatial grid, and clean typography.
- **Seamless Windows 11 DWM Integration**: Frameless edge-to-edge window client area with native DWM corner rounding, eliminating white titlebar bleed.
- **Dynamic 10px Rounded Sheet**: Internal canvas corners dynamically match the background of running TUI applications (OpenCode, Lazygit, Micro) without pixel clipping or background seam exposure.
- **Tactile Button Micro-Interactions**: Real physical feedback on press (micro-depression and opacity dip) with snappy spring release across all buttons, tabs, steppers, and window controls.
- **Dual Curated Themes**: Instant zero-flash crossfading between **Matte Dark** and **Warm Paper**, with automatic Windows dark/light system synchronization.

### 🖱️ Full SGR Mouse Reporting (Clickable TUIs)
- Comprehensive support for SGR mouse modes (`?1000h`, `?1002h`, `?1003h`, `?1006h`).
- Mouse clicks, drags, selections, and scroll wheel gestures work natively inside modern TUI tools like **OpenCode**, **Lazygit**, **Micro**, and **Helix**.
- Hold `Shift` to bypass reporting for standard terminal drag-selection and clipboard copying.

### 🔄 In-App Streaming Background Auto-Updater
- Silent update checks against GitHub Releases.
- Non-blocking streaming chunked downloads with real-time percentage indicators.
- Cryptographic SHA-256 verification (RFC 6234) against official release checksums.
- One-click silent restart applying updates in-place.

### 🪟 Multi-Tab & Split Pane Control
- Multi-PTY tab bar with live process title detection (`pwsh`, `cmd`, `wsl`, `opencode`, etc.).
- Flexible vertical and horizontal split panes with independent working directories, titles, and session lifecycles.
- Fast Command Palette (`Ctrl+K` or `Ctrl+Shift+P`) for keyboard-driven fuzzy command execution.

---

## Architecture & Technology

```
┌────────────────────────────────────────────────────────┐
│                   Atlas Terminal UI                    │
│   (Tabs · Command Palette · Settings · Custom Titlebar) │
├────────────────────────────────────────────────────────┤
│                      GPUI 0.2.2                        │
│      (DirectX DirectComposition · DirectWrite · 240Hz)  │
├──────────────────────────┬─────────────────────────────┤
│   alacritty_terminal     │        portable-pty         │
│  (VTE parser, cell grid, │   (Windows ConPTY process   │
│   SGR mouse, scrollback) │       reader & channels)    │
└──────────────────────────┴─────────────────────────────┘
```

| Layer | Technology | Details |
| :--- | :--- | :--- |
| **Window & Rendering** | `gpui 0.2.2` | Zed's GPU framework running directly on DirectX on Windows |
| **Terminal Core** | `alacritty_terminal 0.26` | Alacritty's high-speed VTE parser, cell grid, and scrollback history |
| **PTY Layer** | `portable-pty 0.8` | Windows ConPTY integration |
| **Async Engine** | `smol 2` | Channel synchronization between ConPTY reader thread & UI thread |
| **Iconography** | Vector SVGs (`crate::icons`) | 100% native Solar Linear stroked vector SVGs (zero unicode emojis) |
| **Typography** | Embedded Nerd Font | Bundled `SymbolsNerdFontMono-Regular.ttf` for icons, fastfetch & starship |

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

## Repository Topics & Tags

For discoverability across GitHub:

`terminal` · `terminal-emulator` · `windows` · `windows-11` · `rust` · `gpui` · `gpu-accelerated` · `directx` · `alacritty` · `conpty` · `aesthetic-ui` · `high-fps` · `smooth-scrolling` · `opencode` · `tui`

---

## Building from Source

Atlas Terminal builds via GitHub Actions on `windows-latest`. To compile locally on Windows:

```powershell
# Prerequisites: Rust stable toolchain with MSVC
git clone https://github.com/akrram0/atlas-terminal.git
cd atlas-terminal/atlas-native
cargo run --release
```

---

## License

Distributed under the MIT or Apache-2.0 License.
