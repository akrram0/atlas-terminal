<p align="center">
  <img src="assets/app-icon.png" alt="Atlas Terminal Logo" width="100" height="100">
</p>

<h1 align="center">Atlas Terminal</h1>

<p align="center">
  <strong>Fast, aesthetic GPU-accelerated terminal for Windows.</strong><br>
  DirectX-accelerated · Native Windows 11 Acrylic Blur · Seamless Tabs & Splits · Zero Webview / Electron
</p>

<p align="center">
  <a href="https://github.com/akrram0/atlas-terminal/releases/latest"><img src="https://img.shields.io/github/v/release/akrram0/atlas-terminal?color=0ea5e9&label=release" alt="Latest Release"></a>
  <img src="https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-blue?logo=windows&logoColor=white" alt="Windows 10 / 11">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="MIT License">
  <a href="https://github.com/akrram0/atlas-terminal/actions/workflows/native.yml"><img src="https://github.com/akrram0/atlas-terminal/actions/workflows/native.yml/badge.svg" alt="CI Status"></a>
</p>

---

## 🚀 Download & Install

Get the latest stable release for Windows (x64):

| Package | Description | Download |
| :--- | :--- | :--- |
| **Windows Installer (`.exe`)** | Automatic setup with Start Menu integration, desktop shortcut, and background updates | [Download Installer](https://github.com/akrram0/atlas-terminal/releases/latest) |
| **Portable Archive (`.zip`)** | Standalone zero-install archive — extract and run anywhere | [Download Portable `.zip`](https://github.com/akrram0/atlas-terminal/releases/latest) |

> Release notes, previous versions, and SHA-256 checksums are available on the [GitHub Releases page](https://github.com/akrram0/atlas-terminal/releases).

---

## ✨ Features at a Glance

- **⚡ GPU DirectX Acceleration**: Native DirectX rendering built in Rust on GPUI for instant startup (< 250ms), low-latency input, and 0% CPU consumption when idle.
- **🎨 Windows 11 Acrylic Glass**: Native DWM Acrylic translucency, pure glass mode, and curated dark/light themes (Tokyo Night, Nord, Catppuccin, Solarized, Gruvbox, and more).
- **📑 Tabs, Splits & Sidebar Layouts**:
  - Horizontal splits (`Alt+D`) and vertical splits (`Alt+Enter`).
  - Maximize single panes (`Alt+Z`) to focus without losing your layout.
  - Choose between a horizontal top bar, a vertical sidebar (`Ctrl+B`), or borderless mode.
- **📜 Interactive Scrollbars**: Draggable Apple-style glass scrollbars with track click-paging and full keyboard navigation (`PageUp`, `PageDown`, `Home`, `End`) for both terminal panes and the Settings view.
- **🖼️ Inline Graphics (iTerm2 OSC 1337)**: Full support for inline graphics and `fastfetch` system logos directly within your terminal scrollback without external tools.
- **📐 Vector Box Drawing**: Hardware-accelerated vector paths for box-drawing characters and powerline symbols (`╭`, `╰`, `─`, `│`), eliminating 1px gaps and jagged prompt lines.
- **🔍 In-Buffer Search (`Ctrl+F`)**: Floating search bar with live buffer highlighting, match navigation (<kbd>Enter</kbd> / <kbd>Shift+Enter</kbd>), and viewport auto-scrolling.
- **⚡ Click-to-Code (`Ctrl + Click`)**: Click file paths and compiler errors (e.g. `src/main.rs:42:15`) to open the exact line in VS Code, Zed, Cursor, or your default editor.
- **🌐 Smart URL Launcher (`Ctrl + Click` / `Ctrl+Shift+U`)**: Open links directly in your default browser or browse active screen URLs with the keyboard link picker.
- **⌨️ Command Palette (`Ctrl+K` / `Ctrl+Shift+P`)**: Instant Raycast-style palette with fuzzy search to switch themes, launch shells, and run actions.
- **🔔 Background Job Notifications**: Windows Action Center notifications when long-running commands finish while the window is unfocused.
- **🐱 Pixel Art Companions**: Optional animated pixel art sprites (White Cat, Penguin, Fox, Bat, Knight) for your top bar.

---

## ⌨️ Essential Keyboard Shortcuts

### Tabs & Panes
| Shortcut | Action |
| :--- | :--- |
| `Ctrl+T` | Open New Tab (Default Shell Profile) |
| `Ctrl+W` / `Alt+W` | Close Active Pane / Tab |
| `Alt+Enter` | Split Pane Vertically (Side by Side) |
| `Alt+D` | Split Pane Horizontally (Top / Bottom) |
| `Alt+Space` | Toggle Split Orientation (Horizontal ↔ Vertical) |
| `Alt+Arrows` / `Alt+H/J/K/L` | Navigate Between Split Panes |
| `Alt+Shift+Arrows` | Resize Active Split Pane |
| `Alt+Z` | Maximize / Restore Active Split Pane |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Next / Previous Tab |
| `Ctrl+1` … `Ctrl+9` | Direct Jump to Tab 1–9 |
| `Ctrl+B` | Toggle Sidebar / Top Tab Bar |

### Navigation & Actions
| Shortcut | Action |
| :--- | :--- |
| `Ctrl+K` / `Ctrl+Shift+P` | Open Command Palette / Shell Switcher |
| `Ctrl+F` | Find / Search in Terminal Buffer |
| `Ctrl+,` | Open Settings Dashboard |
| `Esc` | Close Settings / Search / Palette |
| `Ctrl + Click` | Open URL or Jump to Code in Editor |
| `Ctrl+Shift+U` | Open Keyboard Link Picker |
| `Alt+T` | Cycle Theme Preset |
| `Ctrl+Shift+L` | Toggle Dark / Light Theme Mode |
| `F11` | Toggle Borderless Fullscreen |
| `Ctrl+Shift+1` … `5` | Quick Launch PowerShell / WSL / CMD / Git Bash / OpenCode |

---

## ⚙️ Configuration

Atlas Terminal can be customized through the graphical **Settings page (`Ctrl+,`)** or by editing the JSON configuration file directly.

### Configuration File Path
```text
%USERPROFILE%\.config\atlas\settings.json
```

### Common Settings
- **Default Shell**: `powershell`, `wsl`, `cmd`, `gitbash`, or a custom binary path.
- **Typography**: Set custom monospace font family, font size, line height, and cursor shape (Beam, Block, Underline).
- **Appearance**: Acrylic glass blur, background opacity (0.0 to 1.0), and tab bar position (`top`, `sidebar`, `borderless`).
- **Scrollbar**: Set scrollbar visibility mode (`always`, `auto`, `hidden`).
- **Default Editor**: Configures which editor is launched for `Ctrl+Click` file jumps (`code`, `zed`, `cursor`, etc.).

---

## 🛠️ Built-in Developer Tools (`atlas`)

Atlas Terminal includes zero-overhead CLI helpers available via the `atlas` command:

```powershell
# Port Inspector — inspect listening ports or terminate stuck listeners:
atlas port 3000
atlas port 3000 -k

# Offline JWT Inspector — inspect and decode JSON Web Tokens safely:
atlas jwt <token>

# Terminal QR Code Generator:
atlas qr "https://github.com"

# Terminal Throughput Benchmark:
atlas bench

# Inline Image Viewer:
atlas img photo.png
```

Run `atlas tools` for the full list of available satellite commands.

---

## 🔨 Building from Source

### Prerequisites
- **Windows 10 / 11 (64-bit)**
- **Rust Toolchain**: [rustup.rs](https://rustup.rs/) (`x86_64-pc-windows-msvc` target)
- **Git**

### Build Steps
```powershell
# 1. Clone repository
git clone https://github.com/akrram0/atlas-terminal.git
cd atlas-terminal

# 2. Compile and run release build
cargo run --release
```

---

## 📄 License

Atlas Terminal is open-source software licensed under the [MIT License](LICENSE).
