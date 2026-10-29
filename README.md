<p align="center">
  <img src="assets/app-icon.png" alt="Atlas Terminal Logo" width="120" height="120">
</p>

<h1 align="center">Atlas Terminal</h1>

<p align="center">
  <strong>The fastest, most aesthetic GPU-accelerated terminal for Windows.</strong>
</p>

<p align="center">
  <em>DirectX-accelerated via GPUI · Ultra-Smooth Native Motion · Zero Webview / Electron · Restrained Apple/macOS Aesthetic · iTerm2 Inline Image Graphics</em>
</p>

<p align="center">
  <a href="https://github.com/akrram0/atlas-terminal/actions/workflows/native.yml"><img src="https://github.com/akrram0/atlas-terminal/actions/workflows/native.yml/badge.svg" alt="CI & Release Pipeline"></a>
  <a href="https://github.com/akrram0/atlas-terminal/releases/latest"><img src="https://img.shields.io/github/v/release/akrram0/atlas-terminal?color=C05A28&logo=github" alt="Latest Release"></a>
  <img src="https://img.shields.io/badge/Windows-11%20%7C%2010-0078D6?logo=windows&logoColor=white" alt="Windows 11">
  <img src="https://img.shields.io/badge/Rust-stable-000000?logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/Engine-GPUI%20DirectX-7C3AED" alt="GPUI">
  <img src="https://img.shields.io/badge/Display-DirectX%20Accelerated-success" alt="DirectX Accelerated">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue" alt="License: MIT"></a>
</p>

---

## The Vision

Terminal emulators on Windows have historically forced developers into a compromise: either sluggish Electron wrappers with bloated memory footprints, or dated, rigid Win32 console wrappers.

**Atlas Terminal rejects that compromise.** Built from scratch in Rust on top of Zed's GPUI framework, Atlas combines raw GPU DirectX rendering with a restrained, dark-first Apple/macOS design language. Every pixel, quad, glyph, and spring animation is calculated in Rust and rasterized directly on your GPU at your display's native refresh rate with zero frame drops.

---

## Download & Install

Get the latest release for Windows (x64):

