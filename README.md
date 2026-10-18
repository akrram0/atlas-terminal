<p align="center">
  <img src="assets/app-icon.png" alt="Atlas Terminal Logo" width="120" height="120">
</p>

<h1 align="center">Atlas Terminal</h1>

<p align="center">
  <strong>The fastest, most aesthetic GPU-accelerated terminal for Windows.</strong>
</p>

<p align="center">
  <em>DirectX-accelerated via GPUI · 240Hz Adaptive Motion · Zero Webview / Electron · Restrained Apple/macOS Aesthetic · iTerm2 Inline Image Graphics</em>
</p>

<p align="center">
  <a href="https://github.com/akrram0/atlas-terminal/actions/workflows/native.yml"><img src="https://github.com/akrram0/atlas-terminal/actions/workflows/native.yml/badge.svg" alt="CI & Release Pipeline"></a>
  <a href="https://github.com/akrram0/atlas-terminal/releases/latest"><img src="https://img.shields.io/github/v/release/akrram0/atlas-terminal?color=C05A28&logo=github" alt="Latest Release"></a>
  <img src="https://img.shields.io/badge/Windows-11%20%7C%2010-0078D6?logo=windows&logoColor=white" alt="Windows 11">
  <img src="https://img.shields.io/badge/Rust-stable-000000?logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/Engine-GPUI%20DirectX-7C3AED" alt="GPUI">
  <img src="https://img.shields.io/badge/Display-Up%20to%20240Hz-success" alt="240Hz">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue" alt="License: MIT"></a>
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

