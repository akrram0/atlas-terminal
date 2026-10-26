# Atlas Terminal — User Guide

Welcome to **Atlas Terminal**, the fastest and most aesthetic GPU-accelerated terminal for Windows, built with Rust and GPUI.

---

## Table of Contents
1. [Installation & Setup](#1-installation--setup)
2. [Interface Anatomy](#2-interface-anatomy)
3. [Keyboard Shortcuts Reference](#3-keyboard-shortcuts-reference)
4. [Customization & Settings](#4-customization--settings)
   - [Acrylic Blur & Opacity](#acrylic-blur--opacity)
   - [Themes & Luminance](#themes--luminance)
   - [Fonts & Sizing](#fonts--sizing)
5. [Inline Image Graphics (iTerm2 OSC 1337)](#5-inline-image-graphics-iterm2-osc-1337)
   - [Fastfetch Setup](#fastfetch-setup)
   - [Shell Scripts & CLI Tools](#shell-scripts--cli-tools)
6. [Tabs & Split Panes](#6-tabs--split-panes)
7. [Mouse Interactions & Hyperlinks](#7-mouse-interactions--hyperlinks)
8. [TUI Applications (OpenCode, Neovim, Tmux)](#8-tui-applications-opencode-neovim-tmux)
9. [WSL & Custom Shell Profiles](#9-wsl--custom-shell-profiles)

---

## 1. Installation & Setup

### Installer (Recommended)
Download the installer executable from the latest release:
- 📦 **[Download Latest Installer (`.exe`)](https://github.com/akrram0/atlas-terminal/releases/latest)**

The installer automatically:
- Installs Atlas Terminal into `%LOCALAPPDATA%\Programs\Atlas Terminal`
- Creates Start Menu and Desktop shortcuts
- Enables background delta updates

### Portable Archive
If you prefer a portable setup without an installer:
- 🗜️ **[Download Latest Portable (`.zip`)](https://github.com/akrram0/atlas-terminal/releases/latest)**
- Extract anywhere and run `atlas-terminal.exe`.

---

## 2. Interface Anatomy

Atlas Terminal is designed around a restrained, flat Apple/macOS aesthetic with zero unnecessary borders or nested cards:

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

| Shortcut | Action | Description |
| :--- | :--- | :--- |
| `Ctrl + Shift + T` | **New Tab** | Opens a new terminal tab with your default profile |
| `Ctrl + Shift + W` | **Close Tab / Pane** | Closes the active pane (or entire tab if single pane) |
| `Ctrl + Tab` | **Next Tab** | Cycle to the next tab |
| `Ctrl + Shift + Tab` | **Previous Tab** | Cycle to the previous tab |
| `Ctrl + 1` … `Ctrl + 9` | **Switch Tab** | Jump directly to tab index 1 through 9 |
| `Ctrl + Shift + D` | **Split Horizontal** | Splits current tab horizontally into two panes |
| `Ctrl + Shift + E` | **Split Vertical** | Splits current tab vertically into two panes |
| `Ctrl + Shift + ]` | **Focus Next Pane** | Move focus to adjacent pane in split view |
| `Ctrl + Shift + [` | **Focus Prev Pane** | Move focus to previous pane in split view |
| `Ctrl + Shift + M` | **Toggle Pane Maximize**| Temporarily maximize the active pane |
| `Ctrl + Shift + P` / `Ctrl + K` | **Command Palette** | Open palette to search actions and switch tabs |
| `Ctrl + ,` | **Settings** | Open Atlas settings dashboard |
| `Ctrl + Shift + C` | **Copy** | Copy selected terminal text |
| `Ctrl + Shift + V` | **Paste** | Paste clipboard into shell |
| `Ctrl + +` / `Ctrl + =` | **Zoom In** | Increase font size by 1px |
| `Ctrl + -` | **Zoom Out** | Decrease font size by 1px |
| `Ctrl + 0` | **Reset Zoom** | Reset font size to default (12px) |
| `Ctrl + Shift + K` | **Clear Scrollback** | Clear terminal history and reset viewport |
| `Shift + PageUp` | **Page Up** | Smooth kinetic scroll up one page |
| `Shift + PageDown` | **Page Down** | Smooth kinetic scroll down one page |
| `F11` | **Toggle Fullscreen** | Enter or exit borderless fullscreen mode |

---

## 4. Customization & Settings

Open settings anytime with `Ctrl + ,` or click the gear icon (`⚙`) in the top right.

### Acrylic Blur & Opacity
Atlas Terminal uses native Windows Desktop Window Manager (DWM) composition attributes (`SetWindowCompositionAttribute`) to render true hardware-accelerated Acrylic blur behind a transparent swapchain:
- **Opacity Slider (15% to 100%)**: Adjusts background transparency. At 15%–85%, your desktop wallpaper and background apps blur smoothly through the terminal canvas.
- **100% Solid Mode**: Completely disables transparency for maximum contrast in bright environments.

### Themes & Luminance
Atlas includes a curated selection of refined themes inspired by Zed and Claude Code:
1. **Warm Paper (Light)**: `#EDE8DC` warm cream surface with `#1C1A16` text and terracotta accents.
2. **Matte Dark (Default)**: `#0E0E0D` near-black surface with `#E8E4DC` text and `#D08A5C` cursor.
3. **Nord**: Arctic cool slate palette.
4. **Tokyo Night**: Vibrant cyber-dark palette.
5. **Catppuccin Macchiato & Mocha**: Soft pastel contrast themes.

### Fonts & Sizing
- **Font Family**: Configure any monospaced font installed on your system (`Cascadia Code`, `JetBrains Mono NF`, `Fira Code`, `Consolas`). Fallbacks for Nerd Font symbols and emojis (`Segoe UI Emoji`) are automatically chained.
- **Font Size**: Adjustable in 0.5px increments (Default: `12.0px`).
- **Line Height**: Line spacing multiplier from `1.0` to `1.6` (Default: `1.2`).

---

## 5. Inline Image Graphics (iTerm2 OSC 1337)

Atlas Terminal natively intercepts and decodes inline graphics via the standard iTerm2 OSC 1337 protocol on the GPU, avoiding legacy Sixel limitations.

### Fastfetch Setup

To display your profile picture or logo alongside system info in Fastfetch:

1. Create or edit `%USERPROFILE%\.config\fastfetch\config.jsonc`:
```jsonc
{
  "$schema": "https://github.com/fastfetch-cli/fastfetch/raw/dev/doc/json_schema.json",
  "logo": {
    "type": "iterm",
    "source": "C:/Users/YourUsername/Pictures/avatar.png",
    "width": 36,
    "height": 18,
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

2. Run `fastfetch` in Atlas Terminal. The graphic renders anchored cleanly on the left with intentional vertical centering ("f waset") relative to the spec block.

### Shell Scripts & CLI Tools

Any script or CLI that outputs iTerm2 inline graphics works out of the box:

```bash
# Bash / Zsh
printf '\033]1337;File=inline=1;width=40;height=20:%s\007\n' "$(base64 -w0 photo.png)"
```

```powershell
# PowerShell
$b64 = [Convert]::ToBase64String([IO.File]::ReadAllBytes("photo.png"))
Write-Host "$([char]27)]1337;File=inline=1;width=40;height=20:$b64$([char]7)"
```

---

## 6. Tabs & Split Panes

- **New Tab**: Click the `+` button in the top bar or press `Ctrl + Shift + T`.
- **Rename Tab**: Double click any tab title or press `Ctrl + Shift + P` -> "Rename Tab".
- **Split Panes**: Split any tab into multiple simultaneous terminals:
  - `Ctrl + Shift + D` splits horizontally.
  - `Ctrl + Shift + E` splits vertically.
  - `Ctrl + Shift + ]` cycles focus to the next pane.
  - `Ctrl + Shift + M` toggles pane maximization so you can focus on one task and restore the split layout later.

---

## 7. Mouse Interactions & Hyperlinks

- **Clickable URLs**: Hold `Ctrl` and hover over any URL (`http://`, `https://`, `git://`). The URL underlines and turns into a pointer; clicking launches your default browser.
- **Copy on Select**: Enable "Copy on select" in Settings (`Ctrl + ,`) to automatically copy highlighted text to the clipboard without pressing `Ctrl + Shift + C`.
- **Smooth Kinetic Scrolling**: Mouse wheel scrolling uses Apple spring easing to glide smoothly across history. Holding `Ctrl` while scrolling the mouse wheel zooms font size up or down.

---

## 8. TUI Applications (OpenCode, Neovim, Tmux)

Atlas Terminal provides specialized handling for alternate screen terminal applications:
- **Solid Theme Backgrounds Preserved**: Full-screen TUIs like OpenCode, Claude Code, and Neovim preserve their solid theme backgrounds without unreadable transparency.
- **Rounded Edge Quads**: Edge quads on the bottom row curve seamlessly with the window's 14px outer / 13px inner radius, eliminating sharp 90-degree corner bleed.
- **Mouse Reporting**: Full support for SGR 1006 mouse tracking (drag selection, button clicks, and scroll wheel events passed to TUIs).

---

## 9. WSL & Custom Shell Profiles

Atlas Terminal automatically queries WSL for installed distributions on startup and registers native vector distro marks (`Ubuntu`, `Arch Linux`, `Debian`, `Fedora`, `Kali`, `Alpine`, `Docker`):
- Click the workspace pill on the left of the top bar to open the shell profile menu.
- Launch new tabs directly into PowerShell, CMD, or any detected WSL Linux distribution.