| Package | Description | Link |
| :--- | :--- | :--- |
| **Windows Installer** | Start Menu integration, desktop shortcuts & background auto-updates | [Download `.exe`](https://github.com/akrram0/atlas-terminal/releases/latest) |
| **Portable Archive** | Standalone zero-install archive, extract and run | [Download `.zip`](https://github.com/akrram0/atlas-terminal/releases/latest/download/atlas-terminal-windows-x64.zip) |
| **All Releases** | Release history, changelogs, and SHA-256 checksums | [View Releases](https://github.com/akrram0/atlas-terminal/releases) |

---

## Documentation

Comprehensive guides and architecture specifications are available in the [`docs/`](docs/) directory:

| Guide | Description |
| :--- | :--- |
| 📖 **[User Guide](docs/user-guide.md)** | Complete manual: installation, Fastfetch image setup, keyboard shortcuts, split panes, hyperlinks, and WSL profiles. |
| 🏗️ **[Architecture & Engine Design](docs/architecture.md)** | Technical deep-dive: GPUI DirectX rendering pipeline, Windows DWM acrylic integration, and damped spring cursor kinematics. |

---

## Key Features

### 🏎️ Instant Speed & Fluid Motion
- **Precision Frame Timing**: Optimizes system timer resolution at startup, ensuring immediate keystroke feedback and smooth animation.
- **Adaptive Frame Scheduling**: Renders with ultra-low latency during active motion (scrolling, cursor gliding, tab collapsing) and steps down to consume **0% CPU** when idle.
- **Kinetic Smooth Scroll**: Exponential decay with momentum carryover, flick velocity boost, and subpixel row interpolation for jump-free history review.

### 🎨 Apple/macOS Aesthetic on Windows
- **Restrained Visual Hierarchy**: Strict 8pt spatial grid, SF-inspired typographic hierarchy, and zero arbitrary decorative clutter.
- **Native Windows 11 DWM Acrylic**: Full-bleed frameless client area with hardware-accelerated acrylic blur and native DWM corner rounding.
- **Dynamic Alt-Screen Sheets**: Alternate screen applications (OpenCode, Lazygit, Neovim) seamlessly match theme backgrounds without corner clipping or transparency bleed.
- **Curated Themes**: Instant zero-flash crossfading between **Matte Dark** and **Warm Paper**, plus automatic Windows dark/light system synchronization.

### 🖼️ Native Inline Graphics (iTerm2 OSC 1337)
- Renders images directly within the terminal grid without Sixel or WebViews.
- Fully compatible with `fastfetch`, CLI tools, and shell scripts.
- Text reflow and scrollback keep graphics 1:1 anchored to their output lines.

### 📐 Seam-Free Vector Box Drawing & Prompt Precision
- **Procedural Prompt Geometry**: Renders all box-drawing and corner characters (`U+2500..=U+257F`, including `╭`, `╰`, `─`, `│`) procedurally with hardware-accelerated vector paths rather than delegating them to monospace fonts.
- **Dynamic Line Scaling**: Line weights scale automatically with cell dimensions so prompts remain sharp and balanced at any font size.
- **Seamless Vector Corners**: Eliminates jagged stair-stepping and baseline drift on zoom-out. Prompts (Starship, Oh-My-Posh, Powerlevel10k, WSL) connect flawlessly without 1px gaps.

### 🖱️ Modern TUI & Mouse Reporting
- Complete SGR mouse tracking (`?1000h` / `?1006h`): click buttons, select lists, and scroll diffs inside interactive terminal applications.
- Hold `Shift` while dragging to bypass application mouse capture for standard text selection.
- `Ctrl + Click` to open any visible URL directly in your browser.

### 📑 Workspaces, Tabs & Split Panes
- Organize complex workflows with horizontal (`Ctrl+Shift+D`) and vertical (`Ctrl+Shift+E`) split panes.
- Temporary pane maximization (`Ctrl+Shift+M`) to focus on single tasks without breaking split layouts.
- Auto-detected WSL Linux distributions (`Ubuntu`, `Debian`, `Arch`, `Fedora`, etc.) with native vector marks.

### 🔍 Find in Terminal (`Ctrl+F`)
- Floating macOS-style search bar with live buffer highlighting across the active screen and scrollback history.
- Match stepping (<kbd>Enter</kbd> for next, <kbd>Shift</kbd>+<kbd>Enter</kbd> for previous) with live match counter (`3 of 18`).
- Smooth viewport auto-scrolling directly to matching lines.

### ⚡ Click-to-Code: Smart File Jumps (`Ctrl`+Click)
- Click compiler errors, panic stack traces, and source coordinates (e.g. `src/main.rs:42:15`, `app.ts:18`) to jump directly to the exact file, line, and column.
- Configurable default editor: **Auto-detect**, **VS Code** (`code --goto`), **Cursor**, **Zed**, or system default.
- Seamlessly resolves relative file paths against the active pane's current working directory.

### 🔔 Native Background Job Notifications
- Native Windows Action Center toast alerts when long-running commands finish while Atlas is unfocused or minimized.
- Configurable duration threshold (5s, 10s, 30s, 60s) and full toggle in Settings.

### 🔍 Floating Command Palette (`Ctrl+K` / `Ctrl+Shift+P`)
- Raycast-style instant palette with multi-word fuzzy search.
- Launch shell profiles, split panes, and actions without touching the mouse.
- Pre-filters visible terminal URLs for one-key link opening (`Ctrl+Shift+O`).

### 🔄 Background Delta Auto-Updater
- Silent update checks against GitHub Releases with conditional ETag validation.
- Background asynchronous streaming downloads with real-time progress indicators.
- Cryptographic SHA-256 validation before staging.

---

## 📊 Measured Performance Benchmarks

All metrics below are empirical measurements taken on **Windows 11 Pro 64-bit** (AMD Ryzen 5 / Intel Core processor, NVMe SSD). You can reproduce and verify these tests directly on your own system using the included benchmark harness:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/benchmark.ps1
```

| Metric | Atlas Terminal | Windows Terminal | Alacritty | WezTerm | Hyper (Electron) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Cold Startup Time** | **< 250 ms** | ~470 ms | ~210 ms | ~390 ms | ~1,950 ms |
| **Memory Usage (Idle)** | **~60 MB** | ~90 MB | ~55 MB | ~135 MB | ~420 MB |
| **Input Latency** | **< 5 ms** | 18 – 32 ms | 7 – 12 ms | 16 – 28 ms | 45 – 85 ms |
| **Streaming Speed** | **~20 MB/s** | ~10 MB/s | ~22 MB/s | ~9 MB/s | ~2 MB/s |
| **Refresh & Motion** | **Native Display Rate** | 60 Hz | 60 / 144 Hz | Fixed FPS | Throttled |
| **Idle CPU Usage** | **0.0%** | < 0.2% | 0.0% | 0.1% – 0.5% | 1.2% – 3.5% |

---

## Essential Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Ctrl+K` / `Ctrl+Shift+P` | Open Command Palette |
| `Ctrl+F` | Find in Terminal (Search & Match Navigation) |
| `Ctrl+Shift+N` | New Tab (Default Shell) |
| `Ctrl+Shift+W` | Close Active Pane / Tab |
| `Ctrl+Shift+E` | Split Pane Vertically (Columns) |
| `Ctrl+Shift+D` | Split Pane Horizontally (Rows) |
| `Ctrl+Shift+M` | Toggle Maximize / Restore Active Pane |
| `Ctrl+Shift+T` | Switch Theme (Dark / Light) |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Next / Previous Tab |
| `Ctrl + Click` | Open Hovered URL or Click-to-Code File Jump |
| `Ctrl+Shift+O` | Keyboard Link Picker |
| `Ctrl+,` | Open Settings Dashboard |
| `F11` | Toggle Borderless Fullscreen |

> For the complete 35+ shortcut reference, see the **[User Guide — Keyboard Shortcuts](docs/user-guide.md#3-keyboard-shortcuts-reference)**.

---

## Architecture & Technology

```
┌────────────────────────────────────────────────────────┐
│                   Atlas Terminal UI                    │
│  (Tabs · Command Palette · Settings · Custom Titlebar) │
├────────────────────────────────────────────────────────┤
│                      GPUI 0.2.2                        │
│     (DirectX DirectComposition · DirectWrite · GPU Native) │
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
| **Terminal Core** | `alacritty_terminal 0.26` | High-speed VTE parser, cell grid, and scrollback history |
| **PTY Layer** | `portable-pty 0.8` | Windows ConPTY integration with UTF-8 byte stream processing |
| **Inline Graphics** | `image_osc` | Stateful OSC 1337 scanner + GPU-ready BGRA `gpui::RenderImage` textures |
| **Vector Box Engine** | `src/terminal/procedural.rs` | Windows Terminal AtlasEngine-inspired procedural cubic Bézier vector engine with dynamic stroke scaling |
| **Async Engine** | `smol 2` | Channel synchronization between ConPTY reader thread & UI thread |
| **Iconography** | Vector SVGs (`crate::icons`) | 100% native Solar Linear stroked vector SVGs |
| **Typography** | Embedded Nerd Font | Bundled `SymbolsNerdFontMono-Regular.ttf` for icons, fastfetch & starship |

---

## Building from Source

### Prerequisites
- **Windows 10 / 11** (x86_64)
- **Rust stable toolchain** (MSVC target): `rustup default stable-x86_64-pc-windows-msvc`
- **Git**

### Build & Run
```powershell
# Clone repository
git clone https://github.com/akrram0/atlas-terminal.git
cd atlas-terminal

# Run release build
cargo run --release
```

---

## License

Atlas Terminal is open-source software licensed under the [MIT License](LICENSE).