* 📦 **[Download Windows Installer (.exe)](https://github.com/akrram0/atlas-terminal/releases/latest/download/AtlasTerminal-Setup-0.1.24.exe)** *(Recommended: Start menu integration, desktop shortcuts, background auto-updates)*
* 🗜️ **[Download Portable (.zip)](https://github.com/akrram0/atlas-terminal/releases/latest/download/atlas-terminal-windows-x64.zip)**
* 🚀 **[All Releases & Changelogs](https://github.com/akrram0/atlas-terminal/releases)**

---

## User Guide & Documentation

### 1. Inline Image Rendering (iTerm2 OSC 1337 Protocol)
Atlas Terminal natively intercepts and renders inline images directly inside the terminal grid without relying on Sixel or WebViews.

#### Displaying Images in Fastfetch
You can render photos alongside system info using Fastfetch:
```powershell
fastfetch --logo-type iterm --logo "C:\path\to\your\image.png"
```

#### Making Fastfetch Always Show Your Photo
To display your picture automatically on every `fastfetch` command without typing extra flags, edit your Fastfetch configuration file at `~/.config/fastfetch/config.jsonc` (or `%USERPROFILE%\.config\fastfetch\config.jsonc`):

```jsonc
{
  "$schema": "https://github.com/fastfetch-cli/fastfetch/raw/dev/doc/json_schema.json",
  "logo": {
    "type": "iterm",
    "source": "C:/Users/YourName/.config/fastfetch/avatar.png",
    "width": 38,
    "height": 19,
    "padding": {
      "top": 1,
      "left": 1,
      "right": 3
    }
  },
  "modules": [
    "title",
    "separator",
    "os", "host", "kernel", "uptime", "shell", "display", "cpu", "gpu", "memory"
  ]
}
```

#### Displaying Images from Shell Scripts or Python
Any tool that speaks the standard iTerm2 inline image escape sequence renders natively in Atlas:
```bash
printf '\033]1337;File=inline=1;width=40;height=20:%s\007\n' "$(base64 -w0 photo.png)"
```

---

### 2. Interactive TUI Applications (Full Mouse Control)
Atlas Terminal provides comprehensive SGR mouse reporting (`?1000h`, `?1002h`, `?1003h`, `?1006h`):
- **Clickable Interfaces**: Click buttons, select list items, and collapse accordions inside modern terminal apps like **OpenCode**, **Lazygit**, **Micro**, **Helix**, and **Neovim**.
- **Smooth Mouse Wheel Scrolling**: Scroll through diffs, file trees, and logs directly with the wheel.
- **Copy Text from a Mouse-Aware App**: Hold the **`Shift`** key while dragging to bypass application mouse capture and perform standard terminal selection and clipboard copying.

---

### 3. Workspaces: Tabs & Split Panes
Organize multiple concurrent tasks and long-running services in a single window:
- **New Tab**: Press `Ctrl+Shift+T` or `Ctrl+Shift+N` (or click `+` on the tab bar).
- **Split Vertically**: Press `Ctrl+Shift+E` to divide the screen into side-by-side panes.
- **Split Horizontally**: Press `Ctrl+Shift+D` to divide the screen top and bottom.
- **Maximize Pane**: Press `Ctrl+Shift+M` to temporarily focus entirely on the active pane, and press it again to restore the split layout.
- **Close Pane / Tab**: Press `Ctrl+Shift+W`.
- **Switch Tabs**: Press `Ctrl+Tab` or `Ctrl+Shift+Tab`.

---

### 4. Command Palette (`Ctrl+K` or `Ctrl+Shift+P`)
Press `Ctrl+K` or `Ctrl+Shift+P` at any time to open the instant fuzzy-search Command Palette:
- Jump between tabs and active processes.
- Split panes, close sessions, or toggle fullscreen.
- Switch themes (**Matte Dark** / **Warm Paper**).
- Open Settings directly from your keyboard.

---

### 5. Settings & Theme Customization (`Ctrl+,`)
Press `Ctrl+,` to open the built-in Settings dashboard:
- **Appearance & Themes**: Instant crossfade between **Matte Dark** and **Warm Paper**, or enable automatic Windows system dark/light synchronization.
- **Typography**: Select any installed system font (defaults to `Segoe UI` and `Symbols Nerd Font`), font size, and line height.
- **Cursor Aesthetics**: Switch between smooth kinetic `Block`, `Hollow Block`, `Beam`, and `Underline` with organic breathing alpha.
- **Background Updates**: Configure automatic update checks (30 min, 1 hour, 6 hours, Daily) with silent background streaming.

---

## Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Ctrl+K` / `Ctrl+Shift+P` | Open Command Palette |
| `Ctrl+Shift+T` / `Ctrl+Shift+N` | New Tab |
| `Ctrl+Shift+W` | Close Tab / Active Split Pane |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Next / Previous Tab |
| `Ctrl+Shift+E` | Split Pane Vertically (Side-by-Side) |
| `Ctrl+Shift+D` | Split Pane Horizontally (Top / Bottom) |
| `Ctrl+Shift+M` | Maximize / Restore Active Split Pane |
| `Ctrl+Shift+C` | Copy Selection to Clipboard |
| `Ctrl+Shift+V` | Paste from Clipboard |
| `Ctrl+,` | Toggle Settings Dashboard |
| `Shift + Mouse Drag` | Bypass TUI mouse reporting for text copy |
| `R` | Restart Exited Process Pane |

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

### 🔄 In-App Streaming Background Auto-Updater
- Silent update checks against GitHub Releases with conditional ETag validation.
- Non-blocking streaming chunked downloads with real-time percentage indicators.
- Cryptographic SHA-256 verification (RFC 6234) against official release checksums.
- One-click silent restart applying updates in-place via Inno Setup.

---

## Architecture & Technology

```
┌────────────────────────────────────────────────────────┐
│                   Atlas Terminal UI                    │
│  (Tabs · Command Palette · Settings · Custom Titlebar) │
├────────────────────────────────────────────────────────┤
│                      GPUI 0.2.2                        │
│     (DirectX DirectComposition · DirectWrite · 240Hz)  │
├──────────────────────────┬─────────────────────────────┤
│   alacritty_terminal     │        portable-pty         │
│  (VTE parser, cell grid, │   (Windows ConPTY process   │
│   SGR mouse, scrollback) │       reader & channels)    │
├──────────────────────────┴─────────────────────────────┤
│              image_osc (iTerm2 OSC 1337)               │
│     (Stateful streaming interceptor & BGRA decoder)    │
└────────────────────────────────────────────────────────┘
```

| Layer | Technology | Details |
| :--- | :--- | :--- |
| **Window & Rendering** | `gpui 0.2.2` | Zed's GPU framework running directly on DirectX on Windows |
| **Terminal Core** | `alacritty_terminal 0.26` | Alacritty's high-speed VTE parser, cell grid, and scrollback history |
| **PTY Layer** | `portable-pty 0.8` | Windows ConPTY integration |
| **Inline Graphics** | `image_osc` | Stateful OSC 1337 scanner + GPU-ready BGRA `gpui::RenderImage` textures |
| **Async Engine** | `smol 2` | Channel synchronization between ConPTY reader thread & UI thread |
| **Iconography** | Vector SVGs (`crate::icons`) | 100% native Solar Linear stroked vector SVGs (zero unicode emojis) |
| **Typography** | Embedded Nerd Font | Bundled `SymbolsNerdFontMono-Regular.ttf` for icons, fastfetch & starship |

---

## Building from Source

Atlas Terminal compiles via GitHub Actions on `windows-latest`. To compile locally on Windows:

```powershell
# Prerequisites: Rust stable toolchain with MSVC
git clone https://github.com/akrram0/atlas-terminal.git
cd atlas-terminal/atlas-native
cargo run --release
```

---

## License

This project is licensed under the [MIT License](LICENSE).
