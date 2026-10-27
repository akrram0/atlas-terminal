# Atlas Terminal — User Guide

Welcome to **Atlas Terminal**, the fastest and most aesthetic GPU-accelerated terminal for Windows, built with Rust and GPUI.

---

## Table of Contents
1. [Installation & Setup](#1-installation--setup)
2. [Interface Anatomy](#2-interface-anatomy)
3. [Keyboard Shortcuts Reference](#3-keyboard-shortcuts-reference)
4. [Workspaces: Tabs & Split Panes](#4-workspaces-tabs--split-panes)
5. [Command Palette (`Ctrl+K` / `Ctrl+Shift+P`)](#5-command-palette-ctrlk--ctrlshiftp)
6. [Hyperlinks & Mouse Interactions](#6-hyperlinks--mouse-interactions)
7. [Inline Image Graphics (iTerm2 OSC 1337)](#7-inline-image-graphics-iterm2-osc-1337)
   - [Fastfetch Setup](#fastfetch-setup)
   - [CLI Overrides](#cli-overrides)
   - [Shell Scripts & Python Examples](#shell-scripts--python-examples)
8. [Interactive TUI Applications](#8-interactive-tui-applications)
9. [Customization & Settings (`Ctrl+,`)](#9-customization--settings-ctrl)
   - [Themes & Luminance](#themes--luminance)
   - [Acrylic Blur & Opacity](#acrylic-blur--opacity)
   - [Typography & Fonts](#typography--fonts)
   - [Cursor Aesthetics](#cursor-aesthetics)
10. [WSL & Shell Profiles](#10-wsl--shell-profiles)
11. [Auto-Updater & Releases](#11-auto-updater--releases)

---

## 1. Installation & Setup

### Windows Installer (Recommended)
Download the installer executable from the latest release:
- 📦 **[Download Latest Installer (`.exe`)](https://github.com/akrram0/atlas-terminal/releases/latest)**

The installer automatically:
- Installs Atlas Terminal into `%LOCALAPPDATA%\Programs\Atlas Terminal`
- Creates Start Menu and Desktop shortcuts
- Enables background delta updates and file associations

### Portable Archive
If you prefer a standalone setup without an installer:
- 🗜️ **[Download Latest Portable (`.zip`)](https://github.com/akrram0/atlas-terminal/releases/latest/download/atlas-terminal-windows-x64.zip)**
- Extract the archive to any directory and run `atlas-terminal.exe`. All configuration files are cleanly isolated in `%USERPROFILE%\.config\atlas`.

---

## 2. Interface Anatomy

Atlas Terminal is designed around a restrained Apple/macOS aesthetic with zero unnecessary borders or nested cards:

```
┌────────────────────────────────────────────────────────────────────────┐
│ [⊞] [🌿 pwsh ×] [🌿 bash] [+]              [🔍] [⚙] [-] [□] [✕]      │  <- 36px Unified Chrome Bar
├────────────────────────────────────────────────────────────────────────┤
│                                                                        │
│  🌿 Neweye@AKRAM-PC:~$ fastfetch                                       │  <- Edge-to-Edge Terminal Canvas
│                                                                        │
│   ████████   Neweye@AKRAM-PC                                           │  <- Native OSC 1337 Images
│   ████████   ---------------                                           │
│   ████████   OS: Windows 11 / WSL2 (Ubuntu)                            │
│   ████████   Host: Atlas Native Engine                                 │
│                                                                        │
└────────────────────────────────────────────────────────────────────────┘
 ^ 14px rounded outer window frame with hardware-accelerated acrylic blur
```

- **Top Bar (36px)**: Unified tab control plane with minimal flat pills, process icons, and tight proportional window controls.
- **Terminal Canvas**: Edge-to-edge rendering running directly beneath the chrome bar without ugly nested margins.
- **Frameless Window**: Windows 11 DWM rounded corners (`14px` outer radius, `13px` inner) with client-area transparency.

---

## 3. Keyboard Shortcuts Reference

Atlas Terminal is designed from the ground up to be completely operable from the keyboard:

| Category | Shortcut | Action | Description |
| :--- | :--- | :--- | :--- |
| **Command Palette** | `Ctrl+K` / `Ctrl+Shift+P` | **Open Palette** | Open Command Palette with multi-word search |
| **Tabs** | `Ctrl+Shift+T` / `Ctrl+Shift+N` | **New Tab** | Open a new tab with the default shell profile |
| | `Ctrl+Shift+W` | **Close Tab / Pane** | Close active pane (or tab if single pane) |
| | `Ctrl+Tab` / `Ctrl+PageDown` | **Next Tab** | Cycle focus to the next tab |
| | `Ctrl+Shift+Tab` / `Ctrl+PageUp` | **Previous Tab** | Cycle focus to the previous tab |
| | `Ctrl+1` … `Ctrl+8` | **Switch Tab** | Jump directly to tab index 1 through 8 |
| | `Ctrl+9` | **Last Tab** | Jump directly to the last tab |
| **Split Panes** | `Ctrl+Shift+E` | **Split Vertical** | Divide active pane into side-by-side columns |
| | `Ctrl+Shift+D` | **Split Horizontal** | Divide active pane into top-and-bottom rows |
| | `Ctrl+Shift+]` / `Alt+Right` | **Focus Next Pane** | Move keyboard focus to the adjacent pane |
| | `Ctrl+Shift+[` / `Alt+Left` | **Focus Prev Pane** | Move keyboard focus to the previous pane |
| | `Ctrl+Shift+M` | **Toggle Maximize** | Temporarily maximize active pane (toggle) |
| **Scrolling** | `Shift+PageUp` | **Page Up** | Smooth kinetic scroll up 15 lines |
| | `Shift+PageDown` | **Page Down** | Smooth kinetic scroll down 15 lines |
| | `Ctrl+Shift+Up` | **Scroll 1 Line Up** | Fine-grained scroll up by single line |
| | `Ctrl+Shift+Down` | **Scroll 1 Line Down** | Fine-grained scroll down by single line |
| | `Shift+Home` | **Scroll to Top** | Jump immediately to top of scrollback history |
| | `Shift+End` | **Scroll to Bottom** | Return immediately to prompt / bottom of history |
| | `Ctrl+Shift+K` | **Clear Scrollback** | Clear terminal history and reset viewport |
| **Selection & Clipboard** | `Ctrl+Shift+A` | **Select All** | Select all visible text in current viewport |
| | `Ctrl+Shift+C` (or `Ctrl+C`) | **Copy** | Copy selected text to system clipboard |
| | `Ctrl+Shift+V` | **Paste** | Paste system clipboard into active shell |
| | `Shift + Mouse Drag` | **Bypass Mouse** | Force text selection in mouse-aware TUI apps |
| **Zoom & Sizing** | `Ctrl+=` / `Ctrl++` | **Zoom In** | Increase font size by 1px |
| | `Ctrl+-` | **Zoom Out** | Decrease font size by 1px |
| | `Ctrl+0` | **Reset Zoom** | Reset font size to default (12px) |
| **Links & URLs** | `Ctrl + Click` | **Open Link** | Open hovered URL in default browser |
| | `Ctrl+Shift+O` | **Link Picker** | Pre-filter Command Palette with visible URLs |
| **Window & Shell** | `F11` | **Fullscreen** | Toggle borderless fullscreen |
| | `Ctrl+,` | **Settings** | Open Atlas settings dashboard |
| | `Ctrl+Shift+T` | **Toggle Theme** | Toggle Dark / Light theme preset |
| | `R` | **Restart Process** | Restart an exited process or dead shell pane |

---

## 4. Workspaces: Tabs & Split Panes

Organize multiple concurrent tasks, builds, and long-running services in a single window:

- **New Tabs**: Press `Ctrl+Shift+T` or click the `+` button in the top bar. New tabs inherit the directory or default profile configured in settings.
- **Split Panes**:
  - `Ctrl+Shift+E` splits vertically into side-by-side columns.
  - `Ctrl+Shift+D` splits horizontally into rows.
  - Press `Ctrl+Shift+]` / `Ctrl+Shift+[` or `Alt+Arrows` to navigate between active panes.
- **Maximize Active Pane**: Press `Ctrl+Shift+M` to temporarily expand the active pane to fill the entire window canvas. Press `Ctrl+Shift+M` again to restore the split layout without interrupting running processes.
- **Close Pane vs Tab**: `Ctrl+Shift+W` closes the active focused pane. When only one pane remains, `Ctrl+Shift+W` closes the tab. If only one tab remains, closing it gracefully closes the window.

---

## 5. Command Palette (`Ctrl+K` / `Ctrl+Shift+P`)

Press `Ctrl+K` or `Ctrl+Shift+P` at any time to open the floating, Raycast-style Command Palette:

- **Multi-Word Search**: Filter actions naturally using conversational keywords (e.g., `new wsl`, `split rows`, `open link`, `clear`).
- **Direct Shell Spawning**: Launch dedicated tabs for **PowerShell**, **Ubuntu (WSL)**, **Command Prompt (`cmd.exe`)**, **Git Bash**, or **OpenCode CLI**.
- **Buffer Link Opener**: Any URLs currently visible in your active terminal viewport appear automatically as actionable items (`Open link: https://...`).
- **Keyboard Navigation**: Navigate entries with `↑` and `↓`, execute with `Enter`, or dismiss with `Esc`.

---

## 6. Hyperlinks & Mouse Interactions

Atlas Terminal automatically scans visible terminal rows and wrapped lines for hyperlinks:

- **Supported Protocols**: `https://`, `http://`, `file://`, `www.`, `git://`, `ssh://`, `ftp://`, and `mailto:`.
- **Hover Affordance**: Hold **`Ctrl`** and hover your mouse over any URL. The link highlights in your theme's accent color with an underline and hand pointer cursor.
- **Instant Launch**: **`Ctrl + Click`** opens the URL directly in your default browser or associated Windows application without disturbing terminal selection.
- **Smart Soft-Wrapping**: URLs that wrap across multiple terminal columns are seamlessly reassembled and opened intact.
- **Keyboard Link Picker (`Ctrl+Shift+O`)**: Press `Ctrl+Shift+O` to open links without leaving the keyboard. If multiple links exist in the viewport, the Command Palette opens pre-filtered with every visible link ready for selection with arrow keys and `Enter`.
- **Copy on Select**: Enable "Copy on select" in Settings (`Ctrl+,`) to automatically copy highlighted text to the clipboard when you release the mouse button.

---

## 7. Inline Image Graphics (iTerm2 OSC 1337)

Atlas Terminal natively intercepts and renders inline images directly inside the terminal grid on the GPU, eliminating the performance penalties and pixel limitations of Sixel or WebViews.

### Fastfetch Setup

To display your profile picture or logo alongside system info in Fastfetch:

1. Create or edit `%USERPROFILE%\.config\fastfetch\config.jsonc` (or `~/.config/fastfetch/config.jsonc`):

```jsonc
{
  "$schema": "https://github.com/fastfetch-cli/fastfetch/raw/dev/doc/json_schema.json",
  "logo": {
    "type": "iterm",
    "source": "C:/Users/YourUsername/Pictures/avatar.png",
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

2. Run `fastfetch` in Atlas Terminal. The graphic renders anchored cleanly on the left with vertical centering relative to the spec block.

### CLI Overrides

You can also pass images directly on the command line. Specify the grid dimensions (`--logo-width` and `--logo-height` in character cells) so Fastfetch formats the text columns accurately:

```powershell
fastfetch --logo-type iterm --logo "C:\path\to\your\image.png" --logo-width 40 --logo-height 20
```

### Shell Scripts & Python Examples

Any script or tool that outputs standard iTerm2 inline image escape sequences renders natively in Atlas:

#### Bash / Zsh
```bash
printf '\033]1337;File=inline=1;width=40;height=20:%s\007\n' "$(base64 -w0 photo.png)"
```

#### PowerShell
```powershell
$b64 = [Convert]::ToBase64String([IO.File]::ReadAllBytes("photo.png"))
Write-Host "$([char]27)]1337;File=inline=1;width=40;height=20:$b64$([char]7)"
```

#### Python
```python
import base64, sys

with open("photo.png", "rb") as f:
    encoded = base64.b64encode(f.read()).decode("ascii")
sys.stdout.write(f"\033]1337;File=inline=1;width=40;height=20:{encoded}\007\n")
sys.stdout.flush()
```

---

## 8. Interactive TUI Applications

Atlas Terminal provides complete SGR mouse reporting (`?1000h`, `?1002h`, `?1003h`, `?1006h`):

- **Clickable Interfaces**: Click buttons, select list items, and collapse accordions inside modern terminal apps like **OpenCode**, **Lazygit**, **Micro**, **Helix**, and **Neovim**.
- **Smooth Mouse Wheel Scrolling**: Scroll through diffs, file trees, and logs directly with the wheel.
- **Copy Text from Mouse-Aware Apps**: Hold the **`Shift`** key while dragging to bypass application mouse capture and perform standard terminal selection and clipboard copying.
- **Solid Theme Backgrounds**: Full-screen TUIs like OpenCode preserve their solid theme backgrounds without unreadable transparency bleed.
- **Dynamic Rounded Corners**: Edge quads on the bottom row curve seamlessly with the window's 14px outer / 13px inner radius, eliminating sharp 90-degree corner clipping.

---

## 9. Customization & Settings (`Ctrl+,`)

Press `Ctrl+,` or click the gear icon (`⚙`) in the top right to open the Settings dashboard:

### Themes & Luminance
- **Curated Presets**: Switch instantly between **Matte Dark** (`#0E0E0D` surface) and **Warm Paper** (`#EDE8DC` surface), or select **Nord**, **Tokyo Night**, and **Catppuccin**.
- **System Sync**: Enable automatic dark/light theme switching based on your Windows system preference.

### Acrylic Blur & Opacity
- **Opacity Slider (15% to 100%)**: Adjust background transparency. At 15%–85%, your desktop wallpaper and background apps blur smoothly through the terminal canvas via Windows DWM composition attributes (`SetWindowCompositionAttribute`).
- **100% Solid Mode**: Completely disables transparency for maximum contrast in bright environments.

### Typography & Fonts
- **Font Family**: Configure any monospaced font installed on your system (`Cascadia Code`, `JetBrains Mono NF`, `Fira Code`, `Consolas`). Fallbacks for Nerd Font symbols and emojis (`Segoe UI Emoji`) are automatically chained.
- **Font Size**: Adjustable in 0.5px increments (Default: `12.0px`).
- **Line Height**: Line spacing multiplier from `1.0` to `1.6` (Default: `1.2`).

### Cursor Aesthetics
- **Cursor Shapes**: Choose between `Block`, `Hollow Block`, `Beam`, and `Underline`.
- **Kinetic Physics**: Damped spring deceleration glides smoothly when typing and snaps instantly across large page jumps.
- **Breathing Alpha**: Organic breathing illumination when idle, locking to 100% solid opacity during active typing.

### Configuration & Prompt Files
- **Atlas Config (`settings.json`)**: Click **Edit settings.json** to directly edit font family, opacity %, themes, and custom keybindings.
- **PowerShell Profile (`$PROFILE`)**: Click **Edit $PROFILE** to configure your shell prompt, Starship, or Oh-My-Posh theme.
- **Fastfetch Config (`config.jsonc`)**: When Fastfetch is installed on your system, Settings automatically displays **Edit config.jsonc** to customize logo artwork, padding, and system specs.

---

## 10. WSL & Shell Profiles

Atlas Terminal automatically detects installed Windows Subsystem for Linux (WSL) distributions on startup and registers native vector distro marks (`Ubuntu`, `Arch Linux`, `Debian`, `Fedora`, `Kali`, `Alpine`, `Docker`):

- **Workspace Profile Menu**: Click the workspace pill on the left of the top bar to open the shell profile menu.
- **Fast Profile Launching**: Spawn dedicated tabs directly into PowerShell, Command Prompt, or any detected WSL Linux distribution.
- **Terminal Identity Forwarding**: Atlas forwards terminal identity into WSL environments so tools like `fastfetch` automatically detect and display the Atlas Terminal logo.
- **Procedural Prompt Vector Precision**: Native box-drawing and corner characters (`╭`, `╰`, `─`, `│`) render via DirectX GPU vector curves with dynamic stroke scaling. Starship, Oh-My-Posh, and WSL Arch/Ubuntu multi-line prompt arrows join seamlessly without gaps or jagged pixel steps, even when zoomed out.

---

## 11. Auto-Updater & Releases

Atlas Terminal features a built-in streaming auto-updater:
- **Silent Update Checks**: Automatically queries GitHub Releases with conditional ETag validation at configurable intervals (30 min, 1 hour, 6 hours, Daily).
- **Background Streaming**: Downloads binary delta patches asynchronously with real-time percentage indicators in the status bar.
- **Cryptographic Verification**: Every download is verified via SHA-256 (RFC 6234) against official release checksums before staging.
- **One-Click Restart**: Click the update pill to silently apply the update in-place and restart.
